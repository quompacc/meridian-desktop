// meridian-login — Phase 2: DRM master takeover from bootsplash.
//
// Sequence:
//   1. Handshake with bootsplash via /run/bootsplash.sock — tell it to drop
//      master and stay alive holding its buffer. If the socket isn't there
//      (dev runs without bootsplash), proceed anyway.
//   2. Open /dev/dri/card0, configure CRTC + dumb buffer + framebuffer.
//   3. Render the compass settle frame via meridian-compass-render.
//   4. Tell bootsplash to exit (it can now release its buffer).
//   5. Hold the frame for a short window (Phase 2 has no further animation).
//   6. Release DRM master, exit.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use drm::buffer::DrmFourcc;
use drm::control::{connector, ClipRect, Device as ControlDevice};
use drm::Device as DrmDevice;

use meridian_compass_render::{CompassPainter, Fonts, FrameOpts, SETTLE_T};
use tiny_skia::PixmapMut;
use tracing::{info, warn};

const BOOTSPLASH_SOCKET: &str = "/run/bootsplash.sock";
// bootsplash needs a moment between sending its ack and actually calling
// release_master_lock (one render iteration plus drop). 200ms is comfortable.
const HANDOVER_SETTLE_MS: u64 = 200;
// Phase 2 stays on the settle frame for a fixed window so the transition is
// observable. Future phases replace this with the fall-and-morph animation.
const HOLD_DURATION_SECS: u64 = 3;

struct Card(File);

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}
impl DrmDevice for Card {}
impl ControlDevice for Card {}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    info!("meridian-login starting (Phase 2)");

    match bootsplash_handover() {
        Ok(()) => {
            info!("bootsplash handover acked");
            std::thread::sleep(Duration::from_millis(HANDOVER_SETTLE_MS));
        }
        Err(e) => warn!(error = %e, "bootsplash handover failed (not running?); proceeding"),
    }

    let card = Card(
        OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/dri/card0")?,
    );

    let res = card.resource_handles()?;
    let conn_info = res
        .connectors()
        .iter()
        .map(|&h| card.get_connector(h, false).unwrap())
        .find(|c| c.state() == connector::State::Connected)
        .ok_or("no connected connector")?;
    let mode = pick_mode(conn_info.modes()).ok_or("connector has no usable mode")?;
    let (w, h) = mode.size();
    let (w, h) = (w as u32, h as u32);
    info!(width = w, height = h, refresh = mode.vrefresh(), "drm mode");

    let crtc = if let Some(enc_h) = conn_info.current_encoder() {
        card.get_encoder(enc_h)?.crtc()
    } else {
        None
    }
    .unwrap_or_else(|| res.crtcs()[0]);

    let mut db = card.create_dumb_buffer((w, h), DrmFourcc::Xrgb8888, 32)?;
    let fb = card.add_framebuffer(&db, 24, 32)?;
    card.set_crtc(crtc, Some(fb), (0, 0), &[conn_info.handle()], Some(mode))?;

    let painter = CompassPainter::new(Fonts::quompacc())?;
    {
        let mut mapping = card.map_dumb_buffer(&mut db)?;
        let buf = mapping.as_mut();
        let mut pm = PixmapMut::from_bytes(buf, w, h).ok_or("pixmap bind failed")?;
        painter.render(&mut pm, w as f32, h as f32, SETTLE_T, &FrameOpts::default());
        // tiny-skia writes RGBA; DRM XRGB8888 on LE wants BGRX → swap R<->B
        for px in buf.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
    }
    let clip = ClipRect::new(0, 0, w as u16, h as u16);
    let _ = card.dirty_framebuffer(fb, &[clip]);
    info!("settle frame rendered");

    match bootsplash_exit() {
        Ok(()) => info!("bootsplash exit signalled"),
        Err(e) => warn!(error = %e, "bootsplash exit signal failed"),
    }

    info!(secs = HOLD_DURATION_SECS, "holding frame");
    std::thread::sleep(Duration::from_secs(HOLD_DURATION_SECS));

    match card.release_master_lock() {
        Ok(()) => info!("drm: released master"),
        Err(e) => warn!(error = %e, "drm: release_master failed"),
    }

    info!("meridian-login exiting");
    Ok(())
}

fn bootsplash_handover() -> std::io::Result<()> {
    send_command(BOOTSPLASH_SOCKET, b"handover\n").map(|_| ())
}

fn bootsplash_exit() -> std::io::Result<()> {
    send_command(BOOTSPLASH_SOCKET, b"exit\n").map(|_| ())
}

fn send_command(path: &str, cmd: &[u8]) -> std::io::Result<String> {
    let mut s = UnixStream::connect(path)?;
    s.set_read_timeout(Some(Duration::from_millis(500)))?;
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

// Pick a mid-range mode: largest where the longer side stays ≤ 2560 px.
// Falls back to whatever the connector hands us first.
fn pick_mode(modes: &[drm::control::Mode]) -> Option<drm::control::Mode> {
    let mut filtered: Vec<_> = modes
        .iter()
        .copied()
        .filter(|m| {
            let (w, h) = m.size();
            w.max(h) <= 2560 && w >= 1280 && h >= 720
        })
        .collect();
    filtered.sort_by_key(|m| {
        let (w, h) = m.size();
        std::cmp::Reverse(w as u32 * h as u32)
    });
    filtered.first().copied().or_else(|| modes.first().copied())
}
