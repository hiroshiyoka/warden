use axum::{extract::State, Json};
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::error::AppError;

pub async fn health(State(pool): State<PgPool>) -> Result<Json<Value>, AppError> {
    sqlx::query("SELECT 1").execute(&pool).await?;
    Ok(Json(json!({ "status": "ok" })))
}
