use crate::auth::permission::Permission;
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};

/// Fail-closed by construction: the caller names the exact permission being
/// checked; there is no variant that lets a request through without one.
pub async fn require_permission(
    permission: Permission,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // TODO(task-5): look up the authenticated user's roles/permissions from DB
    // and deny when absent. Pass-through for now so the middleware compiles
    // before the audit log (Task 4) exists. The real check fails closed on any
    // lookup error, so denials can be logged from the moment they're enforced.
    let _ = permission;
    Ok(next.run(request).await)
}