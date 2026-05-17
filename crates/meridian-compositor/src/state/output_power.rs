use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputPowerMode {
    #[default]
    On,
    Off,
}

/// Per-output power-state bookkeeping. Default On (= no entry
/// in the map equals On). 3a has no DRM side-effects.
#[derive(Debug, Default)]
pub struct OutputPowerManager {
    modes: HashMap<String, OutputPowerMode>,
}

impl OutputPowerManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mode_for(&self, output_name: &str) -> OutputPowerMode {
        self.modes.get(output_name).copied().unwrap_or_default()
    }

    /// Returns true iff this changed the stored mode for this output.
    pub fn set_mode(&mut self, output_name: &str, mode: OutputPowerMode) -> bool {
        let prev = self.mode_for(output_name);
        if prev == mode {
            return false;
        }
        self.modes.insert(output_name.to_string(), mode);
        true
    }

    /// Drop bookkeeping for a vanished output. Returns the mode
    /// that was stored (or default if none).
    pub fn forget(&mut self, output_name: &str) -> OutputPowerMode {
        self.modes.remove(output_name).unwrap_or_default()
    }

    pub fn known_count(&self) -> usize {
        self.modes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{OutputPowerManager, OutputPowerMode};

    #[test]
    fn default_mode_for_unknown_output_is_on() {
        let manager = OutputPowerManager::new();
        assert_eq!(manager.mode_for("unknown"), OutputPowerMode::On);
        assert_eq!(manager.known_count(), 0);
    }

    #[test]
    fn set_mode_off_returns_true_first_time() {
        let mut manager = OutputPowerManager::new();
        assert!(manager.set_mode("out-1", OutputPowerMode::Off));
        assert_eq!(manager.mode_for("out-1"), OutputPowerMode::Off);
        assert_eq!(manager.known_count(), 1);
    }

    #[test]
    fn set_mode_same_returns_false() {
        let mut manager = OutputPowerManager::new();
        assert!(manager.set_mode("out-1", OutputPowerMode::Off));
        assert!(!manager.set_mode("out-1", OutputPowerMode::Off));
        assert_eq!(manager.mode_for("out-1"), OutputPowerMode::Off);
    }

    #[test]
    fn set_mode_on_after_off_returns_true() {
        let mut manager = OutputPowerManager::new();
        assert!(manager.set_mode("out-1", OutputPowerMode::Off));
        assert!(manager.set_mode("out-1", OutputPowerMode::On));
        assert_eq!(manager.mode_for("out-1"), OutputPowerMode::On);
    }

    #[test]
    fn forget_removes_mode_and_returns_previous() {
        let mut manager = OutputPowerManager::new();
        assert!(manager.set_mode("out-1", OutputPowerMode::Off));
        assert_eq!(manager.forget("out-1"), OutputPowerMode::Off);
        assert_eq!(manager.mode_for("out-1"), OutputPowerMode::On);
        assert_eq!(manager.known_count(), 0);
    }

    #[test]
    fn forget_unknown_returns_default_on() {
        let mut manager = OutputPowerManager::new();
        assert_eq!(manager.forget("unknown"), OutputPowerMode::On);
        assert_eq!(manager.known_count(), 0);
    }
}
