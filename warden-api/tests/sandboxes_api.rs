mod common;

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tower::ServiceExt as _;
use uuid::Uuid;
use warden_api::{auth::AuthContext, routes, AppState};
use warden_sandbox::{
    config::ResourceLimits,
    runtime::{ExecResult, RuntimeError, SandboxRuntime},
};

const DATABASE_URL: &str = "DATABASE_URL";

struct CountingRuntime {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl SandboxRuntime for CountingRuntime {
    async fn boot(&self, _id: Uuid, _limits: &ResourceLimits) -> Result<(), RuntimeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(RuntimeError::NotImplemented)
    }

    async fn exec(
        &self,
        _id: Uuid,
        _command: &str,
        _timeout: std::time::Duration,
    ) -> Result<ExecResult, RuntimeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(RuntimeError::NotImplemented)
    }

    async fn destroy(&self, _id: Uuid) -> Result<(), RuntimeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(RuntimeError::NotImplemented)
    }
}

async fn make_pool() -> PgPool {
    let url = std::env::var(DATABASE_URL).expect("DATABASE_URL must be set for integration tests");
    PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("failed to connect to postgres")
}

async fn grant_create(pool: &PgPool, tenant_id: Uuid, user_id: Uuid) {
    let role_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO roles (tenant_id, name) VALUES ($1, $2) RETURNING id",
    )
    .bind(tenant_id)
    .bind(format!("creator-{}", Uuid::new_v4()))
    .fetch_one(pool)
    .await
    .expect("insert creator role");
    let permission_id =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM permissions WHERE key = 'sandbox:create'")
            .fetch_one(pool)
            .await
            .expect("lookup create permission");
    sqlx::query("INSERT INTO role_permissions (role_id, permission_id) VALUES ($1, $2)")
        .bind(role_id)
        .bind(permission_id)
        .execute(pool)
        .await
        .expect("link create permission");
    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2)")
        .bind(user_id)
        .bind(role_id)
        .execute(pool)
        .await
        .expect("link creator role");
}

async fn setup(pool: &PgPool) -> (Uuid, Uuid) {
    common::run_migrations(pool).await;
    let tenant_id =
        sqlx::query_scalar::<_, Uuid>("INSERT INTO tenants (name) VALUES ($1) RETURNING id")
            .bind(format!("sandbox-api-{}", Uuid::new_v4()))
            .fetch_one(pool)
            .await
            .expect("insert tenant");
    let user_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO users (tenant_id, email) VALUES ($1, $2) RETURNING id",
    )
    .bind(tenant_id)
    .bind(format!("{}@example.com", Uuid::new_v4()))
    .fetch_one(pool)
    .await
    .expect("insert user");
    (tenant_id, user_id)
}

fn request(tenant_id: Uuid, user_id: Uuid) -> Request<Body> {
    let mut request = Request::builder()
        .uri("/sandboxes")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(json!({}).to_string()))
        .expect("build sandbox request");
    request
        .extensions_mut()
        .insert(AuthContext { tenant_id, user_id });
    request
}

fn app(pool: PgPool, calls: Arc<AtomicUsize>) -> Router {
    Router::new().merge(routes::router_with_state(AppState {
        pool,
        runtime: Arc::new(CountingRuntime { calls }),
    }))
}

#[tokio::test]
async fn denied_request_does_not_insert_or_call_runtime() {
    let pool = make_pool().await;
    let (tenant_id, user_id) = setup(&pool).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let response = app(pool.clone(), calls.clone())
        .oneshot(request(tenant_id, user_id))
        .await
        .expect("call denied create");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM sandboxes WHERE tenant_id = $1")
        .bind(tenant_id)
        .fetch_one(&pool)
        .await
        .expect("count sandboxes");
    assert_eq!(count, 0);
}

#[tokio::test]
async fn quota_rejection_happens_before_runtime() {
    let pool = make_pool().await;
    let (tenant_id, user_id) = setup(&pool).await;
    grant_create(&pool, tenant_id, user_id).await;
    for _ in 0..5 {
        sqlx::query("INSERT INTO sandboxes (tenant_id) VALUES ($1)")
            .bind(tenant_id)
            .execute(&pool)
            .await
            .expect("insert active sandbox");
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let response = app(pool, calls.clone())
        .oneshot(request(tenant_id, user_id))
        .await
        .expect("call quota-rejected create");
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn permitted_request_records_pending_intent_and_returns_503() {
    let pool = make_pool().await;
    let (tenant_id, user_id) = setup(&pool).await;
    grant_create(&pool, tenant_id, user_id).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let response = app(pool.clone(), calls.clone())
        .oneshot(request(tenant_id, user_id))
        .await
        .expect("call pending create");
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let status: String = sqlx::query_scalar(
        "SELECT status FROM sandboxes WHERE tenant_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .expect("find pending sandbox");
    assert_eq!(status, "pending_runtime");
    let audited: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE tenant_id = $1 AND event_type = 'sandbox_create_pending_runtime'").bind(tenant_id).fetch_one(&pool).await.expect("count sandbox audit");
    assert!(audited >= 1);
}
