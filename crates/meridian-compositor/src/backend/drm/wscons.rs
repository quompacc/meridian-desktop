//! Native OpenBSD wscons input source.
//!
//! OpenBSD exposes keyboard and pointer events as fixed-size `wscons_event`
//! records. Reading those devices directly keeps the DRM backend independent
//! of libinput's Linux-oriented udev compatibility layer.

use std::{
    collections::HashSet,
    io,
    mem::{size_of, MaybeUninit},
    os::fd::{AsFd, AsRawFd, BorrowedFd},
    path::PathBuf,
};

use smithay::{
    backend::{
        input::{
            Axis, AxisRelativeDirection, AxisSource, ButtonState, Device, DeviceCapability, Event,
            InputBackend, InputEvent, KeyState, KeyboardKeyEvent, Keycode, PointerAxisEvent,
            PointerButtonEvent, PointerMotionEvent, UnusedEvent,
        },
        session::{libseat::LibSeatSession, Session},
    },
    reexports::{
        calloop::{generic::Generic, Interest, Mode, PostAction},
        rustix::fs::OFlags,
    },
};
use tracing::{info, warn};

use crate::state::MeridianState;

const DEFAULT_KEYBOARD_DEVICE: &str = "/dev/wskbd0";
const DEFAULT_POINTER_DEVICE: &str = "/dev/wsmouse0";

const EVENT_KEY_UP: u32 = 1;
const EVENT_KEY_DOWN: u32 = 2;
const EVENT_ALL_KEYS_UP: u32 = 3;
const EVENT_MOUSE_UP: u32 = 4;
const EVENT_MOUSE_DOWN: u32 = 5;
const EVENT_MOUSE_DELTA_X: u32 = 6;
const EVENT_MOUSE_DELTA_Y: u32 = 7;
const EVENT_MOUSE_DELTA_Z: u32 = 10;
const EVENT_MOUSE_DELTA_W: u32 = 16;
const EVENT_HSCROLL: u32 = 26;
const EVENT_VSCROLL: u32 = 27;

const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;
const BTN_MIDDLE: u32 = 0x112;
const BTN_SIDE: u32 = 0x113;

#[repr(C)]
#[derive(Clone, Copy)]
struct WsconsEvent {
    event_type: libc::c_uint,
    value: libc::c_int,
    time: libc::timespec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum WsconsDevice {
    Keyboard,
    Pointer,
}

impl Device for WsconsDevice {
    fn id(&self) -> String {
        match self {
            Self::Keyboard => "openbsd-wskbd0",
            Self::Pointer => "openbsd-wsmouse0",
        }
        .to_owned()
    }

    fn name(&self) -> String {
        match self {
            Self::Keyboard => "OpenBSD wscons keyboard",
            Self::Pointer => "OpenBSD wscons pointer",
        }
        .to_owned()
    }

    fn has_capability(&self, capability: DeviceCapability) -> bool {
        matches!(
            (self, capability),
            (Self::Keyboard, DeviceCapability::Keyboard)
                | (Self::Pointer, DeviceCapability::Pointer)
        )
    }

    fn usb_id(&self) -> Option<(u32, u32)> {
        None
    }

    fn syspath(&self) -> Option<PathBuf> {
        Some(PathBuf::from(match self {
            Self::Keyboard => DEFAULT_KEYBOARD_DEVICE,
            Self::Pointer => DEFAULT_POINTER_DEVICE,
        }))
    }
}

#[derive(Debug)]
struct WsconsInput;

impl InputBackend for WsconsInput {
    type Device = WsconsDevice;
    type KeyboardKeyEvent = WsconsKeyboardEvent;
    type PointerAxisEvent = WsconsAxisEvent;
    type PointerButtonEvent = WsconsButtonEvent;
    type PointerMotionEvent = WsconsMotionEvent;
    type PointerMotionAbsoluteEvent = UnusedEvent;
    type GestureSwipeBeginEvent = UnusedEvent;
    type GestureSwipeUpdateEvent = UnusedEvent;
    type GestureSwipeEndEvent = UnusedEvent;
    type GesturePinchBeginEvent = UnusedEvent;
    type GesturePinchUpdateEvent = UnusedEvent;
    type GesturePinchEndEvent = UnusedEvent;
    type GestureHoldBeginEvent = UnusedEvent;
    type GestureHoldEndEvent = UnusedEvent;
    type TouchDownEvent = UnusedEvent;
    type TouchUpEvent = UnusedEvent;
    type TouchMotionEvent = UnusedEvent;
    type TouchCancelEvent = UnusedEvent;
    type TouchFrameEvent = UnusedEvent;
    type TabletToolAxisEvent = UnusedEvent;
    type TabletToolProximityEvent = UnusedEvent;
    type TabletToolTipEvent = UnusedEvent;
    type TabletToolButtonEvent = UnusedEvent;
    type SwitchToggleEvent = UnusedEvent;
    type SpecialEvent = UnusedEvent;
}

#[derive(Debug, Clone, Copy)]
struct WsconsKeyboardEvent {
    time: u64,
    key: u32,
    state: KeyState,
    count: u32,
}

impl Event<WsconsInput> for WsconsKeyboardEvent {
    fn time(&self) -> u64 {
        self.time
    }

