mod health;
mod users;

use crate::auth::{middleware::require_permission, permission::Permission};
use axum::{extract::State, middleware, routing::{get, post}, Router};
use sqlx::PgPool;

pub fn router(pool: PgPool) -> Router {
    let invite_permission = Permission::UserInvite;
    let invite = Router::new()
        .route("/users/invite", post(users::invite))
        .route_layer(middleware::from_fn_with_state(
            pool.clone(),
            move |State(pool): State<PgPool>, request: axum::extract::Request, next: axum::middleware::Next| {
                async move { require_permission(pool, invite_permission, request, next).await }
            },
        ));

    Router::new()
        .route("/health", get(health::health))
        .merge(invite)
        .with_state(pool)
}
