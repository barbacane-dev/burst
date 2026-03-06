use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use uuid::Uuid;

use crate::error::ApiError;

/// Extracts the authenticated user ID from Barbacane's `X-Auth-Consumer` header.
pub struct AuthUser {
    pub user_id: Uuid,
}

impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get("x-auth-consumer")
            .and_then(|v| v.to_str().ok())
            .ok_or(ApiError::Unauthorized)?;

        let user_id = Uuid::parse_str(header).map_err(|_| ApiError::Unauthorized)?;

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
