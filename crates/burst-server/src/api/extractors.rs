use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use uuid::Uuid;

use crate::AppState;
use crate::db;
use crate::error::ApiError;

/// Extracts the authenticated user ID from the `X-Auth-Consumer` header
/// set by the Barbacane gateway after authentication (basic-auth, jwt-auth, etc.).
///
/// The header contains the external identity (e.g. a username for basic-auth,
/// a `sub` claim for OIDC). We look up the corresponding Burst user by
/// `external_id` in the database. If no user exists, JIT provisioning creates
/// one automatically (ADR-006).
pub struct AuthUser {
    pub user_id: Uuid,
    pub user_role: String,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let external_id = parts
            .headers
            .get("x-auth-consumer")
            .and_then(|v| v.to_str().ok())
            .ok_or(ApiError::Unauthorized)?;

        let claims = extract_claims(parts);

        let user = match db::users::find_by_external_id(&state.db, external_id).await? {
            Some(user) => {
                // Re-sync profile from OIDC claims on each request (lazy sync).
                if let Some(ref c) = claims {
                    sync_profile_from_claims(&state.db, &user, c).await;
                }
                user
            }
            None => {
                // JIT provisioning: create the user on first authenticated request.
                let display_name = claims
                    .as_ref()
                    .and_then(extract_display_name)
                    .unwrap_or_else(|| humanize_username(external_id));
                let email = claims
                    .as_ref()
                    .and_then(|c| extract_claim_string(c, "email"));
                super::users::jit_provision(
                    &state.db,
                    external_id,
                    external_id,
                    &display_name,
                    email.as_deref(),
                )
                .await?;

                // Set avatar URL if available in claims.
                let user = db::users::find_by_external_id(&state.db, external_id)
                    .await?
                    .ok_or(ApiError::Unauthorized)?;
                if let Some(ref c) = claims
                    && let Some(picture) = extract_claim_string(c, "picture")
                    && let Err(e) = db::users::update_avatar(&state.db, user.id, &picture).await
                {
                    tracing::warn!(user_id = %user.id, error = %e, "failed to set avatar from OIDC claims");
                }
                db::users::find_by_external_id(&state.db, external_id)
                    .await?
                    .ok_or(ApiError::Unauthorized)?
            }
        };

        Ok(AuthUser {
            user_id: user.id,
            user_role: user.role,
        })
    }
}

/// Parse the x-auth-claims header into a JSON value.
fn extract_claims(parts: &Parts) -> Option<serde_json::Value> {
    let claims_str = parts
        .headers
        .get("x-auth-claims")
        .and_then(|v| v.to_str().ok())?;
    serde_json::from_str(claims_str).ok()
}

/// Extract a string claim by key.
fn extract_claim_string(claims: &serde_json::Value, key: &str) -> Option<String> {
    claims.get(key).and_then(|v| v.as_str()).map(String::from)
}

/// Extract display name from OIDC claims.
/// Tries: "name", then "given_name" + "family_name", then "preferred_username".
fn extract_display_name(claims: &serde_json::Value) -> Option<String> {
    if let Some(name) = extract_claim_string(claims, "name") {
        return Some(name);
    }
    let given = extract_claim_string(claims, "given_name");
    let family = extract_claim_string(claims, "family_name");
    match (given, family) {
        (Some(g), Some(f)) => Some(format!("{g} {f}")),
        (Some(g), None) => Some(g),
        _ => extract_claim_string(claims, "preferred_username"),
    }
}

/// Re-sync user profile from OIDC claims (lazy — runs on each authenticated request).
/// Only updates fields that have changed to avoid unnecessary DB writes.
async fn sync_profile_from_claims(
    pool: &sqlx::PgPool,
    user: &db::users::UserRow,
    claims: &serde_json::Value,
) {
    let new_name = extract_display_name(claims);
    let new_email = extract_claim_string(claims, "email");
    let new_avatar = extract_claim_string(claims, "picture");

    let name_changed = new_name.as_ref().is_some_and(|n| n != &user.display_name);
    let email_changed = new_email.is_some() && new_email != user.email;
    let avatar_changed = new_avatar.is_some() && new_avatar != user.avatar_url;

    if (name_changed || email_changed || avatar_changed)
        && let Err(e) = db::users::sync_profile(
            pool,
            user.id,
            new_name.as_deref(),
            new_email.as_deref(),
            new_avatar.as_deref(),
        )
        .await
    {
        tracing::warn!(user_id = %user.id, error = %e, "failed to sync profile from OIDC claims");
    }
}

/// Convert a username like "alice" to a display name like "Alice".
fn humanize_username(username: &str) -> String {
    let mut chars = username.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

/// Extracts an authenticated admin user. Rejects non-admin roles with 403.
pub struct AdminUser {
    pub user_id: Uuid,
}

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth = AuthUser::from_request_parts(parts, state).await?;
        if auth.user_role != "admin" {
            return Err(ApiError::Forbidden);
        }
        Ok(AdminUser {
            user_id: auth.user_id,
        })
    }
}

/// Pagination query parameters.
#[derive(Debug, serde::Deserialize)]
pub struct PaginationParams {
    pub cursor: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    50
}

impl PaginationParams {
    pub fn clamped_limit(&self) -> i64 {
        self.limit.clamp(1, 200)
    }

    /// Parse the cursor string into a UUID, stripping any known prefix
    /// (e.g. `msg_`, `ch_`, `usr_`, `att_`).
    pub fn cursor_uuid(&self) -> Option<Uuid> {
        self.cursor.as_deref().and_then(|s| {
            // Try stripping known prefixes, fall back to raw UUID parse
            for prefix in &["msg_", "ch_", "usr_", "att_"] {
                if let Some(rest) = s.strip_prefix(prefix) {
                    return Uuid::parse_str(rest).ok();
                }
            }
            Uuid::parse_str(s).ok()
        })
    }
}
