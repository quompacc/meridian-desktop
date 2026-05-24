// Keyboard input handling for meridian-login.
//
// Opens every /dev/input/event* device that exposes EV_KEY + KEY_A (i.e. a
// keyboard), sets them non-blocking, and translates raw evdev key events
// into [`KeyAction`]s using xkbcommon for layout-aware utf8 production.
//
// The render loop is expected to call [`poll_keyboards`] once per frame and
// apply the returned actions to the UI state.

use std::path::PathBuf;

use evdev::{AbsoluteAxisCode, Device, EventType, KeyCode, RelativeAxisCode};
use tracing::{debug, info, warn};
use xkbcommon::xkb;

/// Default keyboard layout when /etc/default/keyboard cannot be read.
const FALLBACK_LAYOUT: &str = "de";

/// What the UI should do in response to a keyboard event.
#[derive(Clone, Debug, PartialEq)]
pub enum KeyAction {
    /// Append this utf8 segment to the currently focused field.
    Insert(String),
    /// Remove the last char from the currently focused field.
    Backspace,
    /// Move focus to the next input field.
    CycleFocus,
    /// Move focus to the previous input field.
    CycleFocusBack,
    /// User pressed Enter — submit the form.
    Submit,
    /// User pressed Esc — cancel.
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointerAction {
    LeftPress { x: f32, y: f32 },
}

#[derive(Clone, Copy, Debug)]
struct AxisRange {
    min: i32,
    max: i32,
}

impl AxisRange {
    fn normalize(self, value: i32, span: f32) -> f32 {
        let width = (self.max - self.min).max(1) as f32;
        ((value - self.min) as f32 / width * span).clamp(0.0, span)
    }
}

pub struct PointerDevice {
    device: Device,
    abs_x: Option<AxisRange>,
    abs_y: Option<AxisRange>,
}

#[derive(Clone, Copy, Debug)]
pub struct PointerState {
    pub x: f32,
    pub y: f32,
    width: f32,
    height: f32,
}

impl PointerState {
    pub fn new(width: u32, height: u32) -> Self {
        let width = width as f32;
        let height = height as f32;
        Self {
            x: width / 2.0,
            y: height / 2.0,
            width,
            height,
        }
    }

    fn move_relative(&mut self, dx: i32, dy: i32) {
        self.x = (self.x + dx as f32).clamp(0.0, self.width - 1.0);
        self.y = (self.y + dy as f32).clamp(0.0, self.height - 1.0);
    }

    fn move_absolute_x(&mut self, range: AxisRange, value: i32) {
        self.x = range.normalize(value, self.width - 1.0);
    }

