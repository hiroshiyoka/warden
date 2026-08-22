mod common;

use axum::{body::Body, http::{Request, StatusCode}, Router};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, PgPool};
use tower::ServiceExt as _;
use uuid::Uuid;
use warden_api::{auth::AuthContext, routes};

async fn make_pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set for integration tests");
    PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("failed to connect to postgres")
}

async fn grant_invite(pool: &PgPool, tenant_id: Uuid, user_id: Uuid) {
    let role_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO roles (tenant_id, name) VALUES ($1, 'inviter') RETURNING id",
    )
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .expect("insert inviter role");
    let permission_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM permissions WHERE key = 'user:invite'",
    )
    .fetch_one(pool)
    .await
    .expect("lookup invite permission");
    sqlx::query("INSERT INTO role_permissions (role_id, permission_id) VALUES ($1, $2)")
        .bind(role_id)
        .bind(permission_id)
        .execute(pool)
        .await
        .expect("link invite permission");
    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2)")
        .bind(user_id)
        .bind(role_id)
        .execute(pool)
        .await
        .expect("link inviter role");
}

fn invite_request(auth: Option<AuthContext>, email: &str) -> Request<Body> {
    let mut request = Request::builder()
        .uri("/users/invite")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(json!({ "email": email }).to_string()))
        .expect("build invite request");
    if let Some(auth) = auth {
        request.extensions_mut().insert(auth);
    }
    request
}

#[tokio::test]
async fn invite_requires_permission_and_is_tenant_scoped() {
    let pool = make_pool().await;
    common::run_migrations(&pool).await;
    let tenant_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO tenants (name) VALUES ('invite-test') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("insert tenant");
    let user_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO users (tenant_id, email) VALUES ($1, 'inviter@example.com') RETURNING id",
    )
    .bind(tenant_id)
    .fetch_one(&pool)
    .await
    .expect("insert inviter");
    let auth = AuthContext { tenant_id, user_id };
    let app: Router = routes::router(pool.clone());
    let invited_email = format!("new-{}@example.com", Uuid::new_v4());

    let denied = app.clone()
        .oneshot(invite_request(Some(auth), &format!("denied-{}@example.com", Uuid::new_v4())))
        .await
        .expect("call denied invite");
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);

    grant_invite(&pool, tenant_id, user_id).await;
    let allowed = app
        .oneshot(invite_request(Some(auth), &invited_email))
        .await
        .expect("call allowed invite");
    assert_eq!(allowed.status(), StatusCode::CREATED);

    let invited_tenant: Uuid = sqlx::query_scalar(
        "SELECT tenant_id FROM users WHERE email = $1",
    )
    .bind(&invited_email)
    .fetch_one(&pool)
    .await
    .expect("find invited user");
    assert_eq!(invited_tenant, tenant_id);
}

#[tokio::test]
async fn invite_requires_auth_context() {
    let pool = make_pool().await;
    common::run_migrations(&pool).await;
    let response = routes::router(pool)
        .oneshot(invite_request(None, "no-auth@example.com"))
        .await
        .expect("call unauthenticated invite");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
