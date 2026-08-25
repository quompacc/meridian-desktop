use std::{
    cell::RefCell,
    time::{Duration, Instant},
};

use smithay::{
    desktop::{Space, Window},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Rectangle, Size},
    wayland::{compositor, seat::WaylandFocus},
};

use super::ResizeEdge;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
enum ResizePhase {
    #[default]
    Idle,
    Active,
    WaitingForLastCommit,
}

#[derive(Debug, Clone, Copy)]
struct ConfigureRequest {
    target: Rectangle<i32, Logical>,
    sent_at: Instant,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct ResizeCommitStats {
    pub commits: u64,
    pub configure_acks: u64,
    pub total_ack_latency: Duration,
    pub max_ack_latency: Duration,
    pub last_committed_size: Option<Size<i32, Logical>>,
}

impl ResizeCommitStats {
    pub fn average_ack_latency(self) -> Duration {
        if self.configure_acks == 0 {
            Duration::ZERO
        } else {
            self.total_ack_latency / self.configure_acks as u32
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ResizeSurfaceSnapshot {
    pub commit_generation: u64,
    pub stats: ResizeCommitStats,
}

#[derive(Debug)]
struct ResizeSurfaceState {
    phase: ResizePhase,
    edges: ResizeEdge,
    initial_rect: Rectangle<i32, Logical>,
    preview_rect: Option<Rectangle<i32, Logical>>,
    outstanding_configure: Option<ConfigureRequest>,
    commit_generation: u64,
    stats: ResizeCommitStats,
}

impl Default for ResizeSurfaceState {
    fn default() -> Self {
        Self {
            phase: ResizePhase::Idle,
            edges: ResizeEdge::empty(),
            initial_rect: Rectangle::default(),
            preview_rect: None,
            outstanding_configure: None,
            commit_generation: 0,
            stats: ResizeCommitStats::default(),
        }
    }
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

    fn record_commit(&mut self, committed_size: Size<i32, Logical>, now: Instant) {
        if self.phase == ResizePhase::Idle {
            return;
        }
        self.commit_generation = self.commit_generation.saturating_add(1);
        self.stats.commits = self.stats.commits.saturating_add(1);
        self.stats.last_committed_size = Some(committed_size);
        if let Some(request) = self.outstanding_configure.take() {
            let latency = now.saturating_duration_since(request.sent_at);
            self.stats.configure_acks = self.stats.configure_acks.saturating_add(1);
            self.stats.total_ack_latency += latency;
            self.stats.max_ack_latency = self.stats.max_ack_latency.max(latency);
            tracing::trace!(
                requested_width = request.target.size.w,
                requested_height = request.target.size.h,
                committed_width = committed_size.w,
                committed_height = committed_size.h,
                latency_us = latency.as_micros(),
                "interactive resize client commit"
            );
        }
    }
}

pub(super) fn begin(
    surface: &WlSurface,
    edges: ResizeEdge,
    initial_rect: Rectangle<i32, Logical>,
) -> ResizeSurfaceSnapshot {
    ResizeSurfaceState::with(surface, |state| {
        *state = ResizeSurfaceState {
            phase: ResizePhase::Active,
            edges,
            initial_rect,
            preview_rect: Some(initial_rect),
            stats: ResizeCommitStats {
                last_committed_size: Some(initial_rect.size),
                ..ResizeCommitStats::default()
            },
            ..ResizeSurfaceState::default()
        };
        state.snapshot()
    })
}

pub(super) fn update_preview(surface: &WlSurface, target: Rectangle<i32, Logical>) {
    ResizeSurfaceState::with(surface, |state| {
        if state.phase != ResizePhase::Idle {
            state.preview_rect = Some(target);
        }
    });
}

pub(super) fn note_configure(
    surface: &WlSurface,
    target: Rectangle<i32, Logical>,
    sent_at: Instant,
) {
    ResizeSurfaceState::with(surface, |state| {
        state.outstanding_configure = Some(ConfigureRequest { target, sent_at });
    });
}

pub(super) fn snapshot(surface: &WlSurface) -> ResizeSurfaceSnapshot {
    ResizeSurfaceState::with(surface, |state| state.snapshot())
}

pub(super) fn finish(surface: &WlSurface, final_target: Rectangle<i32, Logical>) {
    ResizeSurfaceState::with(surface, |state| {
        state.preview_rect = Some(final_target);
        if state.stats.last_committed_size == Some(final_target.size)
            && state.outstanding_configure.is_none()
        {
            state.phase = ResizePhase::Idle;
            state.preview_rect = None;
        } else {
            state.phase = ResizePhase::WaitingForLastCommit;
        }
    });
}

pub(crate) fn preview_rect(surface: &WlSurface) -> Option<Rectangle<i32, Logical>> {
    ResizeSurfaceState::with(surface, |state| state.preview_rect)
}

impl ResizeSurfaceState {
    fn snapshot(&self) -> ResizeSurfaceSnapshot {
        ResizeSurfaceSnapshot {
            commit_generation: self.commit_generation,
            stats: self.stats,
        }
    }
}

pub fn handle_commit(space: &mut Space<Window>, surface: &WlSurface) -> Option<()> {
    let window = space
        .elements()
        .find(|window| {
            window
                .wl_surface()
                .is_some_and(|window_surface| window_surface.as_ref() == surface)
        })
        .cloned()?;

    let mut window_loc = space.element_location(&window)?;
    let geometry = window.geometry();
    let now = Instant::now();

    let (new_loc, clear_preview): (Point<Option<i32>, Logical>, bool) =
        ResizeSurfaceState::with(surface, |state| {
            state.record_commit(geometry.size, now);
            let anchored_loc = if state.phase != ResizePhase::Idle {
                anchored_location(state.edges, state.initial_rect, geometry.size)
            } else {
                Default::default()
            };
            let clear = state.phase == ResizePhase::WaitingForLastCommit;
            if clear {
                state.phase = ResizePhase::Idle;
                state.preview_rect = None;
                state.outstanding_configure = None;
            }
            (anchored_loc, clear)
        });

    if let Some(new_x) = new_loc.x {
        window_loc.x = new_x;
    }
    if let Some(new_y) = new_loc.y {
        window_loc.y = new_y;
    }
    if new_loc.x.is_some() || new_loc.y.is_some() {
        space.map_element(window, window_loc, false);
    }
    if clear_preview {
        tracing::trace!("interactive resize preview cleared after final client commit");
    }
    Some(())
}

fn anchored_location(
    edges: ResizeEdge,
    initial_rect: Rectangle<i32, Logical>,
    committed_size: Size<i32, Logical>,
) -> Point<Option<i32>, Logical> {
    let new_x = edges
        .intersects(ResizeEdge::LEFT)
        .then_some(initial_rect.loc.x + (initial_rect.size.w - committed_size.w));
    let new_y = edges
        .intersects(ResizeEdge::TOP)
        .then_some(initial_rect.loc.y + (initial_rect.size.h - committed_size.h));
    (new_x, new_y).into()
}

#[cfg(test)]
mod tests {
    use super::anchored_location;
    use crate::grabs::resize_grab::ResizeEdge;
    use smithay::utils::{Rectangle, Size};

    #[test]
    fn left_and_top_edges_remain_anchored_to_initial_opposite_edges() {
        let initial = Rectangle::new((100, 200).into(), (400, 300).into());
        let location = anchored_location(ResizeEdge::TOP_LEFT, initial, Size::from((500, 350)));
        assert_eq!(location.x, Some(0));
        assert_eq!(location.y, Some(150));
    }

    #[test]
    fn bottom_right_resize_does_not_move_client_origin() {
        let initial = Rectangle::new((100, 200).into(), (400, 300).into());
        let location = anchored_location(ResizeEdge::BOTTOM_RIGHT, initial, Size::from((500, 350)));
        assert_eq!(location.x, None);
        assert_eq!(location.y, None);
    }
}
