//! Native OpenBSD wscons input for the privileged login surface.

use std::{
    collections::HashSet,
    fs::{File, OpenOptions},
    io,
    mem::{size_of, MaybeUninit},
    os::{
        fd::{AsRawFd, RawFd},
        unix::fs::OpenOptionsExt,
    },
    path::{Path, PathBuf},
};

use tracing::{info, warn};

use super::{KeyAction, Keyboard, PointerAction, PointerState};

const DEFAULT_KEYBOARD_DEVICE: &str = "/dev/wskbd0";
const DEFAULT_POINTER_DEVICE: &str = "/dev/wsmouse0";

const EVENT_KEY_UP: u32 = 1;
const EVENT_KEY_DOWN: u32 = 2;
const EVENT_ALL_KEYS_UP: u32 = 3;
const EVENT_MOUSE_DOWN: u32 = 5;
const EVENT_MOUSE_DELTA_X: u32 = 6;
const EVENT_MOUSE_DELTA_Y: u32 = 7;

#[repr(C)]
#[derive(Clone, Copy)]
struct WsconsEvent {
    event_type: libc::c_uint,
    value: libc::c_int,
    time: libc::timespec,
}

pub struct KeyboardDevice {
    file: File,
    pressed: HashSet<u32>,
}

pub struct PointerDevice {
    file: File,
}

pub fn open_keyboards() -> io::Result<Vec<KeyboardDevice>> {
    let path = device_path("MERIDIAN_WSKBD_DEVICE", DEFAULT_KEYBOARD_DEVICE);
    let file = open_device(&path)?;
    info!(path = %path.display(), "OpenBSD login keyboard open");
    Ok(vec![KeyboardDevice {
        file,
        pressed: HashSet::new(),
    }])
}

pub fn open_pointers() -> io::Result<Vec<PointerDevice>> {
    let path = device_path("MERIDIAN_WSMOUSE_DEVICE", DEFAULT_POINTER_DEVICE);
    let file = open_device(&path)?;
    info!(path = %path.display(), "OpenBSD login pointer open");
    Ok(vec![PointerDevice { file }])
}

pub fn poll_keyboards(devices: &mut [KeyboardDevice], keyboard: &mut Keyboard) -> Vec<KeyAction> {
    let mut actions = Vec::new();
    for device in devices {
        match read_events(device.file.as_raw_fd()) {
            Ok(events) => {
                for event in events {
                    match event.event_type {
                        EVENT_KEY_DOWN if event.value >= 0 => {
                            let key = wscons_key_code(event.value as u32);
                            // wscons emits repeated KEY_DOWN records while a key is held.
                            // Login fields deliberately accept one action per physical
                            // press: kernel repeat is too eager for short credentials and
                            // can append characters before the matching KEY_UP is polled.
                            if register_key_down(&mut device.pressed, key) {
                                if let Some(action) = keyboard.process(key as u16, 1) {
                                    actions.push(action);
                                }
                            }
                        }
                        EVENT_KEY_UP if event.value >= 0 => {
                            let key = wscons_key_code(event.value as u32);
                            device.pressed.remove(&key);
                            let _ = keyboard.process(key as u16, 0);
                        }
                        EVENT_ALL_KEYS_UP => {
                            for key in device.pressed.drain() {
                                let _ = keyboard.process(key as u16, 0);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Err(error) => warn!(%error, "failed to read OpenBSD login keyboard"),
        }
    }
    actions
}

pub fn poll_pointers(
    devices: &mut [PointerDevice],
    pointer: &mut PointerState,
) -> Vec<PointerAction> {
    let mut actions = Vec::new();
    for device in devices {
        match read_events(device.file.as_raw_fd()) {
            Ok(events) => {
                for event in events {
                    match event.event_type {
                        EVENT_MOUSE_DELTA_X => pointer.move_relative(event.value, 0),
                        EVENT_MOUSE_DELTA_Y => pointer.move_relative(0, -event.value),
                        EVENT_MOUSE_DOWN if event.value == 0 => {
                            actions.push(PointerAction::LeftPress {
                                x: pointer.x,
                                y: pointer.y,
                            });
                        }
                        _ => {}
                    }
                }
            }
            Err(error) => warn!(%error, "failed to read OpenBSD login pointer"),
        }
    }
    actions
}

fn device_path(variable: &str, fallback: &str) -> PathBuf {
    std::env::var_os(variable)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(fallback))
}

fn open_device(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOCTTY | libc::O_NONBLOCK)
        .open(path)
}

fn read_events(fd: RawFd) -> io::Result<Vec<WsconsEvent>> {
    const EVENT_BATCH: usize = 32;
    let mut storage = [MaybeUninit::<WsconsEvent>::uninit(); EVENT_BATCH];
    let byte_capacity = storage.len() * size_of::<WsconsEvent>();
    // SAFETY: storage is writable for byte_capacity bytes and fd stays valid.
    let bytes_read = unsafe {
        libc::read(
            fd,
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
    Ok(storage
        .iter()
        .take(count)
        // SAFETY: read initialized every byte in each complete record.
        .map(|record| unsafe { record.assume_init_read() })
        .collect())
}

fn wscons_key_code(key: u32) -> u32 {
    match key {
        127 => 119,
        156 => 96,
        157 => 97,
        160 => 113,
        170 | 183 => 99,
        174 => 114,
        176 => 115,
        181 => 98,
        184 => 100,
        199 => 102,
        200 => 103,
        201 => 104,
        203 => 105,
        205 => 106,
        207 => 107,
        208 => 108,
        209 => 109,
        210 => 110,
        211 => 111,
        219 => 125,
        220 => 126,
        221 => 127,
        other => other,
    }
}

fn register_key_down(pressed: &mut HashSet<u32>, key: u32) -> bool {
    pressed.insert(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_extended_pckbd_keys_to_evdev_codes_expected_by_xkb() {
        assert_eq!(wscons_key_code(156), 96);
        assert_eq!(wscons_key_code(184), 100);
        assert_eq!(wscons_key_code(200), 103);
        assert_eq!(wscons_key_code(30), 30);
    }

    #[test]
    fn suppresses_repeated_key_down_until_release() {
        let mut pressed = HashSet::new();
        assert!(register_key_down(&mut pressed, 32));
        assert!(!register_key_down(&mut pressed, 32));
        pressed.remove(&32);
        assert!(register_key_down(&mut pressed, 32));
    }
}
