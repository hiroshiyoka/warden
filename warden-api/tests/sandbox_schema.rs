mod common;

use sqlx::{postgres::PgPoolOptions, PgPool};

async fn make_pool() -> PgPool {
    let url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");
    PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("failed to connect to postgres")
}

#[tokio::test]
async fn sandbox_schema_round_trips() {
    let pool = make_pool().await;
    common::run_migrations(&pool).await;

    let mut tx = pool.begin().await.expect("begin tx");

    let tenant_id: uuid::Uuid =
        sqlx::query_scalar("INSERT INTO tenants (name) VALUES ('sandbox-schema-test') RETURNING id")
            .fetch_one(&mut *tx)
            .await
            .expect("insert tenant");

    let sandbox_id: uuid::Uuid =
        sqlx::query_scalar("INSERT INTO sandboxes (tenant_id) VALUES ($1) RETURNING id")
            .bind(tenant_id)
            .fetch_one(&mut *tx)
            .await
            .expect("insert sandbox");

    let status: String = sqlx::query_scalar("SELECT status FROM sandboxes WHERE id = $1")
        .bind(sandbox_id)
        .fetch_one(&mut *tx)
        .await
        .expect("read default status");
    assert_eq!(status, "pending_runtime");

    sqlx::query(
        "INSERT INTO egress_rules (sandbox_id, destination_cidr, destination_port)
         VALUES ($1, '10.0.0.0/24', 443)",
    )
    .bind(sandbox_id)
    .execute(&mut *tx)
    .await
    .expect("insert egress rule");

    let rule_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM egress_rules WHERE sandbox_id = $1")
            .bind(sandbox_id)
            .fetch_one(&mut *tx)
            .await
            .expect("count egress rules");
    assert_eq!(rule_count, 1);

    sqlx::query("DELETE FROM sandboxes WHERE id = $1")
        .bind(sandbox_id)
        .execute(&mut *tx)
        .await
        .expect("delete sandbox");

    let remaining: i64 =
        sqlx::query_scalar("SELECT count(*) FROM egress_rules WHERE sandbox_id = $1")
            .bind(sandbox_id)
            .fetch_one(&mut *tx)
            .await
            .expect("count egress rules after cascade");
    assert_eq!(remaining, 0);

    tx.rollback().await.expect("rollback tx");
}
