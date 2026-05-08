use meridian_compositor::{
    backend::{drm::init_drm, winit::init_winit},
    protocols::xwayland::start_xwayland,
    state::MeridianState,
};
use smithay::reexports::{calloop::EventLoop, wayland_server::Display};
use tracing::{info, warn};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let mut event_loop: EventLoop<'static, MeridianState> = EventLoop::try_new()?;
    let display: Display<MeridianState> = Display::new()?;
    let mut state = MeridianState::new(&mut event_loop, display);

    // Use DRM when no parent display is available (running from TTY),
    // fall back to winit when inside an existing Wayland/X11 session.
    let in_session = std::env::var("WAYLAND_DISPLAY").is_ok()
        || std::env::var("DISPLAY").is_ok();

    if in_session {
        info!("Detected parent display – using winit backend");
        init_winit(&mut event_loop, &mut state)?;
    } else {
        info!("No parent display – using DRM/KMS backend");
        match init_drm(&mut event_loop, &mut state) {
            Ok(()) => {}
            Err(err) => {
                warn!("DRM init failed ({}), falling back to winit", err);
                init_winit(&mut event_loop, &mut state)?;
            }
        }
    }

    info!("Meridian running on socket: {:?}", state.socket_name);
    unsafe { std::env::set_var("WAYLAND_DISPLAY", &state.socket_name) };

    start_xwayland(&mut state);

    event_loop.run(None, &mut state, move |_| {})?;
    Ok(())
}