    fn device(&self) -> WsconsDevice {
        WsconsDevice::Keyboard
    }
}

impl KeyboardKeyEvent<WsconsInput> for WsconsKeyboardEvent {
    fn key_code(&self) -> Keycode {
        // On amd64, wskbd PC/USB events use XT-compatible key numbers. XKB
        // consumes those in the same +8 namespace used by Smithay/libinput.
        // pckbd consumer keys are the exception and need translating to their
        // evdev equivalents before entering the shared XKB path.
        (wscons_key_code(self.key) + 8).into()
    }

    fn state(&self) -> KeyState {
        self.state
    }

    fn count(&self) -> u32 {
        self.count
    }
}

#[derive(Debug, Clone, Copy)]
struct WsconsMotionEvent {
    time: u64,
    dx: f64,
    dy: f64,
}

impl Event<WsconsInput> for WsconsMotionEvent {
    fn time(&self) -> u64 {
        self.time
    }

    fn device(&self) -> WsconsDevice {
        WsconsDevice::Pointer
    }
}

impl PointerMotionEvent<WsconsInput> for WsconsMotionEvent {
    fn delta_x(&self) -> f64 {
        self.dx
    }

    fn delta_y(&self) -> f64 {
        self.dy
    }

    fn delta_x_unaccel(&self) -> f64 {
        self.dx
    }

    fn delta_y_unaccel(&self) -> f64 {
        self.dy
    }
}

#[derive(Debug, Clone, Copy)]
struct WsconsButtonEvent {
    time: u64,
    button: u32,
    state: ButtonState,
}

impl Event<WsconsInput> for WsconsButtonEvent {
    fn time(&self) -> u64 {
        self.time
    }

    fn device(&self) -> WsconsDevice {
        WsconsDevice::Pointer
    }
}

impl PointerButtonEvent<WsconsInput> for WsconsButtonEvent {
    fn button_code(&self) -> u32 {
        wscons_button_code(self.button)
    }

    fn state(&self) -> ButtonState {
        self.state
    }
}

#[derive(Debug, Clone, Copy)]
struct WsconsAxisEvent {
    time: u64,
    axis: Axis,
    value: f64,
    precise: bool,
}

impl Event<WsconsInput> for WsconsAxisEvent {
    fn time(&self) -> u64 {
        self.time
    }

    fn device(&self) -> WsconsDevice {
        WsconsDevice::Pointer
    }
}

impl PointerAxisEvent<WsconsInput> for WsconsAxisEvent {
    fn amount(&self, axis: Axis) -> Option<f64> {
        (self.precise && axis == self.axis).then_some(self.value / 4096.0)
    }

    fn amount_v120(&self, axis: Axis) -> Option<f64> {
        (!self.precise && axis == self.axis).then_some(self.value * 120.0)
    }

    fn source(&self) -> AxisSource {
        if self.precise {
            AxisSource::Continuous
        } else {
            AxisSource::Wheel
        }
    }

