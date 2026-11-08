use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// Every handler returns `Result<T, AppError>`. This enum is the single place
/// that maps domain failures onto the HTTP error envelope in docs/API_SPEC.md.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("validation error: {0}")]
    Validation(String),

    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("not found")]
    NotFound,

    #[error("email already registered")]
    EmailTaken,

    #[error("invalid state transition: {0}")]
    InvalidState(String),

    #[error("revision limit reached")]
    RevisionLimitReached,

    #[error("insufficient funds")]
    InsufficientFunds,

    #[error("entitlement expired")]
    EntitlementExpired,

    #[error("download limit exhausted")]
    DownloadsExhausted,

    #[error("rate limited")]
    RateLimited,

    #[error("not implemented: {0}")]
    NotImplemented(&'static str),

    #[error(transparent)]
    Db(#[from] sqlx::Error),

    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    fn status_and_code(&self) -> (StatusCode, &'static str) {
        match self {
            AppError::Validation(_) => (StatusCode::BAD_REQUEST, "validation_error"),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            AppError::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            AppError::EmailTaken => (StatusCode::CONFLICT, "email_taken"),
            AppError::InvalidState(_) => (StatusCode::CONFLICT, "invalid_state_transition"),
            AppError::RevisionLimitReached => (StatusCode::CONFLICT, "revision_limit_reached"),
            AppError::InsufficientFunds => (StatusCode::PAYMENT_REQUIRED, "insufficient_funds"),
            AppError::EntitlementExpired => (StatusCode::GONE, "entitlement_expired"),
            AppError::DownloadsExhausted => (StatusCode::GONE, "downloads_exhausted"),
            AppError::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "rate_limited"),
            AppError::NotImplemented(_) => (StatusCode::NOT_IMPLEMENTED, "not_implemented"),
            AppError::Db(_) | AppError::Internal(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
            }
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = self.status_and_code();

        // Never leak database/runtime detail to the client.
        let message = match &self {
            AppError::Db(_) | AppError::Internal(_) => {
                tracing::error!(error = %self, "internal server error");
                "An internal error occurred.".to_string()
            }
            other => other.to_string(),
        };

        let body = Json(json!({
            "error": {
                "code": code,
                "message": message,
                "details": null
            }
        }));

        (status, body).into_response()
    }
}

/// Convenience alias used by handlers.
pub type AppResult<T> = Result<T, AppError>;
