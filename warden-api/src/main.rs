mod auth;
mod db;
mod error;
mod routes;

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let pool = db::connect().await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    let app = routes::router(pool);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    tracing::info!("warden-api listening on :8080");
    axum::serve(listener, app).await?;

    Ok(())
}
