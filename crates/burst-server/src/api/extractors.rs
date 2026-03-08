use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use uuid::Uuid;

use crate::AppState;
use crate::error::ApiError;

/// Extracts the authenticated user ID.
///
/// Tries `X-Auth-Consumer` header first (set by Barbacane gateway after JWT validation).
/// Falls back to validating the `Authorization: Bearer <token>` JWT directly,
/// which is needed for local development without the gateway.
pub struct AuthUser {
    pub user_id: Uuid,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Trust Barbacane header only when explicitly enabled in config.
        // Without this guard, any client that reaches the port can forge X-Auth-Consumer.
        if state.config.trust_auth_headers
            && let Some(header) = parts
                .headers
                .get("x-auth-consumer")
                .and_then(|v| v.to_str().ok())
        {
            let user_id = Uuid::parse_str(header).map_err(|_| ApiError::Unauthorized)?;
            return Ok(AuthUser { user_id });
        }

        // Fall back to JWT validation (local dev without gateway)
        let auth_header = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or(ApiError::Unauthorized)?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(ApiError::Unauthorized)?;

        let claims = crate::auth::validate_access_token(&state.config, token)
            .map_err(|_| ApiError::Unauthorized)?;

        let user_id = Uuid::parse_str(&claims.sub).map_err(|_| ApiError::Unauthorized)?;

        Ok(AuthUser { user_id })
    }
}

/// Pagination query parameters.
#[derive(Debug, serde::Deserialize)]
pub struct PaginationParams {
    pub cursor: Option<Uuid>,
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
}
