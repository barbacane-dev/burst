use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use burst_core::error::ProblemDetails;

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let problem = match &self {
            ApiError::NotFound(resource) => ProblemDetails::not_found(resource),
            ApiError::Unauthorized => ProblemDetails::unauthorized(),
            ApiError::Forbidden => ProblemDetails::forbidden(),
            ApiError::BadRequest(msg) => ProblemDetails::bad_request(msg.as_str()),
            ApiError::Conflict(msg) => ProblemDetails::conflict(msg.as_str()),
            ApiError::PayloadTooLarge(msg) => ProblemDetails::payload_too_large(msg.as_str()),
            ApiError::Internal(msg) => {
                tracing::error!("internal error: {msg}");
                ProblemDetails::internal_error()
            }
        };

        let status =
            StatusCode::from_u16(problem.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

        (
            status,
            [("content-type", "application/problem+json")],
            serde_json::to_string(&problem).unwrap_or_default(),
        )
            .into_response()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("payload too large: {0}")]
    PayloadTooLarge(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        if let sqlx::Error::Database(ref db_err) = err {
            match db_err.code().as_deref() {
                // Unique constraint violation
                Some("23505") => return ApiError::Conflict("resource already exists".into()),
                // Foreign key violation
                Some("23503") => {
                    return ApiError::BadRequest("referenced resource does not exist".into());
                }
                // Check constraint violation
                Some("23514") => return ApiError::BadRequest("value violates constraint".into()),
                // Not-null violation
                Some("23502") => return ApiError::BadRequest("required field is missing".into()),
                _ => {}
            }
        }
        tracing::error!(error = %err, "unhandled database error");
        ApiError::Internal("database error".into())
    }
}
