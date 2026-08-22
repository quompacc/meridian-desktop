pub(super) fn canonical_quick_settings_state(raw: &str) -> Result<String, String> {
    let snapshot = serde_json::from_str::<meridian_ipc::QuickSettingsSnapshot>(raw)
        .map_err(|error| format!("invalid schema: {error}"))?;
    snapshot.validate().map_err(str::to_string)?;
    serde_json::to_string(&snapshot).map_err(|error| format!("cannot encode state: {error}"))
}

pub(super) fn canonical_appearance_state(raw: &str) -> Result<String, String> {
    let snapshot = serde_json::from_str::<meridian_ipc::AppearanceSnapshot>(raw)
        .map_err(|error| format!("invalid schema: {error}"))?;
    snapshot.validate().map_err(str::to_string)?;
    serde_json::to_string(&snapshot).map_err(|error| format!("cannot encode state: {error}"))
}

pub(super) fn canonical_settings_state(raw: &str) -> Result<String, String> {
    let snapshot = serde_json::from_str::<meridian_ipc::SettingsSnapshot>(raw)
        .map_err(|error| format!("invalid schema: {error}"))?;
    snapshot.validate().map_err(str::to_string)?;
    serde_json::to_string(&snapshot).map_err(|error| format!("cannot encode state: {error}"))
}

pub(super) fn canonical_launcher_state<'a>(
    command: &str,
    raw: &'a str,
) -> Result<(&'static str, String), String> {
    if matches!(command, "show-settings" | "settings") {
        canonical_settings_state(raw).map(|state| ("applySettingsState", state))
    } else {
        canonical_appearance_state(raw).map(|state| ("applyAppearanceState", state))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_appearance_state, canonical_quick_settings_state, canonical_settings_state,
    };

    #[test]
    fn quick_settings_state_is_schema_checked_before_javascript() {
        let valid = r#"{"network":{"available":true,"connected":false,"kind":null,"name":null,"signal_percent":null,"wifi_networks":[]},"audio":{"available":true,"output_name":"Audio","volume_percent":62,"muted":false},"battery":{"present":false,"capacity":0,"charging":false,"on_ac":true},"power_profile":null}"#;
        assert!(canonical_quick_settings_state(valid).is_ok());
        assert!(canonical_quick_settings_state("{\"audio\":{}}").is_err());
        assert!(canonical_quick_settings_state(&valid.replace("62", "162")).is_err());
    }

    #[test]
    fn appearance_state_is_schema_checked_before_javascript() {
        let valid = r#"{"theme":"light","wallpaper_name":"Berge.png","wallpaper_mode":"fill"}"#;
        assert!(canonical_appearance_state(valid).is_ok());
        assert!(canonical_appearance_state(&valid.replace("Berge.png", "bad\\nname")).is_err());
        assert!(canonical_appearance_state(&valid.replace("light", "blue")).is_err());
    }

    #[test]
    fn settings_state_is_schema_checked_before_javascript() {
        let valid = r#"{"appearance":{"theme":"dark","wallpaper_name":null,"wallpaper_mode":"fill"},"system":{"os_name":"OpenBSD 7.8","hostname":"meridian","kernel":"OpenBSD 7.8","uptime":"1h","cpu":"Intel","memory":"16 GiB"}}"#;
        assert!(canonical_settings_state(valid).is_ok());
        assert!(canonical_settings_state(&valid.replace("meridian", "bad\\nhost")).is_err());
    }
}
