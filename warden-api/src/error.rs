use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde_json::json;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("bad request")]
    BadRequest,
    #[error("not found")]
    NotFound,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
            AppError::BadRequest => (StatusCode::BAD_REQUEST, "bad request"),
            AppError::NotFound => (StatusCode::NOT_FOUND, "not found"),
        };
        tracing::error!(error = ?self, "request failed");
        (status, Json(json!({ "error": message }))).into_response()
    }
}
