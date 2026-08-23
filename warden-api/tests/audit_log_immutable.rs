mod common;

use sqlx::{postgres::PgPoolOptions, PgPool};
use warden_api::audit::record_event;

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
async fn audit_log_is_append_only() {
    let admin = make_pool().await;
    common::run_migrations(&admin).await;

    // Superuser bypasses REVOKE, so tamper attempts must run as a role that only
    // holds INSERT/SELECT on audit_log. That is what proves REVOKE works.
    sqlx::query("DO $$ BEGIN IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'warden_audit_test') THEN CREATE ROLE warden_audit_test LOGIN; END IF; END $$")
        .execute(&admin)
        .await
        .expect("create audit test role");
    sqlx::query("GRANT USAGE ON SCHEMA public TO warden_audit_test")
        .execute(&admin)
        .await
        .expect("grant schema usage");
    sqlx::query("GRANT INSERT, SELECT ON audit_log TO warden_audit_test")
        .execute(&admin)
        .await
        .expect("grant insert/select on audit_log");

    record_event(
        &admin,
        None,
        None,
        "phase1_test",
        serde_json::json!({ "probe": "insert-works" }),
    )
    .await
    .expect("record_event should insert");

    let mut tx = admin.begin().await.expect("begin tx");
    sqlx::query("SET LOCAL ROLE warden_audit_test")
        .execute(&mut *tx)
        .await
        .expect("set role");

    // The mutation errors abort the transaction, hence savepoint + rollback.
    sqlx::query("SAVEPOINT before_update")
        .execute(&mut *tx)
        .await
        .expect("savepoint");
    let update = sqlx::query("UPDATE audit_log SET event_type = 'tampered'")
        .execute(&mut *tx)
        .await;
    sqlx::query("ROLLBACK TO SAVEPOINT before_update")
        .execute(&mut *tx)
        .await
        .expect("rollback to savepoint");
    assert!(update.is_err(), "UPDATE on audit_log must fail");

    sqlx::query("SAVEPOINT before_delete")
        .execute(&mut *tx)
        .await
        .expect("savepoint");
    let delete = sqlx::query("DELETE FROM audit_log").execute(&mut *tx).await;
    sqlx::query("ROLLBACK TO SAVEPOINT before_delete")
        .execute(&mut *tx)
        .await
        .expect("rollback to savepoint");
    assert!(delete.is_err(), "DELETE on audit_log must fail");

    tx.rollback().await.expect("rollback tx");
}
