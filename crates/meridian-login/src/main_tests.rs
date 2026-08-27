use super::*;

fn smartcard_ready_state() -> LoginUiState {
    LoginUiState {
        security_key_present: true,
        smartcard_user: Some("eduard".to_string()),
        focus: Field::Password,
        ..Default::default()
    }
}

#[test]
fn anim_frame_at_t0_matches_settle_state() {
    let af = compute_anim_frame(0.0);
    assert_eq!(af.card_alpha, 0.0);
    assert_eq!(af.ui_alpha, 0.0);
}

#[test]
fn anim_frame_at_ui_fade_end_is_full() {
    let t = UI_FADE_END_MS as f32 / 1000.0;
    let af = compute_anim_frame(t);
    assert!((af.card_alpha - 1.0).abs() < 1e-3);
    assert!((af.ui_alpha - 1.0).abs() < 1e-3);
}

#[test]
fn anim_frame_reports_steady_after_intro() {
    let t = (UI_FADE_END_MS + 100) as f32 / 1000.0;
    let af = compute_anim_frame(t);
    assert!(anim_frame_is_steady(&af));
}

#[test]
fn card_rect_clamped_dimensions() {
    let (_, _, cw, ch) = card_rect(1920.0, 1440.0);
    assert!((500.0..=640.0).contains(&cw));
    assert!((460.0..=520.0).contains(&ch));
}

