// meridian-login — Phase 7: spawn the user-side compositor after PAM ok.
//
// Animation timeline:
//   0.00..0.20s  hold the settle frame (handover-friendly)
//   0.20..1.40s  compass dims toward watermark; glow falls + grows
//   1.40..1.70s  card outline fades in over the glow
//   1.70..2.00s  card content (title, labels, boxes) fades in
//   2.00s..      keyboard input loop — typed chars appear in the focused
//                field, Tab cycles focus, Enter submits, Esc cancels.
//
// Phase 5b does not actually authenticate; Submit just logs the username
// and password length and exits. Phase 6 will wire PAM.

mod auth;
mod input;
mod session;
mod visual;

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{mpsc, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use drm::buffer::DrmFourcc;
use drm::control::{connector, ClipRect, Device as ControlDevice};
use drm::Device as DrmDevice;

use meridian_compass_render::{CompassPainter, Fonts, Style, TextStyle};
use meridian_config::{ThemeConfig, ThemeManager, ThemeSurface};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, PixmapMut, Stroke, Transform};
use tracing::{info, warn};
use zeroize::Zeroizing;

use auth::{start_auth_session, AuthBackend, AuthDriver, AuthResult};
use input::{
    open_keyboards, open_pointers, poll_keyboards, poll_pointers, KeyAction, Keyboard,
    KeyboardStatus, PointerAction, PointerState,
};
use meridian_boot_common::{
    cleanup_socket_path, read_appearance, secure_socket_permissions, select_boot_mode, Appearance,
    SocketIdentity, BOOTSPLASH_SOCKET_PATH, LOGIN_SOCKET_PATH,
};
use visual::LoginBackdrop;

const BOOTSPLASH_SOCKET_ENV: &str = "BOOTSPLASH_SOCKET";
const LOGIN_DRM_CARD_ENV: &str = "MERIDIAN_LOGIN_DRM_CARD";
const DEFAULT_DRM_CARD: &str = "/dev/dri/card0";
// No client-side sleep needed: bootsplash's `handover` ack is now
// synchronous — it only writes "ok handover" after drmDropMaster has
// completed, so our set_crtc is race-free immediately after the call
// returns.

// Phase 8: IPC server that the spawned compositor uses to hand the screen
// over and announce its first committed frame. Mirror of the bootsplash IPC
// model — see `bootsplash_handover` / `bootsplash_exit` in this file.
/// Maximum time we wait for the compositor to send `handover` before we
/// give up and release DRM anyway. Without this fallback a buggy or
/// pre-Phase-8 compositor would leave the user staring at the frozen
/// login frame indefinitely.
const HANDOVER_DEADLINE: Duration = Duration::from_secs(5);

// Animation parameters
const CARD_FADE_START_MS: u64 = 1400;
const CARD_FADE_END_MS: u64 = 1700;
const UI_FADE_START_MS: u64 = 1700;
const UI_FADE_END_MS: u64 = 2000;
const MAX_FIELD_LEN: usize = 64;
const POWER_CONFIRM_WINDOW: Duration = Duration::from_secs(3);
const SECURITY_KEY_POLL_INTERVAL: Duration = Duration::from_secs(1);
const YUBICO_USB_VENDOR_ID: &str = "1050";
const SMARTCARD_AUTHFILE: &str = "/etc/Yubico/u2f_keys";

type Rect = (f32, f32, f32, f32);
type PowerButtonRects = (Rect, Rect);

const CARD_PAD: f32 = 48.0;

/// Card / control corner radii come from the active theme's central decoration
/// defaults (one source for every surface), not greeter-local literals. The
/// card reads as a modal surface; the inputs/buttons as controls.
fn card_radius() -> f32 {
    login_theme()
        .decorations
        .surface_radius(meridian_config::ThemeSurface::Modal)
}

