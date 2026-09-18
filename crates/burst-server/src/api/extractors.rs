use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use uuid::Uuid;

use crate::AppState;
use crate::api::trusted_peer;
use crate::db;
use crate::error::ApiError;

/// Whether the connecting peer is one whose `X-Auth-*` headers are believed.
///
/// A request with no peer address recorded is refused: that means the server
/// was not built with connection info, and accepting the headers anyway would
/// silently disable the check.
pub(crate) fn peer_is_trusted(parts: &Parts, trusted: &[String]) -> bool {
    match parts
        .extensions
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
    {
        Some(info) => trusted_peer::is_trusted(info.0.ip(), trusted),
        None => false,
    }
}

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
        // The identity headers name the caller and carry the role, so they are
        // believed only from the gateway that sets them. Every other peer is
        // anonymous, whatever it sends.
        if !peer_is_trusted(parts, &state.config.auth.trusted_proxies) {
            return Err(ApiError::Unauthorized);
        }

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
                    sync_profile_from_claims(&state.db, &user, c, parts).await;
                }
                user
            }
            None => {
                // JIT provisioning: create the user on first authenticated request.
                let display_name = claims
                    .as_ref()
                    .and_then(extract_display_name)
                    .unwrap_or_else(|| humanize_username(external_id));
                let username = claims
                    .as_ref()
                    .and_then(|c| extract_claim_string(c, "preferred_username"))
                    .unwrap_or_else(|| external_id.to_string());
                let email = claims
                    .as_ref()
                    .and_then(|c| extract_claim_string(c, "email"));
                let role = role_from_groups(parts).unwrap_or_else(|| "member".to_string());
                super::users::jit_provision(
                    &state.db,
                    external_id,
                    &username,
                    &display_name,
                    email.as_deref(),
                    &role,
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
    parts: &Parts,
) {
    let new_name = extract_display_name(claims);
    let new_email = extract_claim_string(claims, "email");
    let new_avatar = extract_claim_string(claims, "picture");
    let new_username = extract_claim_string(claims, "preferred_username");

    let name_changed = new_name.as_ref().is_some_and(|n| n != &user.display_name);
    let email_changed = new_email.is_some() && new_email != user.email;
    let avatar_changed = new_avatar.is_some() && new_avatar != user.avatar_url;
    let username_changed = new_username.as_ref().is_some_and(|u| u != &user.username);

    if (name_changed || email_changed || avatar_changed || username_changed)
        && let Err(e) = db::users::sync_profile(
            pool,
            user.id,
            new_name.as_deref(),
            new_email.as_deref(),
            new_avatar.as_deref(),
            new_username.as_deref(),
        )
        .await
    {
        tracing::warn!(user_id = %user.id, error = %e, "failed to sync profile from OIDC claims");
    }

    // The identity provider is authoritative for the role where it says
    // anything. Where it says nothing, the stored role stands, so a role an
    // admin set by hand survives the next request.
    if let Some(new_role) = role_from_groups(parts)
        && new_role != user.role
        && let Err(e) = db::users::update_role(pool, user.id, &new_role).await
    {
        tracing::warn!(user_id = %user.id, error = %e, "failed to sync role from groups");
    }
}

/// The role the gateway's `x-auth-consumer-groups` header implies, if it sent
/// one.
///
/// `None` means the header is absent, which is not the same as "no groups": the
/// auth plugins omit it entirely for an identity that has none, and a
/// deployment may not map groups at all. Reading that absence as `member` would
/// demote every user whose identity carries no groups, including one an admin
/// has just promoted by hand.
fn role_from_groups(parts: &Parts) -> Option<String> {
    let groups = parts
        .headers
        .get("x-auth-consumer-groups")
        .and_then(|v| v.to_str().ok())?;
    let groups: Vec<&str> = groups.split(',').map(|g| g.trim()).collect();
    Some(if groups.contains(&"admin") {
        "admin".to_string()
    } else if groups.contains(&"integrator") {
        "integrator".to_string()
    } else {
        "member".to_string()
    })
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

/// Extracts an authenticated user with integration management privileges.
/// Accepts users with `admin` or `integrator` roles. Rejects others with 403.
pub struct IntegrationUser {
    pub user_id: Uuid,
}

impl FromRequestParts<AppState> for IntegrationUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth = AuthUser::from_request_parts(parts, state).await?;
        if auth.user_role != "admin" && auth.user_role != "integrator" {
            return Err(ApiError::Forbidden);
        }
        Ok(IntegrationUser {
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
            for prefix in &["msg_", "ch_", "usr_", "att_", "wh_"] {
                if let Some(rest) = s.strip_prefix(prefix) {
                    return Uuid::parse_str(rest).ok();
                }
            }
            Uuid::parse_str(s).ok()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use serde_json::json;

    #[test]
    fn extract_display_name_from_name_claim() {
        let claims = json!({"name": "Alice Smith", "preferred_username": "alice"});
        assert_eq!(extract_display_name(&claims), Some("Alice Smith".into()));
    }

    #[test]
    fn extract_display_name_from_given_family() {
        let claims = json!({"given_name": "Alice", "family_name": "Smith"});
        assert_eq!(extract_display_name(&claims), Some("Alice Smith".into()));
    }

    #[test]
    fn extract_display_name_from_preferred_username() {
        let claims = json!({"preferred_username": "alice"});
        assert_eq!(extract_display_name(&claims), Some("alice".into()));
    }

    #[test]
    fn extract_display_name_no_claims() {
        let claims = json!({"sub": "abc"});
        assert_eq!(extract_display_name(&claims), None);
    }

    #[test]
    fn humanize_username_capitalizes() {
        assert_eq!(humanize_username("alice"), "Alice");
        assert_eq!(humanize_username(""), "");
    }

    fn make_parts_with_groups(groups: Option<&str>) -> Parts {
        let mut builder = Request::builder().method("GET").uri("/");
        if let Some(g) = groups {
            builder = builder.header("x-auth-consumer-groups", g);
        }
        builder.body(()).unwrap().into_parts().0
    }

    #[test]
    fn role_from_admin_group() {
        let parts = make_parts_with_groups(Some("admin"));
        assert_eq!(role_from_groups(&parts).as_deref(), Some("admin"));
    }

    #[test]
    fn role_from_multiple_groups_with_admin() {
        let parts = make_parts_with_groups(Some("member, admin, moderator"));
        assert_eq!(role_from_groups(&parts).as_deref(), Some("admin"));
    }

    #[test]
    fn role_from_integrator_group() {
        let parts = make_parts_with_groups(Some("integrator"));
        assert_eq!(role_from_groups(&parts).as_deref(), Some("integrator"));
    }

    #[test]
    fn role_admin_takes_precedence_over_integrator() {
        let parts = make_parts_with_groups(Some("integrator, admin"));
        assert_eq!(role_from_groups(&parts).as_deref(), Some("admin"));
    }

    #[test]
    fn role_defaults_to_member() {
        let parts = make_parts_with_groups(Some("user, editor"));
        assert_eq!(role_from_groups(&parts).as_deref(), Some("member"));
    }

    /// The auth plugins omit the header for an identity with no groups, and a
    /// deployment may not map groups at all. Reading that as `member` demoted
    /// every such user on their next request, including one an admin had just
    /// promoted by hand.
    #[test]
    fn an_absent_header_says_nothing_about_the_role() {
        let parts = make_parts_with_groups(None);
        assert_eq!(role_from_groups(&parts), None);
    }

    /// An empty value is a statement, unlike an absent header: the identity was
    /// asked about its groups and has none.
    #[test]
    fn an_empty_header_means_member() {
        let parts = make_parts_with_groups(Some(""));
        assert_eq!(role_from_groups(&parts).as_deref(), Some("member"));
    }

    #[test]
    fn extract_claim_string_returns_value() {
        let claims = json!({"email": "alice@example.com"});
        assert_eq!(
            extract_claim_string(&claims, "email"),
            Some("alice@example.com".into())
        );
    }

    #[test]
    fn extract_claim_string_returns_none_for_missing() {
        let claims = json!({"sub": "abc"});
        assert_eq!(extract_claim_string(&claims, "email"), None);
    }
}
