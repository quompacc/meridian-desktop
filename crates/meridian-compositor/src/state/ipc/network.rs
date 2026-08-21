pub(super) fn valid_connect_request(ssid: &str, password: Option<&str>) -> bool {
    !ssid.is_empty()
        && ssid.len() <= 128
        && !ssid.chars().any(char::is_control)
        && password.is_none_or(|password| {
            !password.is_empty() && password.len() <= 256 && !password.chars().any(char::is_control)
        })
}

#[cfg(test)]
mod tests {
    use super::valid_connect_request;

    #[test]
    fn connect_request_rejects_empty_oversized_and_control_values() {
        assert!(valid_connect_request("Meridian", None));
        assert!(valid_connect_request("Meridian", Some("secret phrase")));
        assert!(!valid_connect_request("", None));
        assert!(!valid_connect_request("bad\nssid", None));
        assert!(!valid_connect_request("Meridian", Some("")));
        assert!(!valid_connect_request("Meridian", Some("bad\npassword")));
        assert!(!valid_connect_request(&"x".repeat(129), None));
    }
}
