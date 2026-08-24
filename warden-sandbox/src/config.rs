use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub vcpu_count: u8,
    pub memory_mib: u32,
    pub exec_timeout: std::time::Duration,
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("vcpu_count must be between 1 and 4, got {0}")]
    InvalidVcpuCount(u8),
    #[error("memory_mib must be between 64 and 2048, got {0}")]
    InvalidMemory(u32),
    #[error("exec_timeout must be between 1s and 300s, got {0:?}")]
    InvalidTimeout(std::time::Duration),
}

impl ResourceLimits {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if !(1..=4).contains(&self.vcpu_count) {
            return Err(ConfigError::InvalidVcpuCount(self.vcpu_count));
        }
        if !(64..=2048).contains(&self.memory_mib) {
            return Err(ConfigError::InvalidMemory(self.memory_mib));
        }
        if self.exec_timeout.as_secs() == 0 || self.exec_timeout.as_secs() > 300 {
            return Err(ConfigError::InvalidTimeout(self.exec_timeout));
        }
        Ok(())
    }
}

// Verifies a guest kernel/rootfs image file exists and matches a recorded
// checksum. Getting this wrong is the kind of bug you don't want to discover
// for the first time while debugging a live Firecracker boot in Phase 2B.
pub fn verify_image_checksum(path: &std::path::Path, expected_sha256: &str) -> Result<(), std::io::Error> {
    use sha2::{Sha256, Digest};
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let actual = format!("{:x}", hasher.finalize());
    if actual != expected_sha256 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("checksum mismatch: expected {expected_sha256}, got {actual}"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::Duration;

    fn valid_limits() -> ResourceLimits {
        ResourceLimits {
            vcpu_count: 2,
            memory_mib: 512,
            exec_timeout: Duration::from_secs(30),
        }
    }

    #[test]
    fn valid_limits_pass() {
        assert!(valid_limits().validate().is_ok());
    }

    #[test]
    fn vcpu_zero_and_five_rejected() {
        let mut l = valid_limits();
        l.vcpu_count = 0;
        assert!(matches!(l.validate(), Err(ConfigError::InvalidVcpuCount(0))));
        l.vcpu_count = 5;
        assert!(matches!(l.validate(), Err(ConfigError::InvalidVcpuCount(5))));
    }

    #[test]
    fn memory_out_of_range_rejected() {
        let mut l = valid_limits();
        l.memory_mib = 63;
        assert!(matches!(l.validate(), Err(ConfigError::InvalidMemory(63))));
        l.memory_mib = 2049;
        assert!(matches!(l.validate(), Err(ConfigError::InvalidMemory(2049))));
    }

    #[test]
    fn timeout_zero_and_over_300_rejected() {
        let mut l = valid_limits();
        l.exec_timeout = Duration::from_secs(0);
        assert!(matches!(l.validate(), Err(ConfigError::InvalidTimeout(_))));
        l.exec_timeout = Duration::from_secs(301);
        assert!(matches!(l.validate(), Err(ConfigError::InvalidTimeout(_))));
    }

    #[test]
    fn checksum_match_passes() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("warden_cfgtest_{}.bin", uuid_like()));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"warden-image").unwrap();
        drop(f);

        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(b"warden-image");
        let sum = format!("{:x}", hasher.finalize());

        let result = verify_image_checksum(&path, &sum);
        let _ = std::fs::remove_file(&path);
        assert!(result.is_ok());
    }

    #[test]
    fn checksum_mismatch_fails() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("warden_cfgtest_{}.bin", uuid_like()));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"warden-image").unwrap();
        drop(f);

        let result = verify_image_checksum(&path, "deadbeef");
        let _ = std::fs::remove_file(&path);
        assert!(result.is_err());
    }

    fn uuid_like() -> String {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("{n}")
    }
}
