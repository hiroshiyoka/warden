use async_trait::async_trait;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error("sandbox runtime is not yet available on this deployment")]
    NotImplemented,
    #[error("resource limit exceeded: {0}")]
    LimitExceeded(String),
}

#[derive(Debug)]
pub struct ExecResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

#[async_trait]
pub trait SandboxRuntime: Send + Sync {
    async fn boot(
        &self,
        sandbox_id: uuid::Uuid,
        limits: &crate::config::ResourceLimits,
    ) -> Result<(), RuntimeError>;
    async fn exec(
        &self,
        sandbox_id: uuid::Uuid,
        command: &str,
        timeout: std::time::Duration,
    ) -> Result<ExecResult, RuntimeError>;
    async fn destroy(&self, sandbox_id: uuid::Uuid) -> Result<(), RuntimeError>;
}

// The only implementation in Phase 2A. Every method fails honestly; Phase 2B
// replaces this struct wholesale with FirecrackerRuntime.
pub struct UnimplementedRuntime;

#[async_trait]
impl SandboxRuntime for UnimplementedRuntime {
    async fn boot(
        &self,
        _sandbox_id: uuid::Uuid,
        _limits: &crate::config::ResourceLimits,
    ) -> Result<(), RuntimeError> {
        Err(RuntimeError::NotImplemented)
    }

    async fn exec(
        &self,
        _sandbox_id: uuid::Uuid,
        _command: &str,
        _timeout: std::time::Duration,
    ) -> Result<ExecResult, RuntimeError> {
        Err(RuntimeError::NotImplemented)
    }

    async fn destroy(&self, _sandbox_id: uuid::Uuid) -> Result<(), RuntimeError> {
        Err(RuntimeError::NotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn limits() -> crate::config::ResourceLimits {
        crate::config::ResourceLimits {
            vcpu_count: 1,
            memory_mib: 512,
            exec_timeout: Duration::from_secs(30),
        }
    }

    #[tokio::test]
    async fn unimplemented_runtime_never_succeeds() {
        let rt = UnimplementedRuntime;
        let id = uuid::Uuid::new_v4();

        assert!(matches!(
            rt.boot(id, &limits()).await,
            Err(RuntimeError::NotImplemented)
        ));
        assert!(matches!(
            rt.exec(id, "ls", Duration::from_secs(1)).await,
            Err(RuntimeError::NotImplemented)
        ));
        assert!(matches!(
            rt.destroy(id).await,
            Err(RuntimeError::NotImplemented)
        ));
    }
}
