use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use warden_sandbox::runtime::RuntimeError;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("bad request")]
    BadRequest,
    #[error("sandbox runtime error")]
    Runtime(#[from] RuntimeError),
    #[error("sandbox quota exceeded")]
    QuotaExceeded,
    #[error("not found")]
    NotFound,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
            AppError::BadRequest => (StatusCode::BAD_REQUEST, "bad request"),
            AppError::Runtime(_) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "sandbox runtime not yet available",
            ),
            AppError::QuotaExceeded => (StatusCode::TOO_MANY_REQUESTS, "sandbox quota exceeded"),
            AppError::NotFound => (StatusCode::NOT_FOUND, "not found"),
        };
        tracing::error!(error = ?self, "request failed");
        (status, Json(json!({ "error": message }))).into_response()
    }
}
