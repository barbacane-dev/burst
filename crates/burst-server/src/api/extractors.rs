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
/// `external_id` in the database.
pub struct AuthUser {
    pub user_id: Uuid,
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

        let user = db::users::find_by_external_id(&state.db, external_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or(ApiError::Unauthorized)?;

        Ok(AuthUser { user_id: user.id })
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
