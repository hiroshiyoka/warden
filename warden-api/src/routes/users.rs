use crate::{auth::AuthContext, error::AppError};
use axum::{extract::{Extension, State}, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct InviteRequest {
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct InviteResponse {
    pub id: Uuid,
    pub email: String,
}

pub async fn invite(
    State(pool): State<PgPool>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<InviteRequest>,
) -> Result<(StatusCode, Json<InviteResponse>), AppError> {
    let email = payload.email.trim();
    if email.is_empty() {
        return Err(AppError::BadRequest);
    }

    let email = email.to_owned();
    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO users (tenant_id, email) VALUES ($1, $2) RETURNING id",
    )
    .bind(auth.tenant_id)
    .bind(&email)
    .fetch_one(&pool)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(InviteResponse { id, email }),
    ))
}
