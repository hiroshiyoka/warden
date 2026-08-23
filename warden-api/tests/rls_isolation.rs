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
async fn rls_isolates_users_per_tenant() {
    let admin = make_pool().await;
    common::run_migrations(&admin).await;

    sqlx::query("DO $$ BEGIN IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'warden_rls_test') THEN CREATE ROLE warden_rls_test LOGIN; END IF; END $$")
        .execute(&admin)
        .await
        .expect("create rls test role");
    sqlx::query("GRANT USAGE ON SCHEMA public TO warden_rls_test")
        .execute(&admin)
        .await
        .expect("grant schema usage");
    sqlx::query("GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO warden_rls_test")
        .execute(&admin)
        .await
        .expect("grant table privileges");

    let mut tx = admin.begin().await.expect("begin tx");

    sqlx::query("SET LOCAL ROLE warden_rls_test")
        .execute(&mut *tx)
        .await
        .expect("set role");

    let tenant_a = sqlx::query_scalar::<_, uuid::Uuid>(
        "INSERT INTO tenants (name) VALUES ('tenant-a') RETURNING id",
    )
    .fetch_one(&mut *tx)
    .await
    .expect("insert tenant a");

    let tenant_b = sqlx::query_scalar::<_, uuid::Uuid>(
        "INSERT INTO tenants (name) VALUES ('tenant-b') RETURNING id",
    )
    .fetch_one(&mut *tx)
    .await
    .expect("insert tenant b");

    // No tenant context set: the policy calls current_setting('app.current_tenant_id'),
    // which errors when unset, so the query must fail closed. Wrapped in a savepoint
    // because the error aborts the transaction.
    sqlx::query("SAVEPOINT before_no_context")
        .execute(&mut *tx)
        .await
        .expect("savepoint");
    let no_context = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM users")
        .fetch_one(&mut *tx)
        .await;
    sqlx::query("ROLLBACK TO SAVEPOINT before_no_context")
        .execute(&mut *tx)
        .await
        .expect("rollback to savepoint");
    assert!(
        no_context.is_err(),
        "queries without a tenant context must fail"
    );

    sqlx::query("SELECT set_config('app.current_tenant_id', $1, true)")
        .bind(tenant_a.to_string())
        .execute(&mut *tx)
        .await
        .expect("set tenant a context");

    sqlx::query("INSERT INTO users (tenant_id, email) VALUES ($1, 'a@example.com')")
        .bind(tenant_a)
        .execute(&mut *tx)
        .await
        .expect("insert user a");

    let visible_a: Vec<(uuid::Uuid, String)> = sqlx::query_as("SELECT id, email FROM users")
        .fetch_all(&mut *tx)
        .await
        .expect("query as tenant a");
    assert_eq!(visible_a.len(), 1, "tenant a must see exactly its own user");
    assert_eq!(visible_a[0].1, "a@example.com");

    sqlx::query("SAVEPOINT before_cross_insert")
        .execute(&mut *tx)
        .await
        .expect("savepoint");
    let cross_insert =
        sqlx::query("INSERT INTO users (tenant_id, email) VALUES ($1, 'x@example.com')")
            .bind(tenant_b)
            .execute(&mut *tx)
            .await;
    sqlx::query("ROLLBACK TO SAVEPOINT before_cross_insert")
        .execute(&mut *tx)
        .await
        .expect("rollback to savepoint");
    assert!(
        cross_insert.is_err(),
        "tenant a must not be able to insert tenant b data"
    );

    sqlx::query("SELECT set_config('app.current_tenant_id', $1, true)")
        .bind(tenant_b.to_string())
        .execute(&mut *tx)
        .await
        .expect("set tenant b context");

    sqlx::query("INSERT INTO users (tenant_id, email) VALUES ($1, 'b@example.com')")
        .bind(tenant_b)
        .execute(&mut *tx)
        .await
        .expect("insert user b");

    let visible_b: Vec<(uuid::Uuid, String)> = sqlx::query_as("SELECT id, email FROM users")
        .fetch_all(&mut *tx)
        .await
        .expect("query as tenant b");
    assert_eq!(visible_b.len(), 1, "tenant b must see exactly its own user");
    assert_eq!(visible_b[0].1, "b@example.com");

    let cross_read: Vec<(uuid::Uuid, String)> =
        sqlx::query_as("SELECT id, email FROM users WHERE tenant_id = $1")
            .bind(tenant_a)
            .fetch_all(&mut *tx)
            .await
            .expect("query tenant a rows as tenant b");
    assert!(
        cross_read.is_empty(),
        "tenant b must not be able to read tenant a rows"
    );

    tx.rollback().await.expect("rollback tx");
}