    fn move_absolute_y(&mut self, range: AxisRange, value: i32) {
        self.y = range.normalize(value, self.height - 1.0);
    }
}

/// Wraps the xkbcommon state required to translate evdev keycodes.
pub struct Keyboard {
    state: xkb::State,
    // The keymap must outlive the state; keep both alive together.
    _keymap: xkb::Keymap,
    _context: xkb::Context,
}

impl Keyboard {
    pub fn new() -> Result<Self, String> {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let layout = read_system_layout().unwrap_or_else(|| FALLBACK_LAYOUT.to_string());
        info!(layout = %layout, "xkb layout selected");
        let keymap = xkb::Keymap::new_from_names(
            &context,
            "",
            "",
            &layout,
            "",
            None,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .ok_or_else(|| format!("xkb keymap compilation failed for layout {}", layout))?;
        let state = xkb::State::new(&keymap);
        Ok(Self {
            state,
            _keymap: keymap,
            _context: context,
        })
    }

    /// Process one evdev key event. Returns the resulting [`KeyAction`] if
    /// the event should produce one (key-down or key-repeat with a meaningful
    /// symbol). Updates the modifier state for key-up so subsequent presses
    /// see the right modifier mask.
    fn process(&mut self, raw_code: u16, value: i32) -> Option<KeyAction> {
        // X11/xkb convention: keycode = evdev keycode + 8.
        let xkb_code: xkb::Keycode = (raw_code as u32 + 8).into();

        let direction = match value {
            1 => Some(xkb::KeyDirection::Down),
            0 => Some(xkb::KeyDirection::Up),
            2 => None, // repeat — don't transition, but do act
            _ => return None,
        };
        if let Some(d) = direction {
            self.state.update_key(xkb_code, d);
        }

        // Act on Down and Repeat, never on Up.
        if value != 1 && value != 2 {
            return None;
        }

        let sym = self.state.key_get_one_sym(xkb_code);
        match sym {
            xkb::Keysym::Tab => Some(KeyAction::CycleFocus),
            xkb::Keysym::ISO_Left_Tab => Some(KeyAction::CycleFocusBack),
            xkb::Keysym::Up => Some(KeyAction::CycleFocusBack),
            xkb::Keysym::Down => Some(KeyAction::CycleFocus),
            xkb::Keysym::BackSpace => Some(KeyAction::Backspace),
            xkb::Keysym::Return | xkb::Keysym::KP_Enter => Some(KeyAction::Submit),
            xkb::Keysym::Escape => Some(KeyAction::Cancel),
            _ => {
                let utf8 = self.state.key_get_utf8(xkb_code);
                // Filter out control characters and empty results — only feed
                // printable characters into the fields.
                if utf8.is_empty() || utf8.chars().any(|c| c.is_control()) {
                    None
                } else {
                    Some(KeyAction::Insert(utf8))
                }
            }
        }
    }
}

/// Open all event* nodes under /dev/input that expose at least KEY_A.
pub fn open_keyboards() -> std::io::Result<Vec<Device>> {
    let mut devs = Vec::new();
    let dir = match std::fs::read_dir("/dev/input") {
        Ok(d) => d,
        Err(e) => {
            warn!(error = %e, "cannot list /dev/input — no keyboard input available");
            return Ok(devs);
        }
    };
    for entry in dir.flatten() {
        let path: PathBuf = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with("event") {
            continue;
        }
        match Device::open(&path) {
            Ok(mut dev) => {
                let is_keyboard = dev
                    .supported_keys()
                    .map(|k| k.contains(evdev::KeyCode::KEY_A))
                    .unwrap_or(false);
                if !is_keyboard {
                    continue;
                }
                if let Err(e) = dev.set_nonblocking(true) {
                    warn!(path = ?path, error = %e, "failed to set device nonblocking; skipping");
                    continue;
                }
                // EVIOCGRAB: route keystrokes exclusively to us. Without this,
                // the kernel TTY (getty on tty1) also receives them — your
                // password would leak into the console buffer and be visible
                // once meridian-login exits and fbcon comes back. The grab is
                // released automatically on Device::drop.
                if let Err(e) = dev.grab() {
                    warn!(
                        path = ?path,
                        error = %e,
                        "failed to grab keyboard exclusively — keystrokes may also reach the kernel TTY"
                    );
                } else {
                    debug!(path = ?path, "grabbed keyboard exclusively");
                }
                debug!(
                    path = ?path,
                    name = ?dev.name().unwrap_or(""),
                    "opened keyboard"
                );
                devs.push(dev);
            }
            Err(e) => {
                debug!(path = ?path, error = %e, "skipping evdev node");
            }
        }
    }
    info!(count = devs.len(), "keyboards open");
    Ok(devs)
}

pub fn open_pointers() -> std::io::Result<Vec<PointerDevice>> {
    let mut devs = Vec::new();
    let dir = match std::fs::read_dir("/dev/input") {
        Ok(d) => d,
        Err(e) => {
            warn!(error = %e, "cannot list /dev/input — no pointer input available");
            return Ok(devs);
        }
    };

    for entry in dir.flatten() {
        let path: PathBuf = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with("event") {
            continue;
        }

        match Device::open(&path) {
            Ok(dev) => {
                let has_rel = dev.supported_relative_axes().is_some_and(|axes| {
                    axes.contains(RelativeAxisCode::REL_X) && axes.contains(RelativeAxisCode::REL_Y)
                });
                let has_abs = dev.supported_absolute_axes().is_some_and(|axes| {
                    axes.contains(AbsoluteAxisCode::ABS_X) && axes.contains(AbsoluteAxisCode::ABS_Y)
                });
                let has_button = dev
                    .supported_keys()
                    .is_some_and(|keys| keys.contains(KeyCode::BTN_LEFT));
                if !(has_button && (has_rel || has_abs)) {
                    continue;
                }
                if let Err(e) = dev.set_nonblocking(true) {
                    warn!(path = ?path, error = %e, "failed to set pointer nonblocking; skipping");
                    continue;
                }

                let mut abs_x = None;
                let mut abs_y = None;
                if has_abs {
                    match dev.get_absinfo() {
                        Ok(absinfo) => {
                            for (code, info) in absinfo {
                                let range = AxisRange {
                                    min: info.minimum(),
                                    max: info.maximum(),
                                };
                                match code {
                                    AbsoluteAxisCode::ABS_X => abs_x = Some(range),
                                    AbsoluteAxisCode::ABS_Y => abs_y = Some(range),
                                    _ => {}
                                }
                            }
                        }
                        Err(e) => {
                            debug!(path = ?path, error = %e, "failed to read absolute pointer range");
                        }
                    }
                }

                debug!(
                    path = ?path,
                    name = ?dev.name().unwrap_or(""),
                    "opened pointer"
                );
                devs.push(PointerDevice {
                    device: dev,
                    abs_x,
                    abs_y,
                });
            }
            Err(e) => {
                debug!(path = ?path, error = %e, "skipping evdev node");
            }
        }
    }
    info!(count = devs.len(), "pointers open");
    Ok(devs)
}

/// Drain any pending events on each device and return the [`KeyAction`]s
/// they produced. Devices that fail to fetch are silently skipped on this
/// frame and tried again next frame.
pub fn poll_keyboards(devs: &mut [Device], kb: &mut Keyboard) -> Vec<KeyAction> {
    let mut actions = Vec::new();
    for dev in devs {
        loop {
            match dev.fetch_events() {
                Ok(events) => {
                    let mut had_event = false;
                    for ev in events {
                        had_event = true;
                        if ev.event_type() == EventType::KEY {
                            if let Some(action) = kb.process(ev.code(), ev.value()) {
                                actions.push(action);
                            }
                        }
                    }
                    if !had_event {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    debug!(error = %e, "fetch_events failed; skipping device this frame");
                    break;
                }
            }
        }
    }
    actions
}

pub fn poll_pointers(devs: &mut [PointerDevice], pointer: &mut PointerState) -> Vec<PointerAction> {
    let mut actions = Vec::new();
    for ptr in devs {
        loop {
            match ptr.device.fetch_events() {
                Ok(events) => {
                    let mut had_event = false;
                    for ev in events {
                        had_event = true;
                        match ev.event_type() {
                            EventType::RELATIVE => match RelativeAxisCode(ev.code()) {
                                RelativeAxisCode::REL_X => pointer.move_relative(ev.value(), 0),
                                RelativeAxisCode::REL_Y => pointer.move_relative(0, ev.value()),
                                _ => {}
                            },
                            EventType::ABSOLUTE => match AbsoluteAxisCode(ev.code()) {
                                AbsoluteAxisCode::ABS_X => {
                                    if let Some(range) = ptr.abs_x {
                                        pointer.move_absolute_x(range, ev.value());
                                    }
                                }
                                AbsoluteAxisCode::ABS_Y => {
                                    if let Some(range) = ptr.abs_y {
                                        pointer.move_absolute_y(range, ev.value());
                                    }
                                }
                                _ => {}
                            },
                            EventType::KEY
                                if KeyCode(ev.code()) == KeyCode::BTN_LEFT && ev.value() == 1 =>
                            {
                                actions.push(PointerAction::LeftPress {
                                    x: pointer.x,
                                    y: pointer.y,
                                });
                            }
                            _ => {}
                        }
                    }
                    if !had_event {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    debug!(error = %e, "fetch pointer events failed; skipping device this frame");
                    break;
                }
            }
        }
    }
    actions
}

/// Best-effort read of XKBLAYOUT from /etc/default/keyboard.
fn read_system_layout() -> Option<String> {
    let s = std::fs::read_to_string("/etc/default/keyboard").ok()?;
    for line in s.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("XKBLAYOUT=") {
            let v = rest.trim_matches('"').trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_constructs_with_default_layout() {
        // best-effort: requires libxkbcommon at test time, which we have
        let _ = Keyboard::new().expect("keyboard");
    }

    #[test]
    fn read_system_layout_returns_some_or_none_without_panic() {
        // We don't assert the value because it depends on the test host.
        let _ = read_system_layout();
    }

    #[test]
    fn process_filters_pure_control_chars() {
        let mut kb = Keyboard::new().unwrap();
        // Ctrl is a modifier — pressing it alone yields no Insert action.
        // KEY_LEFTCTRL = 29 in evdev, so xkb_code = 37.
        let action = kb.process(29, 1);
        // Pressing the modifier by itself does not insert text.
        assert!(!matches!(action, Some(KeyAction::Insert(_))));
    }
}
