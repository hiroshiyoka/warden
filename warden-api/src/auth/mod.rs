pub mod middleware;
pub mod permission;

use uuid::Uuid;

/// Authenticated identity attached to a request by the auth layer. The
/// permission middleware reads this to know whose roles to check; nothing is
/// allowed through when it is absent. Carried as a request `Extension`.
#[derive(Debug, Clone, Copy)]
pub struct AuthContext {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
}
