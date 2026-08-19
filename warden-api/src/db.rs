use sqlx::{postgres::PgPoolOptions, PgPool};

pub async fn connect() -> anyhow::Result<PgPool> {
    let url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set — never hardcode a fallback connection string");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&url)
        .await?;
    Ok(pool)
}

// TODO(phase-1): set app.current_tenant_id per request
