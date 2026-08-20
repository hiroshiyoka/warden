use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::PgPool;

pub async fn record_event(
    pool: &PgPool,
    tenant_id: Option<uuid::Uuid>,
    user_id: Option<uuid::Uuid>,
    event_type: &str,
    detail: Value,
) -> Result<(), sqlx::Error> {
    let prev_hash: Option<String> =
        sqlx::query_scalar("SELECT row_hash FROM audit_log ORDER BY created_at DESC LIMIT 1")
            .fetch_optional(pool)
            .await?;

    let mut hasher = Sha256::new();
    hasher.update(prev_hash.clone().unwrap_or_default());
    hasher.update(event_type);
    hasher.update(detail.to_string());
    let row_hash = format!("{:x}", hasher.finalize());

    sqlx::query(
        "INSERT INTO audit_log (tenant_id, user_id, event_type, detail, prev_hash, row_hash)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(tenant_id)
    .bind(user_id)
    .bind(event_type)
    .bind(detail)
    .bind(prev_hash)
    .bind(row_hash)
    .execute(pool)
    .await?;

    Ok(())
}