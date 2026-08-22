use serde::{Deserialize, Serialize};

use crate::AppearanceSnapshot;

const MAX_SYSTEM_VALUE_BYTES: usize = 512;

/// Read-only state shown by the unprivileged System Settings document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsSnapshot {
    pub appearance: AppearanceSnapshot,
    pub system: SystemSettingsSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemSettingsSnapshot {
    pub os_name: String,
    pub hostname: String,
    pub kernel: String,
    pub uptime: String,
    pub cpu: String,
    pub memory: String,
}

impl SettingsSnapshot {
    pub fn validate(&self) -> Result<(), &'static str> {
        self.appearance.validate()?;
        for value in [
            &self.system.os_name,
            &self.system.hostname,
            &self.system.kernel,
            &self.system.uptime,
            &self.system.cpu,
            &self.system.memory,
        ] {
            if value.is_empty()
                || value.len() > MAX_SYSTEM_VALUE_BYTES
                || value.chars().any(char::is_control)
            {
                return Err("invalid system information display value");
            }
        }
        Ok(())
    }
}
