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

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use drm::buffer::DrmFourcc;
use drm::control::{connector, ClipRect, Device as ControlDevice};
use drm::Device as DrmDevice;

use meridian_compass_render::{CompassPainter, Fonts, FrameOpts, TextStyle, SETTLE_T};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, PixmapMut, Stroke, Transform};
use tracing::{info, warn};
use zeroize::Zeroizing;

use auth::{start_auth_session, AuthDriver, AuthResult};
use input::{
    open_keyboards, open_pointers, poll_keyboards, poll_pointers, KeyAction, Keyboard,
    KeyboardStatus, PointerAction, PointerState,
};
use meridian_boot_common::{
    cleanup_socket_path, secure_socket_permissions, select_boot_mode, SocketIdentity,
};

const BOOTSPLASH_SOCKET_ENV: &str = "BOOTSPLASH_SOCKET";
const BOOTSPLASH_SOCKET: &str = "/run/bootsplash.sock";
const LOGIN_DRM_CARD_ENV: &str = "MERIDIAN_LOGIN_DRM_CARD";
const DEFAULT_DRM_CARD: &str = "/dev/dri/card0";
// No client-side sleep needed: bootsplash's `handover` ack is now
// synchronous — it only writes "ok handover" after drmDropMaster has
// completed, so our set_crtc is race-free immediately after the call
// returns. Kept the constant for backward-compat documentation but it
// is unused.
#[allow(dead_code)]
const HANDOVER_SETTLE_MS: u64 = 0;

// Phase 8: IPC server that the spawned compositor uses to hand the screen
// over and announce its first committed frame. Mirror of the bootsplash IPC
// model — see `bootsplash_handover` / `bootsplash_exit` in this file.
const LOGIN_SOCKET: &str = "/run/meridian-login.sock";
/// Maximum time we wait for the compositor to send `handover` before we
/// give up and release DRM anyway. Without this fallback a buggy or
/// pre-Phase-8 compositor would leave the user staring at the frozen
/// login frame indefinitely.
const HANDOVER_DEADLINE: Duration = Duration::from_secs(5);

// Animation parameters
const WATERMARK_START_MS: u64 = 200;
const WATERMARK_END_MS: u64 = 1400;
const WATERMARK_FINAL_ALPHA: u8 = 180;
const FALL_END_MS: u64 = 1400;
const CARD_FADE_START_MS: u64 = 1400;
const CARD_FADE_END_MS: u64 = 1700;
const UI_FADE_START_MS: u64 = 1700;
const UI_FADE_END_MS: u64 = 2000;
const GLOW_HIDE_MS: u64 = 1700;
const GLOW_FINAL_SCALE: f32 = 4.0;
const MAX_FIELD_LEN: usize = 128;
const POWER_CONFIRM_WINDOW: Duration = Duration::from_secs(3);

type Rect = (f32, f32, f32, f32);
type PowerButtonRects = (Rect, Rect);

const CARD_PAD: f32 = 32.0;
const METRO_STRIPE_HEIGHT: f32 = 4.0;
const TITLE_OFFSET_Y: f32 = 31.0;
const USER_LABEL_OFFSET_Y: f32 = 76.0;
const USER_BOX_OFFSET_Y: f32 = 88.0;
const PASSWORD_LABEL_OFFSET_Y: f32 = 138.0;
const PASSWORD_BOX_OFFSET_Y: f32 = 150.0;
const INPUT_BOX_HEIGHT: f32 = 36.0;
const INPUT_TEXT_PAD_X: f32 = 12.0;
const INPUT_BASELINE_PAD_BOTTOM: f32 = 12.0;
const POWER_BUTTON_WIDTH: f32 = 116.0;
const POWER_BUTTON_HEIGHT: f32 = 34.0;
const POWER_BUTTON_GAP: f32 = 12.0;
const POWER_BUTTON_BOTTOM_PAD: f32 = 32.0;
const HINT_POWER_GAP: f32 = 16.0;

// Card shake animation on auth failure (classic "wrong password" feedback)
const FAILED_DURATION_MS: u64 = 600;
const FAILED_SHAKE_FREQ_HZ: f32 = 14.0;
const FAILED_SHAKE_AMPLITUDE: f32 = 14.0;

struct Card(File);

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}
impl DrmDevice for Card {}
impl ControlDevice for Card {}

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