fn control_radius() -> f32 {
    login_theme()
        .decorations
        .surface_radius(meridian_config::ThemeSurface::Control)
}
const CARD_SHADOW_BLUR: f32 = meridian_tokens::Elevation::LAUNCHER.blur;
const CARD_SHADOW_ALPHA: f32 = meridian_tokens::Elevation::LAUNCHER.alpha;
const CARD_SHADOW_OFFSET_Y: f32 = meridian_tokens::Elevation::LAUNCHER.offset_y as f32;
const BRAND_MARK_OFFSET_Y: f32 = 20.0;
const TITLE_OFFSET_Y: f32 = 90.0;
const SUBTITLE_OFFSET_Y: f32 = 124.0;
const USER_BOX_OFFSET_Y: f32 = 164.0;
const PASSWORD_BOX_OFFSET_Y: f32 = 234.0;
const LOGIN_BUTTON_OFFSET_Y: f32 = 309.0;
const SMARTCARD_PIN_BOX_OFFSET_Y: f32 = 244.0;
const SMARTCARD_PIN_WIDTH: f32 = 168.0;
const INPUT_BOX_HEIGHT: f32 = 52.0;
const INPUT_TEXT_PAD_X: f32 = 50.0;
const INPUT_BASELINE_PAD_BOTTOM: f32 = 17.0;
const LOGIN_BUTTON_HEIGHT: f32 = 50.0;
const POWER_BUTTON_WIDTH: f32 = 96.0;
const POWER_BUTTON_HEIGHT: f32 = 32.0;
const POWER_BUTTON_GAP: f32 = 10.0;
const POWER_BUTTON_EDGE_PAD: f32 = 24.0;
const HINT_OFFSET_Y: f32 = 24.0;

// Card shake animation on auth failure (classic "wrong password" feedback)
const FAILED_DURATION_MS: u64 = 600;
const FAILED_SHAKE_FREQ_HZ: f32 = 14.0;
const FAILED_SHAKE_AMPLITUDE: f32 = 14.0;

struct Card(File);

/// Static greeter rendering resources retained while the supervised desktop
/// session runs. Reusing them lets the next greeter modeset immediately after
/// compositor exit instead of decoding and filtering the wallpaper again.
struct GreeterAssets {
    width: u32,
    height: u32,
    painter: CompassPainter<'static>,
    backdrop: LoginBackdrop,
}

impl GreeterAssets {
    fn new(width: u32, height: u32) -> Result<Self, Box<dyn std::error::Error>> {
        let painter = if light_appearance() {
            CompassPainter::new(Fonts::quompacc())?.with_style(Style::chart())
        } else {
            CompassPainter::new(Fonts::quompacc())?
        };
        Ok(Self {
            width,
            height,
            painter,
            backdrop: LoginBackdrop::new(width, height)?,
        })
    }

    fn matches_output(&self, width: u32, height: u32) -> bool {
        self.width == width && self.height == height
    }
}

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}
impl DrmDevice for Card {}
impl ControlDevice for Card {}

/// True if any connector on this card reports a connected display.
fn card_drives_a_display(card: &Card) -> bool {
    let Ok(res) = card.resource_handles() else {
        return false;
    };
    res.connectors().iter().any(|&h| {
        matches!(
            card.get_connector(h, false),
            Ok(c) if c.state() == connector::State::Connected
        )
    })
}