    fn relative_direction(&self, _axis: Axis) -> AxisRelativeDirection {
        AxisRelativeDirection::Identical
    }
}

pub(super) fn register_wscons_event_sources(
    event_loop: &mut smithay::reexports::calloop::EventLoop<MeridianState>,
    session: &mut LibSeatSession,
) -> Result<(), Box<dyn std::error::Error>> {
    let keyboard_path = device_path("MERIDIAN_WSKBD_DEVICE", DEFAULT_KEYBOARD_DEVICE);
    let pointer_path = device_path("MERIDIAN_WSMOUSE_DEVICE", DEFAULT_POINTER_DEVICE);
    let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK;

    let keyboard_fd = session.open(&keyboard_path, flags)?;
    let pointer_fd = session.open(&pointer_path, flags)?;

    let mut pressed_keys = HashSet::new();
    event_loop.handle().insert_source(
        Generic::new(keyboard_fd, Interest::READ, Mode::Level),
        move |_, fd, state| {
            match read_events(fd.as_fd()) {
                Ok(events) => dispatch_keyboard_events(state, &events, &mut pressed_keys),
                Err(error) => warn!(%error, "failed to read OpenBSD wskbd events"),
            }
            Ok(PostAction::Continue)
        },
    )?;

    event_loop.handle().insert_source(
        Generic::new(pointer_fd, Interest::READ, Mode::Level),
        move |_, fd, state| {
            match read_events(fd.as_fd()) {
                Ok(events) => dispatch_pointer_events(state, &events),
                Err(error) => warn!(%error, "failed to read OpenBSD wsmouse events"),
            }
            Ok(PostAction::Continue)
        },
    )?;

    info!(
        keyboard = %keyboard_path.display(),
        pointer = %pointer_path.display(),
        "native OpenBSD wscons input backend initialized"
    );
    Ok(())
}

fn device_path(variable: &str, fallback: &str) -> PathBuf {
    std::env::var_os(variable)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(fallback))
}

fn read_events(fd: BorrowedFd<'_>) -> io::Result<Vec<WsconsEvent>> {
    const EVENT_BATCH: usize = 32;
    let mut storage = [MaybeUninit::<WsconsEvent>::uninit(); EVENT_BATCH];
    let byte_capacity = storage.len() * size_of::<WsconsEvent>();
    // SAFETY: `storage` points to writable memory for `byte_capacity` bytes and
    // the borrowed descriptor remains valid for the duration of this read.
    let bytes_read = unsafe {
        libc::read(
            fd.as_raw_fd(),
            storage.as_mut_ptr().cast::<libc::c_void>(),
            byte_capacity,
        )
    };
    if bytes_read < 0 {
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::WouldBlock {
            return Ok(Vec::new());
        }
        return Err(error);
    }

    let bytes_read = bytes_read as usize;
    if !bytes_read.is_multiple_of(size_of::<WsconsEvent>()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "partial wscons_event record",
        ));
    }

    let count = bytes_read / size_of::<WsconsEvent>();
    let mut events = Vec::with_capacity(count);
    for record in storage.iter().take(count) {
        // SAFETY: `read` initialized every byte in each complete record.
        events.push(unsafe { record.assume_init_read() });
    }
    Ok(events)
}

fn dispatch_keyboard_events(
    state: &mut MeridianState,
    events: &[WsconsEvent],
    pressed_keys: &mut HashSet<u32>,
) {
    for event in events {
        let time = event_time_micros(event);
        match event.event_type {
            EVENT_KEY_DOWN if event.value >= 0 => {
                let key = event.value as u32;
                // wscons repeats held keys as additional DOWN records. Wayland
                // clients perform key repeat from repeat_info themselves, while
                // forwarding these records would retrigger compositor shortcuts
                // such as Super+Space on every repeat tick.
                if !pressed_keys.insert(key) {
                    continue;
                }
                state.process_input_event(InputEvent::<WsconsInput>::Keyboard {
                    event: WsconsKeyboardEvent {
                        time,
                        key,
                        state: KeyState::Pressed,
                        count: pressed_keys.len() as u32,
                    },
                });
            }
            EVENT_KEY_UP if event.value >= 0 => {
                let key = event.value as u32;
                pressed_keys.remove(&key);
                state.process_input_event(InputEvent::<WsconsInput>::Keyboard {
                    event: WsconsKeyboardEvent {
                        time,
                        key,
                        state: KeyState::Released,
                        count: pressed_keys.len() as u32,
                    },
                });
            }
            EVENT_ALL_KEYS_UP => {
                let mut keys = pressed_keys.drain().collect::<Vec<_>>();
                keys.sort_unstable();
                let mut remaining = keys.len() as u32;
                for key in keys {
                    remaining = remaining.saturating_sub(1);
                    state.process_input_event(InputEvent::<WsconsInput>::Keyboard {
                        event: WsconsKeyboardEvent {
                            time,
                            key,
                            state: KeyState::Released,
                            count: remaining,
                        },
                    });
                }
            }
            _ => {}
        }
    }
}

