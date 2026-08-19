use std::time::{Duration, Instant};

use smithay::{
    desktop::Window,
    input::pointer::{
        AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent,
        GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent,
        GestureSwipeEndEvent, GestureSwipeUpdateEvent, GrabStartData as PointerGrabStartData,
        MotionEvent, PointerGrab, PointerInnerHandle, RelativeMotionEvent,
    },
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::protocol::wl_surface::WlSurface,
    },
    utils::{Logical, Point, Rectangle, Size},
    wayland::{compositor, shell::xdg::SurfaceCachedState},
    xwayland::X11Surface,
};
use tracing::{error, info};

use crate::state::MeridianState;

use super::{pacing::ConfigurePacer, state::ResizeSurfaceState, ResizeEdge};

enum ResizeSurfaceTarget {
    Xdg(smithay::wayland::shell::xdg::ToplevelSurface),
    X11(Box<X11Surface>),
}

pub struct ResizeSurfaceGrab {
    pub start_data: PointerGrabStartData<MeridianState>,
    target: ResizeSurfaceTarget,
    edges: ResizeEdge,
    initial_rect: Rectangle<i32, Logical>,
    last_window_size: Size<i32, Logical>,
    configure_pacer: ConfigurePacer<Rectangle<i32, Logical>>,
}

impl ResizeSurfaceGrab {
    pub fn start(
        configure_interval: Duration,
        start_data: PointerGrabStartData<MeridianState>,
        window: Window,
        edges: ResizeEdge,
        initial_rect: Rectangle<i32, Logical>,
    ) -> Self {
        let target = if let Some(toplevel) = window.toplevel() {
            ResizeSurfaceState::with(toplevel.wl_surface(), |state| {
                *state = ResizeSurfaceState::Resizing {
                    edges,
                    initial_rect,
                };
            });
            ResizeSurfaceTarget::Xdg(toplevel.clone())
        } else if let Some(x11) = window.x11_surface() {
            ResizeSurfaceTarget::X11(Box::new(x11.clone()))
        } else {
            unreachable!("resize grab requires xdg or x11 window target")
        };
        Self {
            start_data,
            target,
            edges,
            initial_rect,
            last_window_size: initial_rect.size,
            configure_pacer: ConfigurePacer::new(configure_interval),
        }
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
            if self.edges.intersects(ResizeEdge::LEFT) {
                delta.x = -delta.x;
            }
            new_w = (self.initial_rect.size.w as f64 + delta.x) as i32;
        }
        if self.edges.intersects(ResizeEdge::TOP | ResizeEdge::BOTTOM) {
            if self.edges.intersects(ResizeEdge::TOP) {
                delta.y = -delta.y;
            }
            new_h = (self.initial_rect.size.h as f64 + delta.y) as i32;
        }

        let (min_w, min_h, max_w, max_h) = match &self.target {
            ResizeSurfaceTarget::Xdg(xdg) => {
                let (min_size, max_size) = compositor::with_states(xdg.wl_surface(), |states| {
                    let mut guard = states.cached_state.get::<SurfaceCachedState>();
                    let data = guard.current();
                    (data.min_size, data.max_size)
                });
                (
                    min_size.w.max(1),
                    min_size.h.max(1),
                    if max_size.w == 0 {
                        i32::MAX
                    } else {
                        max_size.w
                    },
                    if max_size.h == 0 {
                        i32::MAX
                    } else {
                        max_size.h
                    },
                )
            }
            ResizeSurfaceTarget::X11(_) => (1, 1, i32::MAX, i32::MAX),
        };

        self.last_window_size =
            Size::from((new_w.max(min_w).min(max_w), new_h.max(min_h).min(max_h)));

        let requested = resize_target_rect(self.initial_rect, self.last_window_size, self.edges);
        let Some(requested) = self.configure_pacer.offer(requested, Instant::now()) else {
            return;
        };

        match &self.target {
            ResizeSurfaceTarget::Xdg(xdg) => {
                xdg.with_pending_state(|state| {
                    state.states.set(xdg_toplevel::State::Resizing);
                    state.size = Some(requested.size);
                });
                xdg.send_pending_configure();
            }
            ResizeSurfaceTarget::X11(x11) => {
                if let Err(err) = x11.configure(requested) {
                    error!("xwayland resize grab configure failed: {}", err);
                }
            }
        }
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
            let final_target = self.configure_pacer.flush(Instant::now());
            match &self.target {
                ResizeSurfaceTarget::Xdg(xdg) => {
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
                ResizeSurfaceTarget::X11(x11) => {
                    if let Some(requested) = final_target {
                        if let Err(err) = x11.configure(requested) {
                            error!("xwayland resize grab final configure failed: {}", err);
                        }
                    }
                }
            }
            let stats = self.configure_pacer.stats();
            info!(
                "interactive resize pacing summary: target={} offers={} emitted={} duplicates={} coalesced={} interval_us={}",
                match &self.target {
                    ResizeSurfaceTarget::Xdg(_) => "xdg",
                    ResizeSurfaceTarget::X11(_) => "x11",
                },
                stats.offers,
                stats.emitted,
                stats.duplicates,
                stats.coalesced,
                self.configure_pacer.interval().as_micros()
            );
        }
    }

    fn axis(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
        details: AxisFrame,
    ) {
        handle.axis(data, details);
    }
    fn frame(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
    ) {
        handle.frame(data);
    }
    fn gesture_swipe_begin(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
        event: &GestureSwipeBeginEvent,
    ) {
        handle.gesture_swipe_begin(data, event);
    }
    fn gesture_swipe_update(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
        event: &GestureSwipeUpdateEvent,
    ) {
        handle.gesture_swipe_update(data, event);
    }
    fn gesture_swipe_end(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
        event: &GestureSwipeEndEvent,
    ) {
        handle.gesture_swipe_end(data, event);
    }
    fn gesture_pinch_begin(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
        event: &GesturePinchBeginEvent,
    ) {
        handle.gesture_pinch_begin(data, event);
    }
    fn gesture_pinch_update(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
        event: &GesturePinchUpdateEvent,
    ) {
        handle.gesture_pinch_update(data, event);
    }
    fn gesture_pinch_end(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
        event: &GesturePinchEndEvent,
    ) {
        handle.gesture_pinch_end(data, event);
    }
    fn gesture_hold_begin(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
        event: &GestureHoldBeginEvent,
    ) {
        handle.gesture_hold_begin(data, event);
    }
    fn gesture_hold_end(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
        event: &GestureHoldEndEvent,
    ) {
        handle.gesture_hold_end(data, event);
    }

    fn start_data(&self) -> &PointerGrabStartData<MeridianState> {
        &self.start_data
    }
    fn unset(&mut self, _data: &mut MeridianState) {}
}

fn resize_target_rect(
    initial_rect: Rectangle<i32, Logical>,
    size: Size<i32, Logical>,
    edges: ResizeEdge,
) -> Rectangle<i32, Logical> {
    let mut loc = initial_rect.loc;
    if edges.intersects(ResizeEdge::LEFT) {
        loc.x += initial_rect.size.w - size.w;
    }
    if edges.intersects(ResizeEdge::TOP) {
        loc.y += initial_rect.size.h - size.h;
    }
    Rectangle::new(loc, size)
}
