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

    /// Compute how many outputs would be in mode On after applying
    /// `new_mode` to `candidate`, given the currently known output names.
    pub fn projected_on_count(
        &self,
        known_outputs: &[String],
        candidate: &str,
        new_mode: OutputPowerMode,
    ) -> usize {
        known_outputs
            .iter()
            .filter(|name| {
                if name.as_str() == candidate {
                    matches!(new_mode, OutputPowerMode::On)
                } else {
                    matches!(self.mode_for(name), OutputPowerMode::On)
                }
            })
            .count()
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

    #[test]
    fn projected_on_count_no_change_keeps_count() {
        let manager = OutputPowerManager::new();
        let outputs = vec!["o1".to_string(), "o2".to_string(), "o3".to_string()];
        assert_eq!(
            manager.projected_on_count(&outputs, "o1", OutputPowerMode::On),
            3
        );
    }

    #[test]
    fn projected_on_count_turning_off_one_of_many() {
        let manager = OutputPowerManager::new();
        let outputs = vec!["o1".to_string(), "o2".to_string(), "o3".to_string()];
        assert_eq!(
            manager.projected_on_count(&outputs, "o2", OutputPowerMode::Off),
            2
        );
    }

    #[test]
    fn projected_on_count_turning_off_last_returns_zero() {
        let manager = OutputPowerManager::new();
        let outputs = vec!["o1".to_string()];
        assert_eq!(
            manager.projected_on_count(&outputs, "o1", OutputPowerMode::Off),
            0
        );
    }

    #[test]
    fn projected_on_count_turning_on_already_off() {
        let mut manager = OutputPowerManager::new();
        assert!(manager.set_mode("o1", OutputPowerMode::Off));
        let outputs = vec!["o1".to_string(), "o2".to_string(), "o3".to_string()];
        assert_eq!(
            manager.projected_on_count(&outputs, "o1", OutputPowerMode::On),
            3
        );
    }

    #[test]
    fn safety_net_rejects_last_on_off_via_projected_count() {
        let manager = OutputPowerManager::new();
        let outputs = vec!["solo".to_string()];
        assert_eq!(
            manager.projected_on_count(&outputs, "solo", OutputPowerMode::Off),
            0
        );
    }

    #[test]
    fn safety_net_allows_off_when_other_on_exists() {
        let manager = OutputPowerManager::new();
        let outputs = vec!["a".to_string(), "b".to_string()];
        assert_eq!(
            manager.projected_on_count(&outputs, "a", OutputPowerMode::Off),
            1
        );
    }

    #[test]
    fn safety_net_allows_on_anytime() {
        let mut manager = OutputPowerManager::new();
        assert!(manager.set_mode("a", OutputPowerMode::Off));
        let outputs = vec!["a".to_string()];
        assert_eq!(
            manager.projected_on_count(&outputs, "a", OutputPowerMode::On),
            1
        );
    }

    #[test]
    fn cycle_off_on_off_with_multiple_outputs_consistent() {
        let mut manager = OutputPowerManager::new();
        let outputs = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert!(manager.set_mode("a", OutputPowerMode::Off));
        assert_eq!(
            manager.projected_on_count(&outputs, "a", OutputPowerMode::On),
            3
        );
        assert!(manager.set_mode("a", OutputPowerMode::On));
        assert_eq!(
            manager.projected_on_count(&outputs, "b", OutputPowerMode::Off),
            2
        );
    }
}
