mod common;

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::{self, Next},
    routing::get,
    Router,
};
use sqlx::{postgres::PgPoolOptions, PgPool};
use tower::ServiceExt as _;
use uuid::Uuid;
use warden_api::auth::middleware::require_permission;
use warden_api::auth::permission::Permission;
use warden_api::auth::AuthContext;

async fn make_pool() -> PgPool {
    let url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");
    PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("failed to connect to postgres")
}

async fn grant_permission(pool: &PgPool, tenant_id: Uuid, user_id: Uuid, key: &str) {
    let role_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO roles (tenant_id, name) VALUES ($1, 'admin') RETURNING id",
    )
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .expect("insert role");
    let permission_id = sqlx::query_scalar::<_, Uuid>("SELECT id FROM permissions WHERE key = $1")
        .bind(key)
        .fetch_one(pool)
        .await
        .expect("lookup permission");
    sqlx::query("INSERT INTO role_permissions (role_id, permission_id) VALUES ($1, $2)")
        .bind(role_id)
        .bind(permission_id)
        .execute(pool)
        .await
        .expect("link role permission");
    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2)")
        .bind(user_id)
        .bind(role_id)
        .execute(pool)
        .await
        .expect("link user role");
}

async fn app(pool: PgPool) -> Router {
    let permission = Permission::SandboxCreate;
    Router::new()
        .route("/protected", get(|| async { "ok" }))
        .route_layer(middleware::from_fn_with_state(
            pool.clone(),
            move |State(pool): State<PgPool>, request: axum::extract::Request, next: Next| {
                async move { require_permission(pool, permission, request, next).await }
            },
        ))
        .with_state(pool)
}

async fn call_guarded(app: &Router, tenant_id: Uuid, user_id: Uuid) -> StatusCode {
    let request = Request::builder()
        .uri("/protected")
        .method("GET")
        .extension(AuthContext { tenant_id, user_id })
        .body(Body::empty())
        .expect("build request");
    let response = app.clone().oneshot(request).await.expect("call router");
    response.status()
}

#[tokio::test]
async fn permission_gate_denies_and_audits() {
    let admin = make_pool().await;
    common::run_migrations(&admin).await;

    let tenant_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO tenants (name) VALUES ('perm-gate') RETURNING id",
    )
    .fetch_one(&admin)
    .await
    .expect("insert tenant");
    let user_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO users (tenant_id, email) VALUES ($1, 'nogrant@example.com') RETURNING id",
    )
    .bind(tenant_id)
    .fetch_one(&admin)
    .await
    .expect("insert user");

    let app = app(admin.clone()).await;

    let denied = call_guarded(&app, tenant_id, user_id).await;
    assert_eq!(denied, StatusCode::FORBIDDEN);

    let denied_logged: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE event_type = 'permission_denied'")
            .fetch_one(&admin)
            .await
            .expect("count audit rows");
    assert!(denied_logged >= 1, "denial must be audit-logged");

    grant_permission(&admin, tenant_id, user_id, "sandbox:create").await;

    let allowed = call_guarded(&app, tenant_id, user_id).await;
    assert_eq!(allowed, StatusCode::OK);
}

#[tokio::test]
async fn permission_gate_unauthorized_without_context() {
    let admin = make_pool().await;
    common::run_migrations(&admin).await;

    let app = app(admin).await;
    let request = Request::builder()
        .uri("/protected")
        .method("GET")
        .body(Body::empty())
        .expect("build request without auth context");
    let response = app.clone().oneshot(request).await.expect("call router");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
