impl LoginUiState {
    fn apply(&mut self, action: KeyAction) -> ControlFlow {
        self.pending_power = None;
        match action {
            KeyAction::Insert(s) => {
                let target: &mut String =
                    if self.smartcard_login_ready() || self.focus == Field::Password {
                        &mut self.password
                    } else {
                        &mut self.username
                    };
                if target.chars().count() + s.chars().count() <= MAX_FIELD_LEN {
                    target.push_str(&s);
                }
                ControlFlow::Continue
            }
            KeyAction::Backspace => {
                if self.smartcard_login_ready() || self.focus == Field::Password {
                    self.password.pop();
                } else {
                    self.username.pop();
                }
                ControlFlow::Continue
            }
            KeyAction::CycleFocus => {
                self.focus = if self.smartcard_login_ready() {
                    Field::Password
                } else {
                    match self.focus {
                        Field::Username => Field::Password,
                        Field::Password => Field::Username,
                    }
                };
                ControlFlow::Continue
            }
            KeyAction::CycleFocusBack => {
                self.focus = if self.smartcard_login_ready() {
                    Field::Password
                } else {
                    match self.focus {
                        Field::Username => Field::Password,
                        Field::Password => Field::Username,
                    }
                };
                ControlFlow::Continue
            }
            KeyAction::Submit => ControlFlow::Submit,
            KeyAction::Cancel => ControlFlow::Cancel,
        }
    }

    /// Spawn a worker that runs the full PAM lifecycle (authenticate →
    /// open_session → wait → close on signal) so the render loop stays
    /// responsive. The worker owns the credentials (Zeroizing) and the
    /// pam::Client; we just hold the result channel + AuthDriver here.
    fn start_auth(&mut self) {
        let Some(username) = self.auth_username() else {
            self.reject();
            return;
        };
        self.username = username.clone();
        let backend = if self.smartcard_login_ready() {
            AuthBackend::Smartcard
        } else {
            AuthBackend::Password
        };
        let secret = Zeroizing::new(self.password.to_string());
        let (rx, driver) = start_auth_session(username, secret, backend);
        self.auth_rx = Some(rx);
        self.auth_driver = Some(driver);
        self.phase = InputPhase::Authenticating;
    }

    fn poll_auth(&mut self) -> Option<AuthResult> {
        let rx = self.auth_rx.as_ref()?;
        match rx.try_recv() {
            Ok(result) => {
                self.auth_rx = None;
                Some(result)
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.auth_rx = None;
                Some(AuthResult::Error("auth thread vanished".into()))
            }
        }
    }

    /// After a Failed shake completes, drop back to Editing.
    fn tick(&mut self) {
        if let InputPhase::Failed(since) = self.phase {
            if since.elapsed() >= Duration::from_millis(FAILED_DURATION_MS) {
                self.phase = InputPhase::Editing;
            }
        }
    }

    fn confirm_power_action(&mut self, action: PowerAction) -> Option<ControlFlow> {
        if self
            .pending_power
            .is_some_and(|pending| pending.action == action && pending.is_active())
        {
            self.pending_power = None;
            return Some(action.control_flow());
        }
        self.pending_power = Some(PendingPowerAction {
            action,
            since: Instant::now(),
        });
        None
    }

    fn clear_expired_power_confirmation(&mut self) -> bool {
        if self
            .pending_power
            .is_some_and(|pending| !pending.is_active())
        {
            self.pending_power = None;
            true
        } else {
            false
        }
    }

    fn pending_power_action(&self) -> Option<PowerAction> {
        self.pending_power
            .filter(|pending| pending.is_active())
            .map(|pending| pending.action)
    }

    fn reject(&mut self) {
        self.password.clear();
        self.focus = Field::Password;
        self.phase = InputPhase::Failed(Instant::now());
        // The worker thread already exited (Failed/Error path doesn't
        // open_session); dropping the driver here joins it cleanly so we
        // don't leave a zombie thread per failed attempt.
        self.auth_driver = None;
    }

    fn shake_offset(&self) -> f32 {
        let InputPhase::Failed(since) = self.phase else {
            return 0.0;
        };
        let t = since.elapsed().as_secs_f32();
        let dur = FAILED_DURATION_MS as f32 / 1000.0;
        if t >= dur {
            return 0.0;
        }
        let damping = 1.0 - t / dur;
        FAILED_SHAKE_AMPLITUDE
            * damping
            * (t * FAILED_SHAKE_FREQ_HZ * 2.0 * std::f32::consts::PI).sin()
    }

    fn hint(&self) -> String {
        if let Some(action) = self.pending_power_action() {
            return match action {
                PowerAction::PowerOff => "Nochmal klicken zum Ausschalten",
                PowerAction::Reboot => "Nochmal klicken für Neustart",
            }
            .to_string();
        }
        let layout = keyboard_layout_label(&self.keyboard_status);
        match self.phase {
            InputPhase::Editing
                if self.smartcard_login_ready() && self.keyboard_status.caps_lock =>
            {
                format!("Smartcard bereit - Caps Lock aktiv - Layout {layout}")
            }
            InputPhase::Editing if self.smartcard_login_ready() => {
                "YubiKey PIN eingeben - Touch nach Enter".to_string()
            }
            InputPhase::Editing if self.security_key_present => {
                "YubiKey nicht registriert - Passwort-Login möglich".to_string()
            }
            InputPhase::Editing if self.keyboard_status.caps_lock => {
                format!("Caps Lock aktiv - Layout {layout}")
            }
            InputPhase::Editing => "Benutzername und Passwort eingeben".to_string(),
            InputPhase::Authenticating if self.smartcard_login_ready() => {
                "YubiKey berühren …".to_string()
            }
            InputPhase::Authenticating => "Anmelden …".to_string(),
            InputPhase::Failed(_) if self.smartcard_login_ready() => {
                "PIN oder YubiKey abgelehnt".to_string()
            }
            InputPhase::Failed(_) => "Anmeldung fehlgeschlagen".to_string(),
        }
    }

