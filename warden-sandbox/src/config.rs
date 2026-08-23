#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub vcpu_count: u8,
    pub memory_mib: u32,
    pub exec_timeout: std::time::Duration,
}