impl LoginUiState {
    fn apply(&mut self, action: KeyAction) -> ControlFlow {
        self.pending_power = None;
        match action {
            KeyAction::Insert(s) => {
                let target: &mut String = match self.focus {
                    Field::Username => &mut self.username,
                    Field::Password => &mut self.password,
                };
                if target.chars().count() + s.chars().count() <= MAX_FIELD_LEN {
                    target.push_str(&s);
                }
                ControlFlow::Continue
            }
            KeyAction::Backspace => {
                let target: &mut String = match self.focus {
                    Field::Username => &mut self.username,
                    Field::Password => &mut self.password,
                };
                target.pop();
                ControlFlow::Continue
            }
            KeyAction::CycleFocus => {
                self.focus = match self.focus {
                    Field::Username => Field::Password,
                    Field::Password => Field::Username,
                };
                ControlFlow::Continue
            }
            KeyAction::CycleFocusBack => {
                self.focus = match self.focus {
                    Field::Username => Field::Password,
                    Field::Password => Field::Username,
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
        let username = self.username.clone();
        let password = Zeroizing::new(self.password.to_string());
        let (rx, driver) = start_auth_session(username, password);
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
        // Keep the username so retrying only requires the password again.
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
            InputPhase::Editing if self.keyboard_status.caps_lock => {
                format!("Caps Lock aktiv - Layout {layout}")
            }
            InputPhase::Editing => format!("Enter zum Anmelden - Layout {layout}"),
            InputPhase::Authenticating => "Anmelden …".to_string(),
            InputPhase::Failed(_) => "Passwort erneut eingeben".to_string(),
        }
    }
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    info!("meridian-login starting (Phase 7)");

    match bootsplash_handover() {
        Ok(()) => info!("bootsplash handover acked (master released)"),
        Err(e) => warn!(error = %e, "bootsplash handover failed (not running?); proceeding"),
    }

    let drm_card =
        std::env::var(LOGIN_DRM_CARD_ENV).unwrap_or_else(|_| DEFAULT_DRM_CARD.to_string());
    info!(path = %drm_card, "opening login DRM card");
    let card = Card(OpenOptions::new().read(true).write(true).open(&drm_card)?);

    let res = card.resource_handles()?;
    let conn_info = res
        .connectors()
        .iter()
        .filter_map(|&h| match card.get_connector(h, false) {
            Ok(connector) => Some(connector),
            Err(err) => {
                warn!(connector = ?h, error = %err, "failed to inspect DRM connector");
                None
            }
        })
        .find(|c| c.state() == connector::State::Connected)
        .ok_or("no connected connector")?;
    let mode = select_boot_mode(conn_info.modes()).ok_or("connector has no usable mode")?;
    let (w, h) = mode.size();
    let (w, h) = (w as u32, h as u32);
    info!(width = w, height = h, refresh = mode.vrefresh(), "drm mode");

    let crtc = if let Some(enc_h) = conn_info.current_encoder() {
        card.get_encoder(enc_h)?.crtc()
    } else {
        None
    }
    .or_else(|| res.crtcs().first().copied())
    .ok_or("no CRTC available")?;

    let mut db = card.create_dumb_buffer((w, h), DrmFourcc::Xrgb8888, 32)?;
    let fb = card.add_framebuffer(&db, 24, 32)?;

    let painter = CompassPainter::new(Fonts::quompacc())?;

    // Pre-fill the dumb buffer with the settle frame BEFORE set_crtc so
    // the kernel never scans out a zeroed (black) buffer. Without this,
    // the modeset would briefly show black between "bootsplash fb
    // unmapped" and "first dirty_framebuffer push" → visible flash.
    {
        let mut mapping = card.map_dumb_buffer(&mut db)?;
        let buf = mapping.as_mut();
        let mut pm = PixmapMut::from_bytes(buf, w, h).ok_or("pixmap bind failed")?;
        // force_needle_north matches bootsplash's final handover frame
        // (also rendered with force_needle_north=true), so the visual
        // transition has the needle at exactly the same angle across the
        // process boundary.
        painter.render(
            &mut pm,
            w as f32,
            h as f32,
            SETTLE_T,
            &FrameOpts {
                force_needle_north: true,
                ..Default::default()
            },
        );
        for px in buf.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
    }

    card.set_crtc(crtc, Some(fb), (0, 0), &[conn_info.handle()], Some(mode))?;
    let clip = ClipRect::new(0, 0, w as u16, h as u16);
    let _ = card.dirty_framebuffer(fb, &[clip]);
    info!("settle frame committed");

    match bootsplash_exit() {
        Ok(()) => info!("bootsplash exit signalled"),
        Err(e) => warn!(error = %e, "bootsplash exit signal failed"),
    }

    let mut ui_state = LoginUiState::default();
    let mut keyboards = open_keyboards()?;
    let mut pointers = open_pointers()?;
    let mut keyboard = Keyboard::new().map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    ui_state.keyboard_status = keyboard.status();
    let mut pointer = PointerState::new(w, h);

    let exit = run_animation(
        &card,
        &mut db,
        fb,
        &painter,
        w,
        h,
        mode.vrefresh().max(60),
        &mut ui_state,
        &mut keyboards,
        &mut keyboard,
        &mut pointers,
        &mut pointer,
    )?;

    // Release the keyboards BEFORE we spawn the compositor and enter the
    // handover wait. input.rs grabs every keyboard device with
    // EVIOCGRAB so the password never leaks to the kernel TTY; if we
    // keep the fds open while the compositor runs, libinput in the
    // compositor sees the devices but receives zero key events (the
    // grab is per-fd and only releases on close). Dropping here lets
    // the user type into apps immediately after auth.
    drop(keyboards);
    drop(keyboard);
    drop(pointers);
    info!("released input devices");

    if matches!(exit, ControlFlow::PowerOff | ControlFlow::Reboot) {
        match card.release_master_lock() {
            Ok(()) => info!("released drm master before power action"),
            Err(e) => warn!(error = %e, "release_master before power action failed"),
        }
        drop(card);
        run_power_action(exit);
        return Ok(());
    }

    // On successful auth, spawn the compositor as the authenticated user
    // BEFORE releasing master so the new process is already running by the
    // time we let go of the display. Phase 8 will add an IPC handshake to
    // hold the buffer until the compositor's first frame is committed.
    //
    // Phase 7b: the PAM session (held by ui_state.auth_driver) must stay
    // alive for as long as the compositor runs — pam_systemd's logind
    // session is what backs libseat. So after spawning we wait() on the
    // compositor, and only then drop the AuthDriver to close the session.
    let username = ui_state.username.clone();
    let auth_driver = ui_state.auth_driver.take();
    let pam_env = std::mem::take(&mut ui_state.pam_env);

    let compositor_child = match exit {
        ControlFlow::Submit => {
            info!(
                user = %username,
                pam_env_count = pam_env.len(),
                "auth ok — launching compositor"
            );
            match session::launch_compositor_for(&username, &pam_env) {
                Ok(child) => {
                    info!(pid = child.id(), "compositor spawned");
                    Some(child)
                }
                Err(e) => {
                    warn!(error = %e, "compositor spawn failed");
                    None
                }
            }
        }
        ControlFlow::Cancel => {
            info!("login cancelled");
            None
        }
        ControlFlow::PowerOff | ControlFlow::Reboot => None,
        ControlFlow::Continue => {
            info!("inactivity timeout reached");
            None
        }
    };

    // Phase 8: keep the DRM master + framebuffer alive until the compositor
    // signals it is ready to take the screen (via `handover` on the login
    // IPC socket). When that signal arrives we release master + close the
    // fd, which lets the compositor's libseat acquire cleanly. If the
    // compositor crashes early or never signals, the HANDOVER_DEADLINE
    // fallback releases anyway so we don't strand the user on a frozen
    // login frame.
    let (ipc_tx, ipc_rx) = mpsc::channel::<IpcEvent>();
    let ipc_socket_path = match compositor_child.as_ref() {
        Some(_) => match nix::unistd::User::from_name(&username) {
            Ok(Some(user)) => {
                match spawn_login_ipc_server(ipc_tx, user.uid.as_raw(), user.gid.as_raw()) {
                    Ok(p) => Some(p),
                    Err(e) => {
                        warn!(error = %e, "login ipc server bind failed; releasing drm immediately");
                        None
                    }
                }
            }
            Ok(None) => {
                warn!(user = %username, "user lookup returned None; skipping ipc server");
                None
            }
            Err(e) => {
                warn!(error = %e, user = %username, "user lookup failed; skipping ipc server");
                None
            }
        },
        // No compositor to hand over to — skip the IPC server, fall through
        // to immediate release below.
        None => None,
    };

    let mut card_opt: Option<Card> = Some(card);
    let mut master_released = false;
    let mut first_frame_seen = false;
    let spawn_wait_started_at = Instant::now();
    let mut handover_received_at: Option<Instant> = None;

    // Two-stage release mirroring the bootsplash → login pattern:
    //   1. on `handover`: drop DRM master but keep the fd open so the
    //      scanout buffer stays referenced by the kernel — the visible
    //      pixels do not change yet.
    //   2. on `exit`: close the fd. By now the compositor's first frame
    //      is on screen and owns the scanout, so closing our fb is safe.
    // Without (1), the compositor's first commit fights us for master.
    // Without (2)-being-deferred-to-exit, the kernel may drop our fb
    // before the compositor's first commit lands → black flash.
    if let Some(mut child) = compositor_child {
        info!(pid = child.id(), "waiting for compositor handover + exit");
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    info!(pid = child.id(), status = ?status, "compositor exited");
                    if let Some(card) = card_opt.take() {
                        if !master_released {
                            warn!("compositor exited without handover; releasing drm now");
                            let _ = card.release_master_lock();
                        }
                        drop(card);
                    }
                    break;
                }
                Ok(None) => {}
                Err(e) => {
                    warn!(error = %e, "try_wait on compositor failed");
                    break;
                }
            }

            match ipc_rx.recv_timeout(Duration::from_millis(50)) {
                Ok(IpcEvent::Handover) => {
                    if !master_released {
                        if let Some(card) = card_opt.as_ref() {
                            match card.release_master_lock() {
                                Ok(()) => {
                                    info!(
                                        spawn_to_handover_ms =
                                            spawn_wait_started_at.elapsed().as_millis() as u64,
                                        "ipc handover: released drm master (fd kept alive)"
                                    );
                                }
                                Err(e) => warn!(error = %e, "release_master failed"),
                            }
                        }
                        master_released = true;
                    }
                    handover_received_at.get_or_insert_with(Instant::now);
                }
                Ok(IpcEvent::Exit) => {
                    if !first_frame_seen {
                        info!(
                            spawn_to_first_frame_ms =
                                spawn_wait_started_at.elapsed().as_millis() as u64,
                            handover_to_first_frame_ms =
                                handover_received_at.map(|at| at.elapsed().as_millis() as u64),
                            "ipc exit: compositor first frame on screen; closing card0 fd"
                        );
                        first_frame_seen = true;
                    }
                    if let Some(card) = card_opt.take() {
                        drop(card);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if spawn_wait_started_at.elapsed() >= HANDOVER_DEADLINE {
                        if !master_released {
                            if let Some(card) = card_opt.as_ref() {
                                warn!(
                                    deadline_s = HANDOVER_DEADLINE.as_secs(),
                                    "handover deadline missed; releasing drm master"
                                );
                                let _ = card.release_master_lock();
                            }
                            master_released = true;
                        }
                        if !first_frame_seen {
                            if let Some(card) = card_opt.take() {
                                warn!("handover deadline missed; closing card0 fd");
                                drop(card);
                            }
                            first_frame_seen = true;
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    warn!("login ipc thread vanished; falling back to immediate release");
                    if let Some(card) = card_opt.take() {
                        let _ = card.release_master_lock();
                        drop(card);
                    }
                    master_released = true;
                    first_frame_seen = true;
                }
            }
        }
    } else if exit == ControlFlow::Submit {
        // Auth ok but spawn failed — don't leave the session dangling.
        warn!("auth ok but compositor not spawned; tearing session down immediately");
        if let Some(card) = card_opt.take() {
            let _ = card.release_master_lock();
            drop(card);
        }
    } else {
        // Cancel / inactivity — release master and exit cleanly.
        if let Some(card) = card_opt.take() {
            let _ = card.release_master_lock();
            drop(card);
        }
    }

    if let Some((path, identity)) = ipc_socket_path {
        match cleanup_socket_path(&path, identity) {
            Ok(true) | Ok(false) => {}
            Err(err) => warn!(path = %path.display(), error = %err, "login ipc cleanup failed"),
        }
    }

    if let Some(driver) = auth_driver {
        info!(user = %username, "closing PAM session");
        driver.close();
    }

    info!("meridian-login exiting");
    Ok(())
}

struct AnimFrame {
    watermark_alpha: u8,
    glow_visible: bool,
    glow_pos: (f32, f32),
    glow_scale: f32,
    card_alpha: f32,
    ui_alpha: f32,
}

fn compute_anim_frame(t_anim_secs: f32, painter: &CompassPainter, w: f32, h: f32) -> AnimFrame {
    let t_ms = (t_anim_secs * 1000.0) as u64;

    let watermark_alpha = ramp_u8(
        t_ms,
        WATERMARK_START_MS,
        WATERMARK_END_MS,
        0,
        WATERMARK_FINAL_ALPHA,
    );

    let origin = painter.north_glow_position(w, h, SETTLE_T);
    let target = (w / 2.0, h / 2.0);
    let p_fall = (t_ms as f32 / FALL_END_MS as f32).clamp(0.0, 1.0);
    let p_eased = p_fall * p_fall;
    let glow_pos = (
        origin.0 + (target.0 - origin.0) * p_eased,
        origin.1 + (target.1 - origin.1) * p_eased,
    );
    let glow_scale = 1.0 + (GLOW_FINAL_SCALE - 1.0) * p_eased;
    let glow_visible = t_ms < GLOW_HIDE_MS;

    let card_alpha = ramp_f32(t_ms, CARD_FADE_START_MS, CARD_FADE_END_MS, 0.0, 1.0);
    let ui_alpha = ramp_f32(t_ms, UI_FADE_START_MS, UI_FADE_END_MS, 0.0, 1.0);

    AnimFrame {
        watermark_alpha,
        glow_visible,
        glow_pos,
        glow_scale,
        card_alpha,
        ui_alpha,
    }
}

fn anim_frame_is_steady(af: &AnimFrame) -> bool {
    af.watermark_alpha == WATERMARK_FINAL_ALPHA
        && !af.glow_visible
        && (af.card_alpha - 1.0).abs() < f32::EPSILON
        && (af.ui_alpha - 1.0).abs() < f32::EPSILON
}

fn ramp_u8(t: u64, start: u64, end: u64, from: u8, to: u8) -> u8 {
    if t <= start {
        from
    } else if t >= end {
        to
    } else {
        let p = (t - start) as f32 / (end - start) as f32;
        (from as f32 + (to as f32 - from as f32) * p) as u8
    }
}

fn ramp_f32(t: u64, start: u64, end: u64, from: f32, to: f32) -> f32 {
    if t <= start {
        from
    } else if t >= end {
        to
    } else {
        let p = (t - start) as f32 / (end - start) as f32;
        from + (to - from) * p
    }
}

#[allow(clippy::too_many_arguments)]
fn run_animation(
    card: &Card,
    db: &mut drm::control::dumbbuffer::DumbBuffer,
    fb: drm::control::framebuffer::Handle,
    painter: &CompassPainter,
    w: u32,
    h: u32,
    refresh_hz: u32,
    ui_state: &mut LoginUiState,
    keyboards: &mut [evdev::Device],
    keyboard: &mut Keyboard,
    pointers: &mut [input::PointerDevice],
    pointer: &mut PointerState,
) -> Result<ControlFlow, Box<dyn std::error::Error>> {
    let anim_start = Instant::now();
    let frame_dur = Duration::from_micros(1_000_000 / refresh_hz as u64);
    let mut frame_idx: u64 = 0;
    let mut exit = ControlFlow::Continue;
    let mut steady_frame_drawn = false;

    while exit == ControlFlow::Continue {
        let t = anim_start.elapsed();
        let t_secs = t.as_secs_f32();
        let af = compute_anim_frame(t_secs, painter, w as f32, h as f32);
        let animating = !anim_frame_is_steady(&af);
        let mut redraw = animating || !steady_frame_drawn;

        // Read keyboard once the UI is fully faded in. Polling earlier would
        // queue keystrokes the user typed against the still-animating splash.
        if af.ui_alpha >= 1.0 {
            let was_failed = matches!(ui_state.phase, InputPhase::Failed(_));
            // Failed-state shake decays back to Editing in tick()
            ui_state.tick();
            if was_failed != matches!(ui_state.phase, InputPhase::Failed(_)) {
                redraw = true;
            }
            if ui_state.clear_expired_power_confirmation() {
                redraw = true;
            }

            // Ignore key events during Authenticating or Failed — they would
            // queue up against the next Editing window otherwise.
            let accept_input = matches!(ui_state.phase, InputPhase::Editing);
            let actions = poll_keyboards(keyboards, keyboard);
            let keyboard_status = keyboard.status();
            if ui_state.keyboard_status != keyboard_status {
                ui_state.keyboard_status = keyboard_status;
                redraw = true;
            }
            for action in actions {
                if !accept_input {
                    continue;
                }
                redraw = true;
                match ui_state.apply(action) {
                    ControlFlow::Continue => {}
                    ControlFlow::Cancel => {
                        exit = ControlFlow::Cancel;
                        break;
                    }
                    ControlFlow::Submit => {
                        // Hand PAM off to a background thread so the render
                        // loop keeps drawing and the "Anmelden …" hint
                        // appears immediately.
                        ui_state.start_auth();
                        break;
                    }
                    ControlFlow::PowerOff | ControlFlow::Reboot => {}
                }
            }

            // Check if the auth thread reported back this frame.
            if let Some(result) = ui_state.poll_auth() {
                redraw = true;
                match result {
                    AuthResult::Ok(env) => {
                        ui_state.pam_env = env;
                        exit = ControlFlow::Submit;
                    }
                    AuthResult::Failed => ui_state.reject(),
                    AuthResult::Error(e) => {
                        warn!(error = %e, "PAM error — treating as failure");
                        ui_state.reject();
                    }
                }
            }
        }

        let shake_dx = ui_state.shake_offset();
        if matches!(ui_state.phase, InputPhase::Failed(_)) {
            redraw = true;
        }
        if af.ui_alpha >= 1.0 {
            let accept_pointer = matches!(ui_state.phase, InputPhase::Editing);
            let pointer_before = (pointer.x, pointer.y);
            let pointer_actions = poll_pointers(pointers, pointer);
            if (pointer.x, pointer.y) != pointer_before {
                redraw = true;
            }
            for action in pointer_actions {
                if !accept_pointer {
                    continue;
                }
                match action {
                    PointerAction::LeftPress { x, y } => {
                        match click_target_at(w as f32, h as f32, x, y, shake_dx) {
                            Some(ClickTarget::Field(field)) => {
                                ui_state.focus = field;
                                ui_state.pending_power = None;
                                redraw = true;
                            }
                            Some(ClickTarget::PowerOff) => {
                                redraw = true;
                                if let Some(flow) =
                                    ui_state.confirm_power_action(PowerAction::PowerOff)
                                {
                                    exit = flow;
                                    break;
                                }
                            }
                            Some(ClickTarget::Reboot) => {
                                redraw = true;
                                if let Some(flow) =
                                    ui_state.confirm_power_action(PowerAction::Reboot)
                                {
                                    exit = flow;
                                    break;
                                }
                            }
                            None => {}
                        }
                    }
                }
            }
        } else {
            let _ = poll_pointers(pointers, pointer);
        }

        let caret_on = matches!(ui_state.phase, InputPhase::Editing) && af.ui_alpha >= 1.0;

        if exit != ControlFlow::Continue {
            break;
        }

        if !redraw {
            frame_idx += 1;
            let next = anim_start + frame_dur * frame_idx as u32;
            if let Some(wait) = next.checked_duration_since(Instant::now()) {
                std::thread::sleep(wait);
            }
            continue;
        }

        {
            let mut mapping = card.map_dumb_buffer(db)?;
            let buf = mapping.as_mut();
            let mut pm = PixmapMut::from_bytes(buf, w, h).ok_or("pixmap bind failed")?;

            painter.render(
                &mut pm,
                w as f32,
                h as f32,
                SETTLE_T,
                &FrameOpts {
                    include_north_glow: false,
                    watermark_alpha: af.watermark_alpha,
                    force_needle_north: true,
                    ..Default::default()
                },
            );

            if af.glow_visible {
                let r0 = painter.glow_base_radius(w as f32, h as f32);
                painter.render_glow_at(&mut pm, af.glow_pos.0, af.glow_pos.1, r0 * af.glow_scale);
            }

            if af.card_alpha > 0.0 {
                draw_card(&mut pm, w as f32, h as f32, af.card_alpha, shake_dx);
            }

            if af.ui_alpha > 0.0 {
                draw_login_ui(
                    &mut pm,
                    w as f32,
                    h as f32,
                    painter,
                    ui_state,
                    af.ui_alpha,
                    caret_on,
                    shake_dx,
                );
                draw_pointer_cursor(&mut pm, pointer.x, pointer.y, af.ui_alpha);
            }

            for px in buf.chunks_exact_mut(4) {
                px.swap(0, 2);
            }
        }

        let clip = ClipRect::new(0, 0, w as u16, h as u16);
        let _ = card.dirty_framebuffer(fb, &[clip]);
        if !animating {
            steady_frame_drawn = true;
        }

        frame_idx += 1;
        let next = anim_start + frame_dur * frame_idx as u32;
        if let Some(wait) = next.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
    }
    Ok(exit)
}

fn click_target_at(w: f32, h: f32, x: f32, y: f32, shake_dx: f32) -> Option<ClickTarget> {
    let (card_left_raw, card_top, cw, _) = card_rect(w, h);
    let card_left = card_left_raw + shake_dx;
    let inner_left = card_left + CARD_PAD;
    let inner_top = card_top + CARD_PAD;
    let inner_w = cw - 2.0 * CARD_PAD;
    let user_box_top = inner_top + USER_BOX_OFFSET_Y;
    let pwd_box_top = inner_top + PASSWORD_BOX_OFFSET_Y;

    if point_in_rect(x, y, inner_left, user_box_top, inner_w, INPUT_BOX_HEIGHT) {
        Some(ClickTarget::Field(Field::Username))
    } else if point_in_rect(x, y, inner_left, pwd_box_top, inner_w, INPUT_BOX_HEIGHT) {
        Some(ClickTarget::Field(Field::Password))
    } else {
        let (restart, poweroff) = power_button_rects(w, h, shake_dx);
        if point_in_rect(x, y, restart.0, restart.1, restart.2, restart.3) {
            Some(ClickTarget::Reboot)
        } else if point_in_rect(x, y, poweroff.0, poweroff.1, poweroff.2, poweroff.3) {
            Some(ClickTarget::PowerOff)
        } else {
            None
        }
    }
}

fn power_button_rects(w: f32, h: f32, shake_dx: f32) -> PowerButtonRects {
    let (card_left_raw, card_top, cw, ch) = card_rect(w, h);
    let card_left = card_left_raw + shake_dx;
    let total_w = POWER_BUTTON_WIDTH * 2.0 + POWER_BUTTON_GAP;
    let x0 = card_left + cw / 2.0 - total_w / 2.0;
    let y = card_top + ch - POWER_BUTTON_BOTTOM_PAD - POWER_BUTTON_HEIGHT;
    (
        (x0, y, POWER_BUTTON_WIDTH, POWER_BUTTON_HEIGHT),
        (
            x0 + POWER_BUTTON_WIDTH + POWER_BUTTON_GAP,
            y,
            POWER_BUTTON_WIDTH,
            POWER_BUTTON_HEIGHT,
        ),
    )
}

fn run_power_action(action: ControlFlow) {
    let arg = match action {
        ControlFlow::PowerOff => "poweroff",
        ControlFlow::Reboot => "reboot",
        _ => return,
    };
    info!(action = arg, "requesting system power action");
    match Command::new("systemctl").arg(arg).status() {
        Ok(status) if status.success() => info!(action = arg, "system power action accepted"),
        Ok(status) => warn!(action = arg, status = ?status, "system power action failed"),
        Err(err) => warn!(action = arg, error = %err, "failed to invoke systemctl"),
    }
}

fn color_with_alpha(c: Color, alpha: u8) -> Color {
    Color::from_rgba8(
        (c.red() * 255.0) as u8,
        (c.green() * 255.0) as u8,
        (c.blue() * 255.0) as u8,
        alpha,
    )
}

fn mix_color(a: Color, b: Color, t: f32, alpha: u8) -> Color {
    let lerp = |x: f32, y: f32| ((x + (y - x) * t) * 255.0).clamp(0.0, 255.0) as u8;
    Color::from_rgba8(
        lerp(a.red(), b.red()),
        lerp(a.green(), b.green()),
        lerp(a.blue(), b.blue()),
        alpha,
    )
}

fn alpha_byte(alpha: f32, max: f32) -> u8 {
    (alpha.clamp(0.0, 1.0) * max).clamp(0.0, 255.0) as u8
}

fn metro_surface(alpha: f32) -> Color {
    Color::from_rgba8(0x24, 0x28, 0x3b, alpha_byte(alpha, 244.0))
}

fn metro_surface_alt(alpha: f32) -> Color {
    Color::from_rgba8(0x1f, 0x23, 0x35, alpha_byte(alpha, 242.0))
}

fn metro_background(alpha: f32) -> Color {
    Color::from_rgba8(0x1a, 0x1b, 0x26, alpha_byte(alpha, 238.0))
}

fn metro_accent(alpha: f32) -> Color {
    Color::from_rgba8(0x7a, 0xa2, 0xf7, alpha_byte(alpha, 255.0))
}

fn metro_text(alpha: f32) -> Color {
    Color::from_rgba8(0xc0, 0xca, 0xf5, alpha_byte(alpha, 255.0))
}

fn metro_text_dim(alpha: f32) -> Color {
    Color::from_rgba8(0xa9, 0xb1, 0xd6, alpha_byte(alpha, 230.0))
}

fn metro_border(alpha: f32) -> Color {
    Color::from_rgba8(0x41, 0x48, 0x68, alpha_byte(alpha, 230.0))
}

fn metro_error(alpha: f32) -> Color {
    Color::from_rgba8(0xf7, 0x76, 0x8e, alpha_byte(alpha, 255.0))
}

fn draw_soft_card_shadow(pm: &mut PixmapMut, left: f32, top: f32, w: f32, h: f32, alpha: f32) {
    for (dy, spread, opacity) in [(8.0, 4.0, 36u8), (2.0, 1.0, 28u8)] {
        let path = rounded_rect_path(left - spread / 2.0, top + dy, w + spread, h, 0.0);
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(0, 0, 0, (alpha * opacity as f32) as u8));
        paint.anti_alias = true;
        pm.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn draw_card_stroke(pm: &mut PixmapMut, path: &tiny_skia::Path, color: Color, width: f32) {
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    let stroke = Stroke {
        width,
        ..Default::default()
    };
    pm.stroke_path(path, &paint, &stroke, Transform::identity(), None);
}

fn draw_card_highlight(pm: &mut PixmapMut, left: f32, top: f32, w: f32, alpha: f32) {
    let mut pb = PathBuilder::new();
    pb.move_to(left, top + METRO_STRIPE_HEIGHT + 1.0);
    pb.line_to(left + w, top + METRO_STRIPE_HEIGHT + 1.0);
    let Some(path) = pb.finish() else {
        return;
    };
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(255, 255, 255, alpha_byte(alpha, 24.0)));
    paint.anti_alias = true;
    let stroke = Stroke {
        width: 1.0,
        ..Default::default()
    };
    pm.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
}

fn draw_login_button(
    pm: &mut PixmapMut,
    painter: &CompassPainter,
    rect: Rect,
    label: &str,
    accent: Color,
    alpha: f32,
    selected: bool,
) {
    let path = rounded_rect_path(rect.0, rect.1, rect.2, rect.3, 0.0);
    let fill = mix_color(
        metro_surface(1.0),
        accent,
        if selected { 0.26 } else { 0.08 },
        alpha_byte(alpha, if selected { 238.0 } else { 224.0 }),
    );
    let mut fill_paint = Paint::default();
    fill_paint.set_color(fill);
    fill_paint.anti_alias = true;
    pm.fill_path(
        &path,
        &fill_paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    draw_card_stroke(
        pm,
        &path,
        if selected {
            color_with_alpha(accent, alpha_byte(alpha, 240.0))
        } else {
            metro_border(alpha)
        },
        if selected { 2.0 } else { 1.0 },
    );

    let stripe = rounded_rect_path(rect.0, rect.1, rect.2, 3.0, 0.0);
    let mut stripe_paint = Paint::default();
    stripe_paint.set_color(color_with_alpha(accent, alpha_byte(alpha, 230.0)));
    stripe_paint.anti_alias = true;
    pm.fill_path(
        &stripe,
        &stripe_paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    painter.render_text_centered(
        pm,
        TextStyle::SansBold(13.0),
        label,
        rect.0 + rect.2 / 2.0,
        rect.1 + rect.3 / 2.0,
        metro_text(alpha),
    );
}

fn draw_power_buttons(
    pm: &mut PixmapMut,
    w: f32,
    h: f32,
    painter: &CompassPainter,
    alpha: f32,
    shake_dx: f32,
    pending: Option<PowerAction>,
) {
    let (restart, poweroff) = power_button_rects(w, h, shake_dx);
    draw_login_button(
        pm,
        painter,
        restart,
        if pending == Some(PowerAction::Reboot) {
            "Bestätigen"
        } else {
            "Neustart"
        },
        metro_accent(1.0),
        alpha,
        pending == Some(PowerAction::Reboot),
    );
    draw_login_button(
        pm,
        painter,
        poweroff,
        if pending == Some(PowerAction::PowerOff) {
            "Bestätigen"
        } else {
            "Aus"
        },
        metro_error(1.0),
        alpha,
        pending == Some(PowerAction::PowerOff),
    );
}

fn point_in_rect(px: f32, py: f32, x: f32, y: f32, w: f32, h: f32) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}

fn draw_card(pm: &mut PixmapMut, w: f32, h: f32, alpha: f32, shake_dx: f32) {
    let (left, top, cw, ch) = card_rect(w, h);
    let left = left + shake_dx;
    let path = rounded_rect_path(left, top, cw, ch, 0.0);
    draw_soft_card_shadow(pm, left, top, cw, ch, alpha);

    let mut fill = Paint::default();
    fill.set_color(metro_surface_alt(alpha));
    fill.anti_alias = true;
    pm.fill_path(&path, &fill, FillRule::Winding, Transform::identity(), None);

    let stripe = rounded_rect_path(left, top, cw, METRO_STRIPE_HEIGHT, 0.0);
    let mut stripe_paint = Paint::default();
    stripe_paint.set_color(metro_accent(alpha));
    stripe_paint.anti_alias = true;
    pm.fill_path(
        &stripe,
        &stripe_paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    draw_card_stroke(pm, &path, metro_border(alpha), 1.0);
    draw_card_highlight(pm, left, top, cw, alpha);
}

#[allow(clippy::too_many_arguments)]
fn draw_login_ui(
    pm: &mut PixmapMut,
    w: f32,
    h: f32,
    painter: &CompassPainter,
    ui: &LoginUiState,
    alpha: f32,
    caret_on: bool,
    shake_dx: f32,
) {
    let (card_left_raw, card_top, cw, _ch) = card_rect(w, h);
    let card_left = card_left_raw + shake_dx;
    let inner_left = card_left + CARD_PAD;
    let inner_top = card_top + CARD_PAD;
    let inner_w = cw - 2.0 * CARD_PAD;
    let cx = card_left + cw / 2.0;

    let text_color = metro_text(alpha);
    let label_color = metro_text_dim(alpha);
    let hint_color = metro_text_dim(alpha * 0.78);
    let title_color = metro_accent(alpha);
    let caret_color = metro_accent(alpha);
    let box_fill = metro_background(alpha);
    let box_outline = metro_border(alpha);

    // Title
    painter.render_text_centered(
        pm,
        TextStyle::SansBold(24.0),
        "Meridian",
        cx,
        inner_top + TITLE_OFFSET_Y,
        title_color,
    );

    let label_size = 14.0;
    let text_size = 22.0;

    // Username row
    let user_label_y = inner_top + USER_LABEL_OFFSET_Y;
    painter.render_text_left(
        pm,
        TextStyle::SansBold(label_size),
        "User",
        inner_left,
        user_label_y,
        label_color,
    );
    let user_box_top = inner_top + USER_BOX_OFFSET_Y;
    draw_input_box(
        pm,
        inner_left,
        user_box_top,
        inner_w,
        INPUT_BOX_HEIGHT,
        box_fill,
        box_outline,
        ui.focus == Field::Username,
        alpha,
    );
    let user_text_x = inner_left + INPUT_TEXT_PAD_X;
    let user_baseline = user_box_top + INPUT_BOX_HEIGHT - INPUT_BASELINE_PAD_BOTTOM;
    let after_user = painter.render_text_left(
        pm,
        TextStyle::SansBold(text_size),
        &ui.username,
        user_text_x,
        user_baseline,
        text_color,
    );
    if ui.focus == Field::Username && caret_on {
        draw_caret(pm, after_user, user_baseline, text_size, caret_color);
    }

    // Password row
    let pwd_label_y = inner_top + PASSWORD_LABEL_OFFSET_Y;
    painter.render_text_left(
        pm,
        TextStyle::SansBold(label_size),
        "Passwort",
        inner_left,
        pwd_label_y,
        label_color,
    );
    let pwd_box_top = inner_top + PASSWORD_BOX_OFFSET_Y;
    draw_input_box(
        pm,
        inner_left,
        pwd_box_top,
        inner_w,
        INPUT_BOX_HEIGHT,
        box_fill,
        box_outline,
        ui.focus == Field::Password,
        alpha,
    );
    let pwd_text_x = inner_left + INPUT_TEXT_PAD_X;
    let pwd_baseline = pwd_box_top + INPUT_BOX_HEIGHT - INPUT_BASELINE_PAD_BOTTOM;
    let dots = "•".repeat(ui.password.chars().count());
    let after_pwd = painter.render_text_left(
        pm,
        TextStyle::SansBold(text_size),
        &dots,
        pwd_text_x,
        pwd_baseline,
        text_color,
    );
    if ui.focus == Field::Password && caret_on {
        draw_caret(pm, after_pwd, pwd_baseline, text_size, caret_color);
    }

    // Bottom hint — text depends on phase (Editing vs Failed)
    let (restart_rect, _) = power_button_rects(w, h, shake_dx);
    let hint_y = restart_rect.1 - HINT_POWER_GAP;
    let hint_text = ui.hint();
    let hint_color_phase = match ui.phase {
        InputPhase::Failed(_) => metro_error(alpha),
        InputPhase::Editing | InputPhase::Authenticating => hint_color,
    };
    painter.render_text_centered(
        pm,
        TextStyle::SansBold(13.0),
        &hint_text,
        cx,
        hint_y,
        hint_color_phase,
    );
    draw_power_buttons(
        pm,
        w,
        h,
        painter,
        alpha,
        shake_dx,
        ui.pending_power_action(),
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_input_box(
    pm: &mut PixmapMut,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fill: Color,
    outline: Color,
    focused: bool,
    alpha: f32,
) {
    let path = rounded_rect_path(x, y, w, h, 0.0);
    let mut fill_paint = Paint::default();
    fill_paint.set_color(fill);
    fill_paint.anti_alias = true;
    pm.fill_path(
        &path,
        &fill_paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    let mut stroke_paint = Paint::default();
    stroke_paint.set_color(if focused {
        metro_accent(alpha)
    } else {
        outline
    });
    stroke_paint.anti_alias = true;
    let stroke = Stroke {
        width: if focused { 2.0 } else { 1.0 },
        ..Default::default()
    };
    pm.stroke_path(&path, &stroke_paint, &stroke, Transform::identity(), None);

    if focused {
        let accent = rounded_rect_path(x, y, 3.0, h, 0.0);
        let mut accent_paint = Paint::default();
        accent_paint.set_color(metro_accent(alpha));
        accent_paint.anti_alias = true;
        pm.fill_path(
            &accent,
            &accent_paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn draw_caret(pm: &mut PixmapMut, x: f32, baseline_y: f32, font_size: f32, color: Color) {
    let top = baseline_y - 0.75 * font_size;
    let bottom = baseline_y + 0.1 * font_size;
    let mut pb = PathBuilder::new();
    pb.move_to(x, top);
    pb.line_to(x, bottom);
    let path = pb.finish().unwrap();
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    let stroke = Stroke {
        width: 1.5,
        ..Default::default()
    };
    pm.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
}

fn draw_pointer_cursor(pm: &mut PixmapMut, x: f32, y: f32, alpha: f32) {
    let alpha = (alpha * 255.0) as u8;
    let mut pb = PathBuilder::new();
    pb.move_to(x, y);
    pb.line_to(x, y + 22.0);
    pb.line_to(x + 5.5, y + 17.0);
    pb.line_to(x + 9.0, y + 25.0);
    pb.line_to(x + 13.0, y + 23.0);
    pb.line_to(x + 9.5, y + 15.5);
    pb.line_to(x + 17.0, y + 15.5);
    pb.close();
    let Some(path) = pb.finish() else {
        return;
    };

    let mut fill = Paint::default();
    fill.set_color(Color::from_rgba8(235, 241, 252, alpha));
    fill.anti_alias = true;
    pm.fill_path(&path, &fill, FillRule::Winding, Transform::identity(), None);

    let mut stroke_paint = Paint::default();
    stroke_paint.set_color(Color::from_rgba8(5, 8, 14, alpha));
    stroke_paint.anti_alias = true;
    let stroke = Stroke {
        width: 1.25,
        ..Default::default()
    };
    pm.stroke_path(&path, &stroke_paint, &stroke, Transform::identity(), None);
}

fn card_rect(w: f32, h: f32) -> (f32, f32, f32, f32) {
    let cw = (w * 0.32).clamp(360.0, 720.0);
    let ch = (h * 0.28).clamp(300.0, 420.0);
    let left = w / 2.0 - cw / 2.0;
    let top = h / 2.0 - ch / 2.0;
    (left, top, cw, ch)
}

fn rounded_rect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> tiny_skia::Path {
    let r = r.min(w / 2.0).min(h / 2.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish().unwrap()
}

fn bootsplash_handover() -> std::io::Result<()> {
    send_command(&bootsplash_socket_path(), b"handover\n").map(|_| ())
}

fn bootsplash_exit() -> std::io::Result<()> {
    send_command(&bootsplash_socket_path(), b"exit\n").map(|_| ())
}

fn bootsplash_socket_path() -> String {
    std::env::var(BOOTSPLASH_SOCKET_ENV).unwrap_or_else(|_| BOOTSPLASH_SOCKET.to_string())
}

fn send_command(path: &str, cmd: &[u8]) -> std::io::Result<String> {
    let mut s = UnixStream::connect(path)?;
    // 10s read timeout: covers the synchronous bootsplash handover which
    // only acks after release_master_lock, and that itself waits until
    // bootsplash's render loop reaches HANDOVER_MIN_T (~3s from boot).
    // 500ms was correct for the old fire-and-forget protocol but too
    // short for the new synchronous one.
    s.set_read_timeout(Some(Duration::from_secs(10)))?;
    s.set_write_timeout(Some(Duration::from_millis(500)))?;
    s.write_all(cmd)?;
    let mut buf = [0u8; 256];
    let n = s.read(&mut buf).unwrap_or(0);
    let resp = String::from_utf8_lossy(&buf[..n]).into_owned();
    if resp.starts_with("ok") {
        Ok(resp)
    } else {
        Err(std::io::Error::other(format!(
            "peer refused: {}",
            resp.trim()
        )))
    }
}

// ---- Phase 8 IPC: login as handover server ----

/// Events the compositor (or any client) can signal over the login IPC
/// socket. The login main loop reacts to these by dropping its DRM master /
/// fd (Handover) or by logging that the new compositor is fully visible
/// (Exit).
enum IpcEvent {
    /// Compositor is ready to take the screen. Login must release DRM
    /// master and close its card fd so the compositor's libseat acquire
    /// succeeds. This is the moment that gates the visible handover.
    Handover,
    /// Compositor's first frame is on screen. Informational; lets us log
    /// the handover latency and could later be used to suppress the
    /// fallback timeout.
    Exit,
}

/// Bind the login IPC socket and spawn an accept thread that forwards
/// each parsed command as an [`IpcEvent`] into `tx`. Returns the socket
/// path and identity so the caller can unlink exactly this socket on shutdown.
///
/// `owner_uid`/`owner_gid` are the authenticated user — we chown the
/// socket to them and clamp to mode 0600 so only the spawned compositor
/// (running as that user) can connect. Without this clamp the socket
/// would be world-connectable and any local non-root account could send
/// `handover` to make us drop DRM master prematurely.
fn spawn_login_ipc_server(
    tx: mpsc::Sender<IpcEvent>,
    owner_uid: u32,
    owner_gid: u32,
) -> std::io::Result<(PathBuf, SocketIdentity)> {
    let path = PathBuf::from(LOGIN_SOCKET);
    let _ = fs::remove_file(&path);
    let listener = UnixListener::bind(&path)?;
    // chown FIRST, then chmod: shrinking the access window during the
    // tiny gap between bind and lockdown.
    nix::unistd::chown(
        &path,
        Some(nix::unistd::Uid::from_raw(owner_uid)),
        Some(nix::unistd::Gid::from_raw(owner_gid)),
    )
    .map_err(|e| std::io::Error::other(format!("chown ipc socket: {}", e)))?;
    let socket_identity = secure_socket_permissions(&path)?;
    info!(
        path = %path.display(),
        owner_uid,
        owner_gid,
        "login ipc: listening (chown'd to owner, mode 0600)"
    );

    thread::spawn(move || {
        for conn in listener.incoming() {
            match conn {
                Ok(stream) => {
                    let tx2 = tx.clone();
                    thread::spawn(move || handle_login_ipc_client(stream, tx2));
                }
                Err(e) => {
                    warn!(error = %e, "login ipc: accept failed");
                    break;
                }
            }
        }
    });
    Ok((path, socket_identity))
}

fn handle_login_ipc_client(mut stream: UnixStream, tx: mpsc::Sender<IpcEvent>) {
    let read_clone = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    let reader = BufReader::new(read_clone);
    for line in reader.lines() {
        let Ok(cmd) = line else { return };
        match cmd.trim() {
            "" => continue,
            "handover" => {
                let _ = tx.send(IpcEvent::Handover);
                let _ = writeln!(stream, "ok handover");
            }
            "exit" => {
                let _ = tx.send(IpcEvent::Exit);
                let _ = writeln!(stream, "ok exit");
            }
            other => {
                let _ = writeln!(stream, "err unknown command: {}", other);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> CompassPainter<'static> {
        CompassPainter::new(Fonts::quompacc()).unwrap()
    }

    #[test]
    fn anim_frame_at_t0_matches_settle_state() {
        let painter = p();
        let af = compute_anim_frame(0.0, &painter, 1920.0, 1080.0);
        assert_eq!(af.watermark_alpha, 0);
        assert!(af.glow_visible);
        assert!((af.glow_scale - 1.0).abs() < 1e-3);
        assert_eq!(af.card_alpha, 0.0);
        assert_eq!(af.ui_alpha, 0.0);
    }

    #[test]
    fn anim_frame_at_ui_fade_end_is_full() {
        let painter = p();
        let t = UI_FADE_END_MS as f32 / 1000.0;
        let af = compute_anim_frame(t, &painter, 1920.0, 1080.0);
        assert!((af.card_alpha - 1.0).abs() < 1e-3);
        assert!((af.ui_alpha - 1.0).abs() < 1e-3);
        assert_eq!(af.watermark_alpha, WATERMARK_FINAL_ALPHA);
        assert!(!af.glow_visible);
    }

    #[test]
    fn anim_frame_reports_steady_after_intro() {
        let painter = p();
        let t = (UI_FADE_END_MS + 100) as f32 / 1000.0;
        let af = compute_anim_frame(t, &painter, 1920.0, 1080.0);
        assert!(anim_frame_is_steady(&af));
    }

    #[test]
    fn ramp_u8_clamps_outside_window() {
        assert_eq!(ramp_u8(50, 100, 200, 10, 90), 10);
        assert_eq!(ramp_u8(150, 100, 200, 10, 90), 50);
        assert_eq!(ramp_u8(300, 100, 200, 10, 90), 90);
    }

    #[test]
    fn card_rect_clamped_dimensions() {
        let (_, _, cw, ch) = card_rect(1920.0, 1440.0);
        assert!((360.0..=720.0).contains(&cw));
        assert!((300.0..=420.0).contains(&ch));
    }

    #[test]
    fn power_buttons_are_click_targets() {
        let (restart, poweroff) = power_button_rects(1920.0, 1080.0, 0.0);
        let restart_center = (restart.0 + restart.2 / 2.0, restart.1 + restart.3 / 2.0);
        let poweroff_center = (poweroff.0 + poweroff.2 / 2.0, poweroff.1 + poweroff.3 / 2.0);

        assert_eq!(
            click_target_at(1920.0, 1080.0, restart_center.0, restart_center.1, 0.0),
            Some(ClickTarget::Reboot)
        );
        assert_eq!(
            click_target_at(1920.0, 1080.0, poweroff_center.0, poweroff_center.1, 0.0),
            Some(ClickTarget::PowerOff)
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
    fn insert_appends_to_focused_field() {
        let mut s = LoginUiState::default();
        assert_eq!(s.focus, Field::Username);
        assert_eq!(
            s.apply(KeyAction::Insert("a".into())),
            ControlFlow::Continue
        );
        assert_eq!(
            s.apply(KeyAction::Insert("b".into())),
            ControlFlow::Continue
        );
        assert_eq!(s.username, "ab");
        assert!(s.password.is_empty());

        s.focus = Field::Password;
        s.apply(KeyAction::Insert("x".into()));
        assert_eq!(s.username, "ab");
        assert_eq!(s.password.as_str(), "x");
    }

    #[test]
    fn backspace_removes_last_char_from_focused_field() {
        let mut s = LoginUiState::default();
        s.apply(KeyAction::Insert("abc".into()));
        s.apply(KeyAction::Backspace);
        assert_eq!(s.username, "ab");
        s.apply(KeyAction::Backspace);
        s.apply(KeyAction::Backspace);
        s.apply(KeyAction::Backspace); // no-op on empty
        assert_eq!(s.username, "");
    }

    #[test]
    fn cycle_focus_toggles_between_username_and_password() {
        let mut s = LoginUiState::default();
        s.apply(KeyAction::CycleFocus);
        assert_eq!(s.focus, Field::Password);
        s.apply(KeyAction::CycleFocus);
        assert_eq!(s.focus, Field::Username);
    }

    #[test]
    fn submit_and_cancel_return_their_control_flow() {
        let mut s = LoginUiState::default();
        assert_eq!(s.apply(KeyAction::Submit), ControlFlow::Submit);
        assert_eq!(s.apply(KeyAction::Cancel), ControlFlow::Cancel);
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
        assert_eq!(s.hint(), "Enter zum Anmelden - Layout DE");
        s.phase = InputPhase::Authenticating;
        assert_eq!(s.hint(), "Anmelden …");
        s.phase = InputPhase::Failed(Instant::now());
        assert_eq!(s.hint(), "Passwort erneut eingeben");
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
    fn poll_auth_returns_none_when_no_thread_running() {
        let mut s = LoginUiState::default();
        assert!(s.poll_auth().is_none());
    }

    #[test]
    fn start_auth_with_empty_username_returns_failed_quickly() {
        // start_auth spawns a worker; with empty username the worker
        // short-circuits to Failed and the channel delivers within a few ms.
        let mut s = LoginUiState::default();
        s.start_auth();
        assert!(matches!(s.phase, InputPhase::Authenticating));
        assert!(s.auth_driver.is_some());
        // Spin for up to 200ms waiting for the result.
        let deadline = Instant::now() + Duration::from_millis(200);
        let result = loop {
            if let Some(r) = s.poll_auth() {
                break r;
            }
            if Instant::now() > deadline {
                panic!("auth thread didn't post within 200ms");
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(result, AuthResult::Failed);
        // Driver still holds the worker join handle; reject() drops it which
        // joins the (already-exited) worker.
        s.reject();
        assert!(s.auth_driver.is_none());
    }

    #[test]
    fn insert_respects_max_field_len() {
        let mut s = LoginUiState::default();
        for _ in 0..MAX_FIELD_LEN + 16 {
            s.apply(KeyAction::Insert("a".into()));
        }
        assert_eq!(s.username.chars().count(), MAX_FIELD_LEN);
    }
}
