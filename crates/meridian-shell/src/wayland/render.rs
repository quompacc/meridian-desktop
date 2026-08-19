use std::hash::{Hash, Hasher};

use smithay_client_toolkit::shell::WaylandSurface;
use tracing::{debug, info, warn};
use wayland_client::QueueHandle;

use crate::{
    audio_popup, buffer, network_popup, notification_popup, panel, status_notifier_popup,
    workspaces, Painter, Rect, AUDIO_POPUP_HEIGHT, AUDIO_POPUP_WIDTH, CALENDAR_POPUP_HEIGHT,
    CALENDAR_POPUP_WIDTH, LAUNCHER_HEIGHT, LAUNCHER_WIDTH, NETWORK_POPUP_HEIGHT,
    NETWORK_POPUP_WIDTH, NOTIFICATION_HEIGHT, NOTIFICATION_WIDTH, WORKSPACE_POPUP_HEIGHT,
    WORKSPACE_POPUP_WIDTH,
};

use super::{
    calendar::{weekday_labels, CalendarMonthModel},
    shell::{PanelRenderSignature, ThemeRenderSignature},
    time, CommitReason, CommitSurfaceKind, MeridianShell, RepaintReason,
};

const CANVAS_RETRY_ATTEMPTS: usize = 2;

include!("render/core.rs");
include!("render/desktop_overlays.rs");
include!("render/launcher.rs");
include!("render/calendar_workspace.rs");
include!("render/network_audio.rs");
include!("render/notifications.rs");

/// German month name from a 1-based month number.
fn german_month_name(month: u8) -> &'static str {
    match month {
        1 => "Januar",
        2 => "Februar",
        3 => "März",
        4 => "April",
        5 => "Mai",
        6 => "Juni",
        7 => "Juli",
        8 => "August",
        9 => "September",
        10 => "Oktober",
        11 => "November",
        12 => "Dezember",
        _ => "",
    }
}

/// Make the four corners of a packed ARGB8888 buffer transparent so a
/// rectangular surface renders with rounded corners. Anti-aliased via a 1px
/// coverage falloff; channels are scaled together (premultiplied-safe).
fn round_buffer_corners(buf: &mut [u8], w: usize, h: usize, radius: i32) {
    if radius <= 0 || w == 0 || h == 0 {
        return;
    }
    let rad = (radius as usize).min(w / 2).min(h / 2);
    let r = rad as f32;
    let corners = [
        (r, r, 0usize, 0usize),
        ((w - rad) as f32, r, w - rad, 0),
        (r, (h - rad) as f32, 0, h - rad),
        ((w - rad) as f32, (h - rad) as f32, w - rad, h - rad),
    ];
    for (cx, cy, x0, y0) in corners {
        for yy in 0..rad {
            for xx in 0..rad {
                let px = x0 + xx;
                let py = y0 + yy;
                let dx = (px as f32 + 0.5) - cx;
                let dy = (py as f32 + 0.5) - cy;
                let dist = (dx * dx + dy * dy).sqrt();
                let cov = (r - dist + 0.5).clamp(0.0, 1.0);
                if cov < 1.0 {
                    let idx = (py * w + px) * 4;
                    if idx + 4 <= buf.len() {
                        for k in 0..4 {
                            buf[idx + k] = (buf[idx + k] as f32 * cov).round() as u8;
                        }
                    }
                }
            }
        }
    }
}
