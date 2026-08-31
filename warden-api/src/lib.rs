pub mod audit;
pub mod auth;
pub mod db;
pub mod error;
pub mod routes;

use axum::extract::FromRef;
use sqlx::PgPool;
use std::sync::Arc;
use warden_sandbox::{runtime::SandboxRuntime, runtime::UnimplementedRuntime};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub runtime: Arc<dyn SandboxRuntime>,
}

impl AppState {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            runtime: Arc::new(UnimplementedRuntime),
        }
    }
}

impl FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}
