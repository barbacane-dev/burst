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

        let user = match db::users::find_by_external_id(&state.db, external_id).await? {
            Some(user) => user,
            None => {
                // JIT provisioning: create the user on first authenticated request.
                let email = extract_email_from_claims(parts);
                let display_name = humanize_username(external_id);
                super::users::jit_provision(
                    &state.db,
                    external_id,
                    external_id,
                    &display_name,
                    email.as_deref(),
                )
                .await?;
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

/// Try to extract an email from the `x-auth-claims` header (JSON-encoded JWT claims).
fn extract_email_from_claims(parts: &Parts) -> Option<String> {
    let claims_str = parts
        .headers
        .get("x-auth-claims")
        .and_then(|v| v.to_str().ok())?;
    let claims: serde_json::Value = serde_json::from_str(claims_str).ok()?;
    claims
        .get("email")
        .and_then(|v| v.as_str())
        .map(String::from)
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
