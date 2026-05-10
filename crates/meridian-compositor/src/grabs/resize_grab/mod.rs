use bitflags::bitflags;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;

mod grab;
mod state;

pub use grab::ResizeSurfaceGrab;
pub use state::handle_commit;

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