/// Pick the DRM card that actually drives a display, and return its path plus
/// the opened handle. An explicit `MERIDIAN_LOGIN_DRM_CARD` wins; otherwise we
/// probe every `/dev/dri/cardN` and take the first whose connectors include a
/// connected display.
///
/// This must auto-detect rather than trust a fixed node: the kernel's `cardN`
/// numbering is NOT stable across boots. On a hybrid-GPU laptop (Intel iGPU +
/// discrete GPU) the two can swap card numbers from one boot to the next — and
/// only the GPU wired to the panel has a connected connector — so a hardcoded
/// `/dev/dri/card0` points at the headless GPU on the unlucky boot and the
/// login screen never appears. The compositor's `select_gpu` already works this
/// way; this gives the greeter the same robustness.
fn open_display_card() -> Result<(String, Card), Box<dyn std::error::Error>> {
    if let Ok(path) = std::env::var(LOGIN_DRM_CARD_ENV) {
        let card = Card(OpenOptions::new().read(true).write(true).open(&path)?);
        return Ok((path, card));
    }

    let mut cards: Vec<PathBuf> = fs::read_dir("/dev/dri")
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("card"))
        })
        .collect();
    cards.sort();

    for path in &cards {
        let Ok(file) = OpenOptions::new().read(true).write(true).open(path) else {
            continue;
        };
        let card = Card(file);
        if card_drives_a_display(&card) {
            return Ok((path.display().to_string(), card));
        }
    }

    // Last resort: keep behaviour defined if nothing reported a connected
    // connector (e.g. a connector probe raced very early boot).
    let card = Card(
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(DEFAULT_DRM_CARD)?,
    );
    Ok((DEFAULT_DRM_CARD.to_string(), card))
}

#[derive(Default)]
struct LoginUiState {
    username: String,
    /// Wrapped in Zeroizing so the bytes are wiped on drop. We never log or
    /// persist this value.
    password: Zeroizing<String>,
    focus: Field,
    phase: InputPhase,
    /// Set while [`InputPhase::Authenticating`] — a background thread holds
    /// a copy of the credentials and posts its result here. Polling this
    /// every frame keeps the render loop responsive even while PAM blocks.
    auth_rx: Option<mpsc::Receiver<AuthResult>>,
    /// Phase 7b: alive for the duration of a successful login. Owns the PAM
    /// handle in a worker thread; closing it (or dropping it) calls
    /// close_session + pam_end on the worker side. main pulls this out
    /// after `AuthResult::Ok` and keeps it until the compositor exits.
    auth_driver: Option<AuthDriver>,
    /// Snapshot of pam_getenvlist captured by the auth worker right after
    /// pam_open_session. Forwarded into the compositor environment so it
    /// inherits XDG_SESSION_ID / XDG_SEAT / XDG_VTNR from pam_systemd.
    pam_env: Vec<(String, String)>,
    pending_power: Option<PendingPowerAction>,
    keyboard_status: KeyboardStatus,
    security_key_present: bool,
    smartcard_user: Option<String>,
}

#[derive(Default, Clone, Copy, Debug)]
enum InputPhase {
    /// Normal editing — typing fills the focused field.
    #[default]
    Editing,
    /// PAM is running in a background thread. Input is ignored, hint shows
    /// "Anmelden …" so the user has immediate feedback after Enter.
    Authenticating,
    /// Auth just rejected the last submit. Card shakes, fields are cleared,
    /// hint changes briefly. After FAILED_DURATION_MS we go back to Editing.
    Failed(Instant),
}

#[derive(Default, Clone, Copy, PartialEq, Debug)]
enum Field {
    #[default]
    Username,
    Password,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum ControlFlow {
    Continue,
    Submit,
    Cancel,
    PowerOff,
    Reboot,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum ClickTarget {
    Field(Field),
    Submit,
    PowerOff,
    Reboot,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum PowerAction {
    PowerOff,
    Reboot,
}

#[derive(Clone, Copy, Debug)]
struct PendingPowerAction {
    action: PowerAction,
    since: Instant,
}

impl PowerAction {
    fn control_flow(self) -> ControlFlow {
        match self {
            PowerAction::PowerOff => ControlFlow::PowerOff,
            PowerAction::Reboot => ControlFlow::Reboot,
        }
    }
}

impl PendingPowerAction {
    fn is_active(self) -> bool {
        self.since.elapsed() <= POWER_CONFIRM_WINDOW
    }
}

include!("main/state.rs");
include!("main/runtime.rs");
include!("main/animation.rs");
include!("main/geometry_and_buttons.rs");
include!("main/ui.rs");
include!("main/controls.rs");
include!("main/handover_ipc.rs");

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