fn dispatch_pointer_events(state: &mut MeridianState, events: &[WsconsEvent]) {
    for event in events {
        let time = event_time_micros(event);
        match event.event_type {
            EVENT_MOUSE_DELTA_X => {
                state.process_input_event(InputEvent::<WsconsInput>::PointerMotion {
                    event: WsconsMotionEvent {
                        time,
                        dx: event.value as f64,
                        dy: 0.0,
                    },
                });
            }
            EVENT_MOUSE_DELTA_Y => {
                state.process_input_event(InputEvent::<WsconsInput>::PointerMotion {
                    event: WsconsMotionEvent {
                        time,
                        dx: 0.0,
                        // wscons positive Y points up; Wayland logical Y points down.
                        dy: -(event.value as f64),
                    },
                });
            }
            EVENT_MOUSE_DOWN | EVENT_MOUSE_UP if event.value >= 0 => {
                state.process_input_event(InputEvent::<WsconsInput>::PointerButton {
                    event: WsconsButtonEvent {
                        time,
                        button: event.value as u32,
                        state: if event.event_type == EVENT_MOUSE_DOWN {
                            ButtonState::Pressed
                        } else {
                            ButtonState::Released
                        },
                    },
                });
            }
            EVENT_MOUSE_DELTA_Z | EVENT_MOUSE_DELTA_W | EVENT_HSCROLL | EVENT_VSCROLL => {
                let horizontal = matches!(event.event_type, EVENT_MOUSE_DELTA_W | EVENT_HSCROLL);
                state.process_input_event(InputEvent::<WsconsInput>::PointerAxis {
                    event: WsconsAxisEvent {
                        time,
                        axis: if horizontal {
                            Axis::Horizontal
                        } else {
                            Axis::Vertical
                        },
                        value: event.value as f64,
                        precise: matches!(event.event_type, EVENT_HSCROLL | EVENT_VSCROLL),
                    },
                });
            }
            _ => {}
        }
    }
}

fn event_time_micros(event: &WsconsEvent) -> u64 {
    let seconds = event.time.tv_sec.max(0) as u64;
    let nanoseconds = event.time.tv_nsec.clamp(0, 999_999_999) as u64;
    seconds
        .saturating_mul(1_000_000)
        .saturating_add(nanoseconds / 1_000)
}

fn wscons_button_code(button: u32) -> u32 {
    match button {
        0 => BTN_LEFT,
        1 => BTN_MIDDLE,
        2 => BTN_RIGHT,
        other => BTN_SIDE.saturating_add(other - 3),
    }
}

fn wscons_key_code(key: u32) -> u32 {
    // OpenBSD pckbd uses extended XT positions for these keys while Smithay's
    // backend contract expects Linux input-event codes. The ordinary key block
    // is already numerically identical, so translate only the extended keys.
    match key {
        127 => 119,      // Pause
        156 => 96,       // KP Enter
        157 => 97,       // Right Ctrl
        160 => 113,      // Mute
        170 | 183 => 99, // Print Screen / SysRq
        174 => 114,      // Volume Down
        176 => 115,      // Volume Up
        181 => 98,       // KP Divide
        184 => 100,      // Right Alt
        199 => 102,      // Home
        200 => 103,      // Up
        201 => 104,      // Page Up
        203 => 105,      // Left
        205 => 106,      // Right
        207 => 107,      // End
        208 => 108,      // Down
        209 => 109,      // Page Down
        210 => 110,      // Insert
        211 => 111,      // Delete
        219 => 125,      // Left Meta / Super
        220 => 126,      // Right Meta / Super
        221 => 127,      // Menu
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_primary_wscons_buttons_to_linux_codes_expected_by_smithay() {
        assert_eq!(wscons_button_code(0), BTN_LEFT);
        assert_eq!(wscons_button_code(1), BTN_MIDDLE);
        assert_eq!(wscons_button_code(2), BTN_RIGHT);
        assert_eq!(wscons_button_code(3), BTN_SIDE);
    }

    #[test]
    fn maps_extended_pckbd_keys_to_evdev_codes_expected_by_xkb() {
        for (wscons, evdev) in [
            (127, 119),
            (156, 96),
            (157, 97),
            (160, 113),
            (170, 99),
            (174, 114),
            (176, 115),
            (181, 98),
            (183, 99),
            (184, 100),
            (199, 102),
            (200, 103),
            (201, 104),
            (203, 105),
            (205, 106),
            (207, 107),
            (208, 108),
            (209, 109),
            (210, 110),
            (211, 111),
            (219, 125),
            (220, 126),
            (221, 127),
        ] {
            assert_eq!(wscons_key_code(wscons), evdev);
        }
        assert_eq!(wscons_key_code(30), 30);
    }

    #[test]
    fn converts_wscons_timespec_to_microseconds() {
        let event = WsconsEvent {
            event_type: EVENT_KEY_DOWN,
            value: 1,
            time: libc::timespec {
                tv_sec: 2,
                tv_nsec: 345_678_000,
            },
        };
        assert_eq!(event_time_micros(&event), 2_345_678);
    }
}
