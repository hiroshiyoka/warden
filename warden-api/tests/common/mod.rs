use sqlx::migrate::Migrator;
use sqlx::PgPool;

const MIGRATION_LOCK_KEY: i64 = 642310845;

pub async fn run_migrations(pool: &PgPool) {
    let mut conn = pool.acquire().await.expect("acquire connection");
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(MIGRATION_LOCK_KEY)
        .execute(&mut *conn)
        .await
        .expect("acquire migration lock");
    let result = Migrator::new(std::path::Path::new("./migrations"))
        .await
        .expect("load migrations")
        .run(&mut *conn)
        .await;
    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(MIGRATION_LOCK_KEY)
        .execute(&mut *conn)
        .await
        .expect("release migration lock");
    result.expect("migrations failed");
}
