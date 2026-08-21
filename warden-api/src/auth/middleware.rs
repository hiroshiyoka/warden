use crate::{
    audit,
    auth::{permission::Permission, AuthContext},
};
use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
use sqlx::PgPool;

pub async fn require_permission(
    pool: PgPool,
    permission: Permission,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let (parts, body) = request.into_parts();
    let auth = parts
        .extensions
        .get::<AuthContext>()
        .copied()
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let request = Request::from_parts(parts, body);

    let has_permission: bool = sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM user_roles ur
            JOIN role_permissions rp ON rp.role_id = ur.role_id
            JOIN permissions p ON p.id = rp.permission_id
            WHERE ur.user_id = $1 AND p.key = $2
         )",
    )
    .bind(auth.user_id)
    .bind(permission.key())
    .fetch_one(&pool)
    .await
    // Database errors become false, never an allow.
    .unwrap_or(false);

    if !has_permission {
        let _ = audit::record_event(
            &pool,
            Some(auth.tenant_id),
            Some(auth.user_id),
            "permission_denied",
            serde_json::json!({ "permission": permission.key() }),
        )
        .await;
        return Err(StatusCode::FORBIDDEN);
    }

    Ok(next.run(request).await)
}
