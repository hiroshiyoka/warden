mod common;

use sqlx::{postgres::PgPoolOptions, PgPool};

async fn make_pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set for integration tests");
    PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("failed to connect to postgres")
}

#[tokio::test]
async fn rbac_schema_round_trips() {
    let pool = make_pool().await;
    common::run_migrations(&pool).await;

    let mut tx = pool.begin().await.expect("begin tx");

    let tenant_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO tenants (name) VALUES ('rbac-test') RETURNING id",
    )
    .fetch_one(&mut *tx)
    .await
    .expect("insert tenant");

    let role_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO roles (tenant_id, name) VALUES ($1, 'admin') RETURNING id",
    )
    .bind(tenant_id)
    .fetch_one(&mut *tx)
    .await
    .expect("insert role");

    let perm_id: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM permissions WHERE key = 'sandbox:create'")
            .fetch_one(&mut *tx)
            .await
            .expect("lookup sandbox:create permission");

    sqlx::query("INSERT INTO role_permissions (role_id, permission_id) VALUES ($1, $2)")
        .bind(role_id)
        .bind(perm_id)
        .execute(&mut *tx)
        .await
        .expect("attach permission to role");

    let user_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO users (tenant_id, email) VALUES ($1, 'rbac@example.com') RETURNING id",
    )
    .bind(tenant_id)
    .fetch_one(&mut *tx)
    .await
    .expect("insert user");

    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2)")
        .bind(user_id)
        .bind(role_id)
        .execute(&mut *tx)
        .await
        .expect("assign role to user");

    let keys: Vec<String> = sqlx::query_scalar(
        "SELECT p.key
         FROM user_roles ur
         JOIN role_permissions rp ON rp.role_id = ur.role_id
         JOIN permissions p ON p.id = rp.permission_id
         WHERE ur.user_id = $1
         ORDER BY p.key",
    )
    .bind(user_id)
    .fetch_all(&mut *tx)
    .await
    .expect("query permissions through joins");

    assert_eq!(keys, vec!["sandbox:create".to_string()]);

    tx.rollback().await.expect("rollback tx");
}