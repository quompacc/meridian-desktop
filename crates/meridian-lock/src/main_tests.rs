use super::*;

// Baut einen echten us-Keymap-State, damit handle_key die Keysyms
// genau wie zur Laufzeit auflöst (Keycode = evdev + 8).
fn us_keymap_state() -> xkb::State {
    let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    let keymap =
        xkb::Keymap::new_from_names(&ctx, "", "", "us", "", None, xkb::KEYMAP_COMPILE_NO_FLAGS)
            .expect("compile us keymap (needs xkb data installed)");
    xkb::State::new(&keymap)
}

fn test_state() -> AppState {
    AppState {
        running: true,
        compositor: None,
        shm: None,
        seat: None,
        lock_manager: None,
        lock: None,
        session_locked: false,
        finished: false,
        pending_outputs: Vec::new(),
        lock_surfaces: Vec::new(),
        xkb_ctx: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
        xkb_state: Some(us_keymap_state()),
        password: Zeroizing::new(String::new()),
        username: "tester".to_string(),
        status: LockStatus::Idle,
        auth_rx: None,
        style: LockStyle::from_theme(&meridian_config::ThemeConfig::default()),
    }
}

// evdev-Keycodes; handle_key addiert intern +8 auf den X/xkb-Keycode.
const KEY_ESC: u32 = 1;
const KEY_BACKSPACE: u32 = 14;
const KEY_ENTER: u32 = 28;
const KEY_A: u32 = 30;
const KEY_S: u32 = 31;
const KEY_D: u32 = 32;

#[test]
fn color_unpacks_rgba_channels() {
    assert_eq!(
        color(0xFFFFFFFF),
        Color::from_rgba(1.0, 1.0, 1.0, 1.0).unwrap()
    );
    assert_eq!(
        color(0x00000000),
        Color::from_rgba(0.0, 0.0, 0.0, 0.0).unwrap()
    );
    assert_eq!(
        color(0xFF112233),
        Color::from_rgba(
            0x11 as f32 / 255.0,
            0x22 as f32 / 255.0,
            0x33 as f32 / 255.0,
            1.0,
        )
        .unwrap()
    );
}

#[test]
fn typing_appends_characters() {
    let mut state = test_state();
    handle_key(&mut state, KEY_A);
    handle_key(&mut state, KEY_S);
    handle_key(&mut state, KEY_D);
    assert_eq!(state.password.as_str(), "asd");
}

#[test]
fn backspace_removes_one_ascii_char() {
    let mut state = test_state();
    state.password = Zeroizing::new("abc".to_string());
    handle_key(&mut state, KEY_BACKSPACE);
    assert_eq!(state.password.as_str(), "ab");
}

#[test]
fn backspace_removes_one_full_utf8_char() {
    let mut state = test_state();
    // "a" + U+00E9 (e-acute, 2 bytes): backspace muss den ganzen
    // Codepoint entfernen, nicht nur ein Byte.
    // "a" + U+00E9 (e-acute) = bytes 0x61, 0xC3 0xA9 in UTF-8.
    let p = String::from_utf8(vec![0x61, 0xc3, 0xa9]).unwrap();
    state.password = Zeroizing::new(p);
    handle_key(&mut state, KEY_BACKSPACE);
    assert_eq!(state.password.as_str(), "a");
}

#[test]
fn backspace_on_empty_password_is_noop() {
    let mut state = test_state();
    handle_key(&mut state, KEY_BACKSPACE);
    assert_eq!(state.password.as_str(), "");
}

#[test]
fn escape_clears_password_and_resets_status() {
    let mut state = test_state();
    state.password = Zeroizing::new("secret".to_string());
    state.status = LockStatus::Failed;
    handle_key(&mut state, KEY_ESC);
    assert_eq!(state.password.as_str(), "");
    assert!(state.status == LockStatus::Idle);
}

#[test]
fn auth_in_progress_blocks_input() {
    let mut state = test_state();
    state.password = Zeroizing::new("ab".to_string());
    let (_tx, rx) = mpsc::channel();
    state.auth_rx = Some(rx);
    handle_key(&mut state, KEY_A);
    handle_key(&mut state, KEY_BACKSPACE);
    assert_eq!(state.password.as_str(), "ab");
}

#[test]
fn return_with_empty_password_does_not_start_auth() {
    let mut state = test_state();
    handle_key(&mut state, KEY_ENTER);
    assert!(state.auth_rx.is_none());
    assert!(state.status == LockStatus::Idle);
}

#[test]
fn typing_resets_failed_status_to_idle() {
    let mut state = test_state();
    state.status = LockStatus::Failed;
    handle_key(&mut state, KEY_A);
    assert!(state.status == LockStatus::Idle);
    assert_eq!(state.password.as_str(), "a");
}

#[test]
fn card_frame_is_bounded_and_reflects_password_length() {
    let state = test_state();
    let empty = render_card_frame(0, &state.username, &LockStatus::Idle, &state.style);
    let typed = render_card_frame(3, &state.username, &LockStatus::Idle, &state.style);

    assert_eq!(empty.len(), (CARD_W * CARD_H * 4.0) as usize);
    assert_eq!(typed.len(), empty.len());
    assert_ne!(typed, empty);
}
