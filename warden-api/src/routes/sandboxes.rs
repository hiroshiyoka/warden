use crate::{audit, auth::AuthContext, error::AppError, AppState};
use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use warden_sandbox::config::ResourceLimits;

const MAX_ACTIVE_SANDBOXES_PER_TENANT: i64 = 5;

#[derive(Debug, Deserialize)]
pub struct CreateRequest {
    #[serde(default = "default_vcpus")]
    pub vcpu_count: u8,
    #[serde(default = "default_memory")]
    pub memory_mib: u32,
    #[serde(default = "default_timeout")]
    pub exec_timeout_secs: u64,
}

#[derive(Debug, Serialize, FromRow)]
pub struct SandboxResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub destroyed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct ExecRequest {
    pub command: String,
}

fn default_vcpus() -> u8 {
    1
}
fn default_memory() -> u32 {
    512
}
fn default_timeout() -> u64 {
    30
}

pub async fn create(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<CreateRequest>,
) -> Result<(StatusCode, Json<SandboxResponse>), AppError> {
    let active: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sandboxes WHERE tenant_id = $1 AND status != 'destroyed'",
    )
    .bind(auth.tenant_id)
    .fetch_one(&state.pool)
    .await?;
    if active >= MAX_ACTIVE_SANDBOXES_PER_TENANT {
        return Err(AppError::QuotaExceeded);
    }

    let limits = ResourceLimits {
        vcpu_count: payload.vcpu_count,
        memory_mib: payload.memory_mib,
        exec_timeout: std::time::Duration::from_secs(payload.exec_timeout_secs),
    };
    limits.validate().map_err(|_| AppError::BadRequest)?;

    let sandbox_id = Uuid::new_v4();
    match state.runtime.boot(sandbox_id, &limits).await {
        Ok(()) => Err(AppError::Runtime(
            warden_sandbox::runtime::RuntimeError::NotImplemented,
        )),
        Err(error @ warden_sandbox::runtime::RuntimeError::NotImplemented) => {
            sqlx::query("INSERT INTO sandboxes (id, tenant_id) VALUES ($1, $2)")
                .bind(sandbox_id)
                .bind(auth.tenant_id)
                .execute(&state.pool)
                .await?;
            audit::record_event(
                &state.pool,
                Some(auth.tenant_id),
                Some(auth.user_id),
                "sandbox_create_pending_runtime",
                serde_json::json!({ "sandbox_id": sandbox_id }),
            )
            .await?;
            Err(AppError::Runtime(error))
        }
        Err(error) => Err(AppError::Runtime(error)),
    }
}

pub async fn list(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<SandboxResponse>>, AppError> {
    let rows = sqlx::query_as::<_, SandboxResponse>(
        "SELECT id, tenant_id, status, created_at, destroyed_at
         FROM sandboxes WHERE tenant_id = $1 ORDER BY created_at DESC",
    )
    .bind(auth.tenant_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

pub async fn exec(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ExecRequest>,
) -> Result<Json<warden_sandbox::runtime::ExecResult>, AppError> {
    ensure_tenant_owns_sandbox(&state.pool, auth.tenant_id, id).await?;
    let result = state
        .runtime
        .exec(id, &payload.command, std::time::Duration::from_secs(30))
        .await?;
    Ok(Json(result))
}

pub async fn destroy(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    ensure_tenant_owns_sandbox(&state.pool, auth.tenant_id, id).await?;
    state.runtime.destroy(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn ensure_tenant_owns_sandbox(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    sandbox_id: Uuid,
) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM sandboxes WHERE id = $1 AND tenant_id = $2)",
    )
    .bind(sandbox_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;
    if !exists {
        return Err(AppError::NotFound);
    }
    Ok(())
}
