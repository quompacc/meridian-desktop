use bitflags::bitflags;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::utils::{Logical, Point};
use std::time::Duration;

use crate::state::MeridianState;

mod grab;
mod pacing;
mod state;

pub use grab::ResizeSurfaceGrab;
pub use state::handle_commit;
pub(crate) use state::preview_rect;

pub(crate) fn configure_interval_at(
    state: &MeridianState,
    location: Point<f64, Logical>,
) -> Duration {
    let refresh_millihz = state
        .output_registry
        .list()
        .iter()
        .find(|output| {
            let geo = output.geometry;
            location.x >= geo.x as f64
                && location.y >= geo.y as f64
                && location.x < (geo.x + geo.width) as f64
                && location.y < (geo.y + geo.height) as f64
        })
        .and_then(|output| output.refresh_millihz);
    configure_interval_from_millihz(refresh_millihz)
}

fn configure_interval_from_millihz(refresh_millihz: Option<i32>) -> Duration {
    const FALLBACK_REFRESH_MILLIHZ: u64 = 60_000;
    let refresh = refresh_millihz
        .filter(|refresh| *refresh > 0)
        .map_or(FALLBACK_REFRESH_MILLIHZ, |refresh| refresh as u64);
    Duration::from_nanos(1_000_000_000_000_u64.div_ceil(refresh))
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct ResizeEdge: u32 {
        const TOP          = 0b0001;
        const BOTTOM       = 0b0010;
        const LEFT         = 0b0100;
        const RIGHT        = 0b1000;
        const TOP_LEFT     = Self::TOP.bits()    | Self::LEFT.bits();
        const BOTTOM_LEFT  = Self::BOTTOM.bits() | Self::LEFT.bits();
        const TOP_RIGHT    = Self::TOP.bits()    | Self::RIGHT.bits();
        const BOTTOM_RIGHT = Self::BOTTOM.bits() | Self::RIGHT.bits();
    }
}

impl From<xdg_toplevel::ResizeEdge> for ResizeEdge {
    fn from(x: xdg_toplevel::ResizeEdge) -> Self {
        Self::from_bits(x as u32).unwrap_or(ResizeEdge::empty())
    }
}

#[cfg(test)]
mod interval_tests {
    use super::configure_interval_from_millihz;
    use std::time::Duration;

    #[test]
    fn configure_interval_tracks_refresh_and_has_safe_fallback() {
        assert_eq!(
            configure_interval_from_millihz(Some(60_000)),
            Duration::from_nanos(16_666_667)
        );
        assert_eq!(
            configure_interval_from_millihz(Some(120_000)),
            Duration::from_nanos(8_333_334)
        );
        assert_eq!(
            configure_interval_from_millihz(None),
            Duration::from_nanos(16_666_667)
        );
    }
}
