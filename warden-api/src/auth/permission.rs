#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    SandboxCreate,
    SandboxDestroy,
    SandboxExec,
    UserInvite,
    UserRemove,
}

impl Permission {
    pub fn key(&self) -> &'static str {
        match self {
            Permission::SandboxCreate => "sandbox:create",
            Permission::SandboxDestroy => "sandbox:destroy",
            Permission::SandboxExec => "sandbox:exec",
            Permission::UserInvite => "user:invite",
            Permission::UserRemove => "user:remove",
        }
    }
}