use std::cell::RefCell;

use bitflags::bitflags;
use smithay::{
    desktop::{Space, Window},
    input::pointer::{
        AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent,
        GesturePinchBeginEvent, GesturePinchEndEvent, GesturePinchUpdateEvent,
        GestureSwipeBeginEvent, GestureSwipeEndEvent, GestureSwipeUpdateEvent,
        GrabStartData as PointerGrabStartData, MotionEvent, PointerGrab, PointerInnerHandle,
        RelativeMotionEvent,
    },
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::protocol::wl_surface::WlSurface,
    },
    utils::{Logical, Point, Rectangle, Size},
    wayland::{compositor, shell::xdg::SurfaceCachedState},
};

use crate::state::MeridianState;

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

pub struct ResizeSurfaceGrab {
    pub start_data: PointerGrabStartData<MeridianState>,
    window: Window,
    edges: ResizeEdge,
    initial_rect: Rectangle<i32, Logical>,
    last_window_size: Size<i32, Logical>,
}

impl ResizeSurfaceGrab {
    pub fn start(
        start_data: PointerGrabStartData<MeridianState>,
        window: Window,
        edges: ResizeEdge,
        initial_rect: Rectangle<i32, Logical>,
    ) -> Self {
        ResizeSurfaceState::with(window.toplevel().unwrap().wl_surface(), |state| {
            *state = ResizeSurfaceState::Resizing { edges, initial_rect };
        });
        Self { start_data, window, edges, initial_rect, last_window_size: initial_rect.size }
    }
}

impl PointerGrab<MeridianState> for ResizeSurfaceGrab {
    fn motion(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
        _focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &MotionEvent,
    ) {
        handle.motion(data, None, event);

        let mut delta = event.location - self.start_data.location;
        let mut new_w = self.initial_rect.size.w;
        let mut new_h = self.initial_rect.size.h;

        if self.edges.intersects(ResizeEdge::LEFT | ResizeEdge::RIGHT) {
            if self.edges.intersects(ResizeEdge::LEFT) { delta.x = -delta.x; }
            new_w = (self.initial_rect.size.w as f64 + delta.x) as i32;
        }
        if self.edges.intersects(ResizeEdge::TOP | ResizeEdge::BOTTOM) {
            if self.edges.intersects(ResizeEdge::TOP) { delta.y = -delta.y; }
            new_h = (self.initial_rect.size.h as f64 + delta.y) as i32;
        }

        let (min_size, max_size) =
            compositor::with_states(self.window.toplevel().unwrap().wl_surface(), |states| {
                let mut guard = states.cached_state.get::<SurfaceCachedState>();
                let data = guard.current();
                (data.min_size, data.max_size)
            });

        let min_w = min_size.w.max(1);
        let min_h = min_size.h.max(1);
        let max_w = if max_size.w == 0 { i32::MAX } else { max_size.w };
        let max_h = if max_size.h == 0 { i32::MAX } else { max_size.h };

        self.last_window_size = Size::from((
            new_w.max(min_w).min(max_w),
            new_h.max(min_h).min(max_h),
        ));

        let xdg = self.window.toplevel().unwrap();
        xdg.with_pending_state(|state| {
            state.states.set(xdg_toplevel::State::Resizing);
            state.size = Some(self.last_window_size);
        });
        xdg.send_pending_configure();
    }

    fn relative_motion(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
        focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &RelativeMotionEvent,
    ) {
        handle.relative_motion(data, focus, event);
    }

    fn button(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
        event: &ButtonEvent,
    ) {
        handle.button(data, event);
        const BTN_LEFT: u32 = 0x110;
        if !handle.current_pressed().contains(&BTN_LEFT) {
            handle.unset_grab(self, data, event.serial, event.time, true);
            let xdg = self.window.toplevel().unwrap();
            xdg.with_pending_state(|state| {
                state.states.unset(xdg_toplevel::State::Resizing);
                state.size = Some(self.last_window_size);
            });
            xdg.send_pending_configure();
            ResizeSurfaceState::with(xdg.wl_surface(), |state| {
                *state = ResizeSurfaceState::WaitingForLastCommit {
                    edges: self.edges,
                    initial_rect: self.initial_rect,
                };
            });
        }
    }

