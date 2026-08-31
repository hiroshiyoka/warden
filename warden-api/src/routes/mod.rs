mod health;
mod sandboxes;
mod users;

use crate::{
    auth::{middleware::require_permission, permission::Permission},
    AppState,
};
use axum::{
    extract::State,
    middleware,
    routing::{get, post},
    Router,
};
use sqlx::PgPool;

pub fn router(pool: PgPool) -> Router {
    router_with_state(AppState::new(pool))
}

pub fn router_with_state(state: AppState) -> Router {
    let invite_permission = Permission::UserInvite;
    let invite = Router::new()
        .route("/users/invite", post(users::invite))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            move |State(state): State<AppState>, request: axum::extract::Request, next: axum::middleware::Next| {
                async move { require_permission(state.pool, invite_permission, request, next).await }
            },
        ));

    let create_permission = Permission::SandboxCreate;
    let create = Router::new()
        .route("/sandboxes", post(sandboxes::create).get(sandboxes::list))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            move |State(state): State<AppState>, request: axum::extract::Request, next: axum::middleware::Next| {
                async move { require_permission(state.pool, create_permission, request, next).await }
            },
        ));
    let exec_permission = Permission::SandboxExec;
    let exec = Router::new()
        .route("/sandboxes/:id/exec", post(sandboxes::exec))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            move |State(state): State<AppState>,
                  request: axum::extract::Request,
                  next: axum::middleware::Next| {
                async move { require_permission(state.pool, exec_permission, request, next).await }
            },
        ));
    let destroy_permission = Permission::SandboxDestroy;
    let destroy =
        Router::new()
            .route("/sandboxes/:id", axum::routing::delete(sandboxes::destroy))
            .route_layer(middleware::from_fn_with_state(
                state.clone(),
                move |State(state): State<AppState>,
                      request: axum::extract::Request,
                      next: axum::middleware::Next| {
                    async move {
                        require_permission(state.pool, destroy_permission, request, next).await
                    }
                },
            ));

    Router::new()
        .route("/health", get(health::health))
        .merge(invite)
        .merge(create)
        .merge(exec)
        .merge(destroy)
        .with_state(state)
}
