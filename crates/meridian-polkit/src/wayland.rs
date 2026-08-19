// Wayland side of the polkit agent. Raw wayland-client + wlr-layer-shell,
// modelled on meridian-lock (single layer surface, tiny-skia render, xkb
// keyboard) but with the surface created on demand instead of always-on,
// and driven by polkit BeginAuthentication requests instead of a session
// lock.
//
// We deliberately do NOT use smithay-client-toolkit here: meridian-lock's
// raw dispatch idiom is a smaller surface area and matches the rest of
// the codebase.

use std::os::fd::{AsFd, FromRawFd, OwnedFd};
use std::os::raw::c_void;
use std::os::unix::io::AsRawFd;

use meridian_config::{MeridianConfig, ThemeConfig, ThemeManager};
use smithay_client_toolkit::reexports::calloop::channel as cchannel;
use tracing::{debug, info, warn};
use wayland_client::{
    delegate_noop,
    protocol::{
        wl_buffer, wl_compositor, wl_keyboard, wl_output, wl_registry, wl_seat, wl_shm,
        wl_shm_pool, wl_surface,
    },
    Connection, Dispatch, EventQueue, QueueHandle, WEnum,
};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};
use xkbcommon::xkb;
use zeroize::Zeroizing;

use crate::dbus::{AuthRequest, Identity, Outcome};
use crate::ui;

fn create_anonymous_shm() -> std::io::Result<OwnedFd> {
    #[cfg(target_os = "openbsd")]
    {
        unsafe extern "C" {
            fn shm_mkstemp(template: *mut libc::c_char) -> libc::c_int;
        }

        let mut template = *b"/meridian-polkit.XXXXXXXXXX\0";
        // SAFETY: the template is writable, NUL-terminated, and has the six
        // trailing X characters required by OpenBSD's shm_mkstemp(3).
        let raw_fd = unsafe { shm_mkstemp(template.as_mut_ptr().cast()) };
        if raw_fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: shm_mkstemp returned a new descriptor owned by this call.
        let fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };
        if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(fd)
    }

    #[cfg(not(target_os = "openbsd"))]
    {
        // SAFETY: the name is a static NUL-terminated C string.
        let raw_fd = unsafe { libc::memfd_create(c"meridian-polkit".as_ptr(), libc::MFD_CLOEXEC) };
        if raw_fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: memfd_create returned a new descriptor owned by this call.
        Ok(unsafe { OwnedFd::from_raw_fd(raw_fd) })
    }
}

pub struct PamResult {
    pub cookie: String,
    pub ok: bool,
}

pub struct ActiveAuth {
    // Nur fuers Logging/zukuenftige Anzeige gehalten.
    #[allow(dead_code)]
    pub action_id: String,
    pub message: String,
    pub cookie: String,
    pub identity: Identity,
    pub password: Zeroizing<String>,
    pub status: ui::Status,
    pub retries: u32,
    pub reply: Option<tokio::sync::oneshot::Sender<Outcome>>,
}

pub struct AppState {
    pub running: bool,
    pub theme: ThemeConfig,

    // Wayland globals (set in registry dispatch)
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    seat: Option<wl_seat::WlSeat>,
    layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,

    // Keyboard
    xkb_ctx: xkb::Context,
    xkb_state: Option<xkb::State>,

    // Active popup
    active: Option<ActiveAuth>,
    popup: Option<PopupSurface>,

    // PAM result channel
    pub pam_tx: cchannel::Sender<PamResult>,
}

struct PopupSurface {
    surface: wl_surface::WlSurface,
    layer_surface: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    width: u32,
    height: u32,
    configured: bool,
    shm_ptr: *mut u8,
    shm_size: usize,
    buffer: Option<wl_buffer::WlBuffer>,
}

// PopupSurface is only ever touched on the main thread (calloop driver).
unsafe impl Send for AppState {}

const POPUP_W: u32 = 520;
const POPUP_H: u32 = 340;

include!("wayland/state.rs");
include!("wayland/dispatch.rs");