    fn axis(&mut self, data: &mut MeridianState, handle: &mut PointerInnerHandle<'_, MeridianState>, details: AxisFrame) { handle.axis(data, details); }
    fn frame(&mut self, data: &mut MeridianState, handle: &mut PointerInnerHandle<'_, MeridianState>) { handle.frame(data); }
    fn gesture_swipe_begin(&mut self, data: &mut MeridianState, handle: &mut PointerInnerHandle<'_, MeridianState>, event: &GestureSwipeBeginEvent) { handle.gesture_swipe_begin(data, event); }
    fn gesture_swipe_update(&mut self, data: &mut MeridianState, handle: &mut PointerInnerHandle<'_, MeridianState>, event: &GestureSwipeUpdateEvent) { handle.gesture_swipe_update(data, event); }
    fn gesture_swipe_end(&mut self, data: &mut MeridianState, handle: &mut PointerInnerHandle<'_, MeridianState>, event: &GestureSwipeEndEvent) { handle.gesture_swipe_end(data, event); }
    fn gesture_pinch_begin(&mut self, data: &mut MeridianState, handle: &mut PointerInnerHandle<'_, MeridianState>, event: &GesturePinchBeginEvent) { handle.gesture_pinch_begin(data, event); }
    fn gesture_pinch_update(&mut self, data: &mut MeridianState, handle: &mut PointerInnerHandle<'_, MeridianState>, event: &GesturePinchUpdateEvent) { handle.gesture_pinch_update(data, event); }
    fn gesture_pinch_end(&mut self, data: &mut MeridianState, handle: &mut PointerInnerHandle<'_, MeridianState>, event: &GesturePinchEndEvent) { handle.gesture_pinch_end(data, event); }
    fn gesture_hold_begin(&mut self, data: &mut MeridianState, handle: &mut PointerInnerHandle<'_, MeridianState>, event: &GestureHoldBeginEvent) { handle.gesture_hold_begin(data, event); }
    fn gesture_hold_end(&mut self, data: &mut MeridianState, handle: &mut PointerInnerHandle<'_, MeridianState>, event: &GestureHoldEndEvent) { handle.gesture_hold_end(data, event); }

    fn start_data(&self) -> &PointerGrabStartData<MeridianState> { &self.start_data }
    fn unset(&mut self, _data: &mut MeridianState) {}
}

// ── ResizeSurfaceState ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
enum ResizeSurfaceState {
    #[default]
    Idle,
    Resizing {
        edges: ResizeEdge,
        initial_rect: Rectangle<i32, Logical>,
    },
    WaitingForLastCommit {
        edges: ResizeEdge,
        initial_rect: Rectangle<i32, Logical>,
    },
}

impl ResizeSurfaceState {
    fn with<F, T>(surface: &WlSurface, cb: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        compositor::with_states(surface, |states| {
            states.data_map.insert_if_missing(RefCell::<Self>::default);
            cb(&mut states.data_map.get::<RefCell<Self>>().unwrap().borrow_mut())
        })
    }

    fn commit(&mut self) -> Option<(ResizeEdge, Rectangle<i32, Logical>)> {
        match *self {
            Self::Resizing { edges, initial_rect } => Some((edges, initial_rect)),
            Self::WaitingForLastCommit { edges, initial_rect } => {
                *self = Self::Idle;
                Some((edges, initial_rect))
            }
            Self::Idle => None,
        }
    }
}

/// Fensterposition nach Resize-Commit korrigieren (TOP/LEFT edges verschieben das Fenster).
pub fn handle_commit(space: &mut Space<Window>, surface: &WlSurface) -> Option<()> {
    let window = space
        .elements()
        .find(|w| w.toplevel().map_or(false, |t| t.wl_surface() == surface))
        .cloned()?;

    let mut window_loc = space.element_location(&window)?;
    let geometry = window.geometry();

    let new_loc: Point<Option<i32>, Logical> = ResizeSurfaceState::with(surface, |state| {
        state.commit().and_then(|(edges, initial_rect)| {
            edges.intersects(ResizeEdge::TOP_LEFT).then(|| {
                let new_x = edges
                    .intersects(ResizeEdge::LEFT)
                    .then_some(initial_rect.loc.x + (initial_rect.size.w - geometry.size.w));
                let new_y = edges
                    .intersects(ResizeEdge::TOP)
                    .then_some(initial_rect.loc.y + (initial_rect.size.h - geometry.size.h));
                (new_x, new_y).into()
            })
        }).unwrap_or_default()
    });

    if let Some(new_x) = new_loc.x { window_loc.x = new_x; }
    if let Some(new_y) = new_loc.y { window_loc.y = new_y; }
    if new_loc.x.is_some() || new_loc.y.is_some() {
        space.map_element(window, window_loc, false);
    }
    Some(())
}