    fn smartcard_login_ready(&self) -> bool {
        self.security_key_present && self.smartcard_user.is_some()
    }

    fn auth_username(&self) -> Option<String> {
        if self.smartcard_login_ready() {
            self.smartcard_user.clone()
        } else {
            let username = self.username.trim();
            if username.is_empty() {
                None
            } else {
                Some(username.to_string())
            }
        }
    }

    fn update_security_key_state(&mut self) -> bool {
        let was_smartcard_ready = self.smartcard_login_ready();
        let present = yubikey_present();
        let smartcard_user = if present {
            smartcard_user_from_authfile(Path::new(SMARTCARD_AUTHFILE))
        } else {
            None
        };
        let changed = self.security_key_present != present || self.smartcard_user != smartcard_user;
        self.security_key_present = present;
        self.smartcard_user = smartcard_user;
        let is_smartcard_ready = self.smartcard_login_ready();
        if was_smartcard_ready != is_smartcard_ready {
            self.password.clear();
        }
        if let Some(user) = self.smartcard_user.clone() {
            self.username = user;
            self.focus = Field::Password;
        } else {
            if self.username.trim().is_empty() {
                self.focus = Field::Username;
            }
        }
        if changed {
            info!(
                present = self.security_key_present,
                smartcard_ready = is_smartcard_ready,
                smartcard_user = self.smartcard_user.as_deref().unwrap_or("-"),
                "security key state changed"
            );
        }
        changed
    }
}

fn yubikey_present() -> bool {
    yubikey_present_in_sysfs(Path::new("/sys/bus/usb/devices"))
        || yubikey_present_in_hidraw_sysfs(Path::new("/sys/class/hidraw"))
}

fn yubikey_present_in_sysfs(root: &Path) -> bool {
    let Ok(entries) = fs::read_dir(root) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = entry.path();
        fs::read_to_string(path.join("idVendor"))
            .ok()
            .is_some_and(|vendor| is_yubico_vendor_id(&vendor))
            || fs::read_to_string(path.join("manufacturer"))
                .ok()
                .is_some_and(|name| is_yubikey_name(&name))
            || fs::read_to_string(path.join("product"))
                .ok()
                .is_some_and(|name| is_yubikey_name(&name))
    })
}

fn yubikey_present_in_hidraw_sysfs(root: &Path) -> bool {
    let Ok(entries) = fs::read_dir(root) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let uevent_path = entry.path().join("device/uevent");
        fs::read_to_string(uevent_path).ok().is_some_and(|uevent| {
            hid_id_vendor_from_uevent(&uevent).is_some_and(is_yubico_vendor_id)
                || hid_name_from_uevent(&uevent).is_some_and(is_yubikey_name)
        })
    })
}

fn hid_id_vendor_from_uevent(uevent: &str) -> Option<&str> {
    uevent.lines().find_map(|line| {
        let hid_id = line.strip_prefix("HID_ID=")?;
        hid_id.split(':').nth(1)
    })
}

fn hid_name_from_uevent(uevent: &str) -> Option<&str> {
    uevent
        .lines()
        .find_map(|line| line.strip_prefix("HID_NAME="))
}

fn is_yubico_vendor_id(vendor: &str) -> bool {
    let vendor = vendor.trim();
    vendor.eq_ignore_ascii_case(YUBICO_USB_VENDOR_ID)
        || vendor
            .trim_start_matches('0')
            .eq_ignore_ascii_case(YUBICO_USB_VENDOR_ID)
}

fn is_yubikey_name(name: &str) -> bool {
    let name = name.trim();
    name.contains("Yubico") || name.contains("YubiKey")
}

fn smartcard_user_from_authfile(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    raw.lines()
        .filter_map(|line| line.split_once(':').map(|(user, _)| user.trim()))
        .find(|user| !user.is_empty())
        .map(str::to_string)
}

fn keyboard_layout_label(status: &KeyboardStatus) -> String {
    let layout = status
        .layout
        .split(',')
        .next()
        .unwrap_or(status.layout.as_str())
        .trim();
    if layout.is_empty() {
        "DE".to_string()
    } else {
        layout.to_uppercase()
    }
}

/// Light appearance, read once from the boot-chain marker at startup and
/// consulted by the metro_* card colours and the compass style so the
/// login matches the active desktop theme.
static LIGHT_APPEARANCE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static LOGIN_THEME: OnceLock<ThemeConfig> = OnceLock::new();

fn light_appearance() -> bool {
    LIGHT_APPEARANCE.load(std::sync::atomic::Ordering::Relaxed)
}

fn load_login_theme(appearance: Appearance) -> ThemeConfig {
    let mut theme_manager = ThemeManager::new();
    let theme_name = if appearance.is_light() {
        "light"
    } else {
        "dark"
    };
    if let Err(err) = theme_manager.set_theme(theme_name) {
        warn!(
            theme = theme_name,
            error = %err,
            "login theme load failed; using default theme"
        );
    }
    theme_manager.current().config.clone()
}

fn login_theme() -> &'static ThemeConfig {
    LOGIN_THEME.get_or_init(|| load_login_theme(read_appearance()))
}