#[test]
fn yubikey_detector_matches_yubico_vendor_id() {
    let root = std::env::temp_dir().join(format!(
        "meridian-login-yubikey-test-{}",
        std::process::id()
    ));
    let dev = root.join("1-1");
    fs::create_dir_all(&dev).unwrap();
    fs::write(dev.join("idVendor"), "1050\n").unwrap();

    assert!(yubikey_present_in_sysfs(&root));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn yubikey_detector_accepts_zero_padded_hid_vendor_id() {
    assert!(is_yubico_vendor_id("00001050"));
    assert!(is_yubico_vendor_id("1050"));
    assert!(!is_yubico_vendor_id("0627"));
}

#[test]
fn yubikey_detector_matches_yubikey_product_name() {
    let root = std::env::temp_dir().join(format!(
        "meridian-login-yubikey-name-test-{}",
        std::process::id()
    ));
    let dev = root.join("1-9");
    fs::create_dir_all(&dev).unwrap();
    fs::write(dev.join("idVendor"), "1234\n").unwrap();
    fs::write(dev.join("product"), "Yubico YubiKey OTP+FIDO+CCID\n").unwrap();

    assert!(yubikey_present_in_sysfs(&root));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn hidraw_detector_matches_yubico_hid_id() {
    let root = std::env::temp_dir().join(format!(
        "meridian-login-hidraw-yubikey-test-{}",
        std::process::id()
    ));
    let dev = root.join("hidraw0/device");
    fs::create_dir_all(&dev).unwrap();
    fs::write(
        dev.join("uevent"),
        "HID_ID=0003:00001050:00000407\nHID_NAME=Yubico YubiKey\n",
    )
    .unwrap();

    assert!(yubikey_present_in_hidraw_sysfs(&root));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn hidraw_detector_matches_yubikey_hid_name() {
    let root = std::env::temp_dir().join(format!(
        "meridian-login-hidraw-yubikey-name-test-{}",
        std::process::id()
    ));
    let dev = root.join("hidraw0/device");
    fs::create_dir_all(&dev).unwrap();
    fs::write(
        dev.join("uevent"),
        "HID_ID=0003:00001234:00005678\nHID_NAME=Yubico Security Key\n",
    )
    .unwrap();

    assert!(yubikey_present_in_hidraw_sysfs(&root));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn yubikey_detector_ignores_other_vendor_ids() {
    let root = std::env::temp_dir().join(format!(
        "meridian-login-no-yubikey-test-{}",
        std::process::id()
    ));
    let dev = root.join("1-1");
    fs::create_dir_all(&dev).unwrap();
    fs::write(dev.join("idVendor"), "1234\n").unwrap();

    assert!(!yubikey_present_in_sysfs(&root));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn smartcard_user_reads_first_mapping_user() {
    let root = std::env::temp_dir().join(format!(
        "meridian-login-u2f-map-test-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    let file = root.join("u2f_keys");
    fs::write(&file, "eduard:credential-data\n").unwrap();

    assert_eq!(
        smartcard_user_from_authfile(&file),
        Some("eduard".to_string())
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn power_buttons_are_click_targets() {
    let (restart, poweroff) = power_button_rects(1920.0, 1080.0, 0.0);
    let restart_center = (restart.0 + restart.2 / 2.0, restart.1 + restart.3 / 2.0);
    let poweroff_center = (poweroff.0 + poweroff.2 / 2.0, poweroff.1 + poweroff.3 / 2.0);

    assert_eq!(
        click_target_at(
            1920.0,
            1080.0,
            restart_center.0,
            restart_center.1,
            0.0,
            false
        ),
        Some(ClickTarget::Reboot)
    );
    assert_eq!(
        click_target_at(
            1920.0,
            1080.0,
            poweroff_center.0,
            poweroff_center.1,
            0.0,
            false
        ),
        Some(ClickTarget::PowerOff)
    );
}

#[test]
fn login_button_is_submit_target() {
    let login = login_button_rect(1920.0, 1080.0, 0.0);
    assert_eq!(
        click_target_at(
            1920.0,
            1080.0,
            login.0 + login.2 / 2.0,
            login.1 + login.3 / 2.0,
            0.0,
            false,
        ),
        Some(ClickTarget::Submit)
    );
}

#[test]
fn empty_placeholder_keeps_caret_at_text_start() {
    assert_eq!(caret_x(true, 120.0, 230.0), 120.0);
    assert_eq!(caret_x(false, 120.0, 230.0), 230.0);
}

#[test]
fn smartcard_mode_does_not_focus_username_field() {
    let (card_left, card_top, _, _) = card_rect(1920.0, 1080.0);
    let x = card_left + CARD_PAD + 8.0;
    let y = card_top + CARD_PAD + USER_BOX_OFFSET_Y + 8.0;
    assert_eq!(click_target_at(1920.0, 1080.0, x, y, 0.0, true), None);
}

#[test]
fn smartcard_mode_focuses_short_pin_field() {
    let pin = smartcard_pin_rect(1920.0, 1080.0, 0.0);
    assert_eq!(
        click_target_at(
            1920.0,
            1080.0,
            pin.0 + pin.2 / 2.0,
            pin.1 + pin.3 / 2.0,
            0.0,
            true
        ),
        Some(ClickTarget::Field(Field::Password))
    );
}

#[test]
fn manual_mode_focuses_username_and_password_fields() {
    let (card_left, card_top, cw, _) = card_rect(1920.0, 1080.0);
    let inner_left = card_left + CARD_PAD;
    let inner_top = card_top + CARD_PAD;
    let inner_w = cw - 2.0 * CARD_PAD;
    let username_center = (
        inner_left + inner_w / 2.0,
        inner_top + USER_BOX_OFFSET_Y + INPUT_BOX_HEIGHT / 2.0,
    );
    let password_center = (
        inner_left + inner_w / 2.0,
        inner_top + PASSWORD_BOX_OFFSET_Y + INPUT_BOX_HEIGHT / 2.0,
    );

    assert_eq!(
        click_target_at(
            1920.0,
            1080.0,
            username_center.0,
            username_center.1,
            0.0,
            false,
        ),
        Some(ClickTarget::Field(Field::Username))
    );
    assert_eq!(
        click_target_at(
            1920.0,
            1080.0,
            password_center.0,
            password_center.1,
            0.0,
            false,
        ),
        Some(ClickTarget::Field(Field::Password))
    );
}

#[test]
fn power_action_requires_second_matching_click() {
    let mut s = LoginUiState::default();
    assert_eq!(s.confirm_power_action(PowerAction::Reboot), None);
    assert_eq!(s.pending_power_action(), Some(PowerAction::Reboot));
    assert_eq!(
        s.confirm_power_action(PowerAction::Reboot),
        Some(ControlFlow::Reboot)
    );
}

#[test]
fn power_confirmation_switches_and_expires() {
    let mut s = LoginUiState::default();
    assert_eq!(s.confirm_power_action(PowerAction::Reboot), None);
    assert_eq!(s.confirm_power_action(PowerAction::PowerOff), None);
    assert_eq!(s.pending_power_action(), Some(PowerAction::PowerOff));
    assert_eq!(
        s.confirm_power_action(PowerAction::PowerOff),
        Some(ControlFlow::PowerOff)
    );

    s.pending_power = Some(PendingPowerAction {
        action: PowerAction::Reboot,
        since: Instant::now() - POWER_CONFIRM_WINDOW - Duration::from_millis(1),
    });
    assert!(s.clear_expired_power_confirmation());
    assert_eq!(s.pending_power_action(), None);
}

#[test]
fn rounded_rect_path_does_not_panic_on_small_inputs() {
    let _ = rounded_rect_path(0.0, 0.0, 4.0, 4.0, 10.0);
}

#[test]
fn insert_appends_to_pin_field() {
    let mut s = smartcard_ready_state();
    assert_eq!(s.focus, Field::Password);
    assert_eq!(
        s.apply(KeyAction::Insert("a".into())),
        ControlFlow::Continue
    );
    assert_eq!(
        s.apply(KeyAction::Insert("b".into())),
        ControlFlow::Continue
    );
    assert_eq!(s.username, "");
    assert_eq!(s.password.as_str(), "ab");
}

#[test]
fn backspace_removes_last_char_from_pin_field() {
    let mut s = smartcard_ready_state();
    s.apply(KeyAction::Insert("abc".into()));
    s.apply(KeyAction::Backspace);
    assert_eq!(s.password.as_str(), "ab");
    s.apply(KeyAction::Backspace);
    s.apply(KeyAction::Backspace);
    s.apply(KeyAction::Backspace); // no-op on empty
    assert_eq!(s.password.as_str(), "");
}

#[test]
fn smartcard_focus_cycles_through_power_controls() {
    let mut s = smartcard_ready_state();
    s.apply(KeyAction::CycleFocus);
    assert_eq!(s.focus, Field::Password);
    assert_eq!(s.power_focus, Some(PowerAction::Reboot));
    s.apply(KeyAction::CycleFocus);
    assert_eq!(s.focus, Field::Password);
    assert_eq!(s.power_focus, Some(PowerAction::PowerOff));
    s.apply(KeyAction::CycleFocus);
    assert_eq!(s.power_focus, None);
}

#[test]
fn manual_mode_edits_username_and_password() {
    let mut s = LoginUiState::default();
    assert_eq!(s.focus, Field::Username);
    s.apply(KeyAction::Insert("eduard".into()));
    assert_eq!(s.username, "eduard");
    assert_eq!(s.password.as_str(), "");

    s.apply(KeyAction::CycleFocus);
    assert_eq!(s.focus, Field::Password);
    s.apply(KeyAction::Insert("secret".into()));
    s.apply(KeyAction::Backspace);

    assert_eq!(s.username, "eduard");
    assert_eq!(s.password.as_str(), "secre");
}

#[test]
fn manual_mode_cycles_between_username_and_password() {
    let mut s = LoginUiState::default();
    assert_eq!(s.focus, Field::Username);
    s.apply(KeyAction::CycleFocus);
    assert_eq!(s.focus, Field::Password);
    s.apply(KeyAction::CycleFocusBack);
    assert_eq!(s.focus, Field::Username);
}

#[test]
fn keyboard_can_confirm_reboot_and_escape_cancels_confirmation() {
    let mut s = LoginUiState::default();
    s.apply(KeyAction::CycleFocus);
    s.apply(KeyAction::CycleFocus);
    assert_eq!(s.power_focus, Some(PowerAction::Reboot));
    assert_eq!(s.apply(KeyAction::Submit), ControlFlow::Continue);
    assert_eq!(s.pending_power_action(), Some(PowerAction::Reboot));
    assert_eq!(s.apply(KeyAction::Cancel), ControlFlow::Continue);
    assert_eq!(s.pending_power_action(), None);
    assert_eq!(s.power_focus, None);

    s.apply(KeyAction::CycleFocus);
    assert_eq!(s.apply(KeyAction::Submit), ControlFlow::Continue);
    assert_eq!(s.apply(KeyAction::Submit), ControlFlow::Reboot);
}

#[cfg(any(target_os = "openbsd", target_os = "freebsd"))]
#[test]
fn bsd_power_actions_use_absolute_shutdown_commands() {
    assert_eq!(
        power_command(PowerAction::Reboot).unwrap(),
        PowerCommand {
            program: "/sbin/shutdown",
            args: &["-r", "now"],
        }
    );
    assert_eq!(
        power_command(PowerAction::PowerOff).unwrap(),
        PowerCommand {
            program: "/sbin/shutdown",
            args: &["-p", "now"],
        }
    );
}

#[test]
fn failed_power_action_restores_greeter_but_success_does_not() {
    let failed = Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "test command missing",
    ));
    assert!(restore_greeter_after_power_result(
        PowerAction::Reboot,
        &failed
    ));
    assert!(!restore_greeter_after_power_result(
        PowerAction::PowerOff,
        &Ok(())
    ));
}

#[cfg(target_os = "linux")]
#[test]
fn linux_power_actions_use_absolute_systemctl_commands() {
    assert_eq!(
        power_command(PowerAction::Reboot).unwrap(),
        PowerCommand {
            program: "/usr/bin/systemctl",
            args: &["reboot"],
        }
    );
}

#[test]
fn submit_and_cancel_return_their_control_flow() {
    let mut s = LoginUiState::default();
    assert_eq!(s.apply(KeyAction::Submit), ControlFlow::Submit);
    assert_eq!(s.apply(KeyAction::Cancel), ControlFlow::Cancel);
}

#[test]
fn greeter_restarts_after_desktop_exit_or_failed_spawn() {
    assert!(should_restart_greeter(ControlFlow::Submit, true));
    assert!(should_restart_greeter(ControlFlow::Submit, false));
    assert!(!should_restart_greeter(ControlFlow::Cancel, false));
}

#[test]
fn reject_keeps_username_and_resets_password_focus() {
    let mut s = LoginUiState {
        username: "eduard".into(),
        password: Zeroizing::new("badpassword".into()),
        focus: Field::Password,
        ..Default::default()
    };
    s.reject();
    assert_eq!(s.username, "eduard");
    assert!(s.password.is_empty());
    assert_eq!(s.focus, Field::Password);
    assert!(matches!(s.phase, InputPhase::Failed(_)));
}

#[test]
fn shake_offset_is_zero_outside_failed_state() {
    let s = LoginUiState::default();
    assert_eq!(s.shake_offset(), 0.0);
}

#[test]
fn shake_offset_nonzero_inside_failed_window() {
    let s = LoginUiState {
        phase: InputPhase::Failed(Instant::now()),
        ..Default::default()
    };
    // Sample several t-values within the window — at least one must
    // produce a nonzero offset (sine isn't always at a zero crossing).
    let mut saw_nonzero = false;
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(10));
        if s.shake_offset().abs() > 0.01 {
            saw_nonzero = true;
            break;
        }
    }
    assert!(
        saw_nonzero,
        "shake_offset stayed at 0 across 200ms of Failed"
    );
}

#[test]
fn hint_changes_with_phase() {
    let mut s = LoginUiState::default();
    assert_eq!(s.hint(), "Benutzername und Passwort eingeben");
    s.phase = InputPhase::Authenticating;
    assert_eq!(s.hint(), "Anmelden …");
    s.phase = InputPhase::Failed(Instant::now());
    assert_eq!(s.hint(), "Anmeldung fehlgeschlagen");
}

#[test]
fn hint_warns_when_caps_lock_is_active() {
    let s = LoginUiState {
        keyboard_status: KeyboardStatus {
            layout: "de".to_string(),
            caps_lock: true,
        },
        ..Default::default()
    };
    assert_eq!(s.hint(), "Caps Lock aktiv - Layout DE");
}

#[test]
fn hint_mentions_yubikey_when_present() {
    let s = LoginUiState {
        keyboard_status: KeyboardStatus {
            layout: "de".to_string(),
            caps_lock: false,
        },
        ..smartcard_ready_state()
    };
    assert_eq!(s.hint(), "YubiKey PIN eingeben - Touch nach Enter");
}

#[test]
fn hint_allows_password_login_with_unregistered_yubikey() {
    let s = LoginUiState {
        security_key_present: true,
        ..Default::default()
    };
    assert_eq!(
        s.hint(),
        "YubiKey nicht registriert - Passwort-Login möglich"
    );
}

#[test]
fn hint_prompts_touch_while_authenticating_with_yubikey() {
    let mut s = smartcard_ready_state();
    s.phase = InputPhase::Authenticating;
    assert_eq!(s.hint(), "YubiKey berühren …");
}

#[test]
fn poll_auth_returns_none_when_no_thread_running() {
    let mut s = LoginUiState::default();
    assert!(s.poll_auth().is_none());
}

#[test]
fn start_auth_with_empty_username_returns_failed_quickly() {
    let mut s = LoginUiState::default();
    s.start_auth();
    assert!(matches!(s.phase, InputPhase::Failed(_)));
    assert!(s.auth_driver.is_none());
}

#[test]
fn auth_username_uses_manual_username_without_yubikey() {
    let s = LoginUiState {
        username: " eduard ".into(),
        ..Default::default()
    };
    assert_eq!(s.auth_username(), Some("eduard".to_string()));
}

#[test]
fn auth_username_uses_smartcard_mapping_when_ready() {
    let s = LoginUiState {
        username: "typed-user".into(),
        ..smartcard_ready_state()
    };
    assert_eq!(s.auth_username(), Some("eduard".to_string()));
}

#[test]
fn insert_respects_max_field_len() {
    let mut s = LoginUiState::default();
    for _ in 0..MAX_FIELD_LEN + 16 {
        s.apply(KeyAction::Insert("a".into()));
    }
    assert_eq!(s.username.chars().count(), MAX_FIELD_LEN);
}
