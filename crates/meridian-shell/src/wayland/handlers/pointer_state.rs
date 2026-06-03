use meridian_ui::{Event, PointerButton, WidgetPath, WidgetState};
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};
use wayland_client::QueueHandle;

use crate::wayland::{MeridianShell, RepaintReason};

pub(super) fn apply_pointer_event(
    current: Option<(WidgetPath, WidgetState)>,
    event: &Event,
    hit_path: Option<WidgetPath>,
) -> Option<(WidgetPath, WidgetState)> {
    match event {
        Event::PointerLeave => None,
        Event::PointerEnter { .. } | Event::PointerMove { .. } => {
            if let Some((ref p, WidgetState::Pressed)) = current {
                if hit_path.as_ref() == Some(p) {
                    return current;
                }
            }
            hit_path.map(|p| (p, WidgetState::Hovered))
        }
        Event::PointerPress {
            button: PointerButton::Left,
            ..
        } => hit_path.map(|p| (p, WidgetState::Pressed)).or(current),
        Event::PointerPress { .. } => current,
        Event::PointerRelease {
            button: PointerButton::Left,
            ..
        } => hit_path.map(|p| (p, WidgetState::Hovered)),
        Event::PointerRelease { .. } => current,
    }
}

pub(super) fn detect_click(
    prev: Option<&(WidgetPath, WidgetState)>,
    event: &Event,
    hit_path: Option<&WidgetPath>,
) -> Option<WidgetPath> {
    match event {
        Event::PointerRelease {
            button: PointerButton::Left,
            ..
        } => match prev {
            Some((p, WidgetState::Pressed)) => match hit_path {
                Some(q) if q == p => Some(p.clone()),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

impl MeridianShell {
    /// Drive the region-picker drag state machine. Press starts a new
    /// selection (clears any pending one). Motion updates the live drag
    /// end. Release freezes the drag into `pending` (or clears the drag
    /// without a pending if the rectangle was degenerate).
    pub(super) fn handle_region_picker_pointer(
        &mut self,
        qh: &QueueHandle<Self>,
        event: &PointerEvent,
    ) {
        let (px, py) = event.position;
        let pi = (px.round() as i32, py.round() as i32);
        let w = self.region_picker_width;
        let h = self.region_picker_height;
        match event.kind {
            PointerEventKind::Press { .. } => {
                self.region_picker_drag_start = Some(pi);
                self.region_picker_drag_current = Some(pi);
                self.region_picker_pending = None;
                self.draw_region_picker(qh, RepaintReason::Pointer);
            }
            PointerEventKind::Motion { .. } if self.region_picker_drag_start.is_some() => {
                self.region_picker_drag_current = Some(pi);
                self.draw_region_picker(qh, RepaintReason::Pointer);
            }
            PointerEventKind::Release { .. } => {
                if let Some(start) = self.region_picker_drag_start {
                    let rect = crate::region_picker::rect_from_drag(start, pi, w, h);
                    self.region_picker_pending = rect;
                    self.region_picker_drag_start = None;
                    self.region_picker_drag_current = None;
                    self.draw_region_picker(qh, RepaintReason::Pointer);
                }
            }
            _ => {}
        }
    }

    /// Handle pointer events on the screenshot consent modal: hover highlights
    /// a button; a left-press inside Allow/Deny answers and closes the modal.
    pub(super) fn handle_consent_pointer(&mut self, qh: &QueueHandle<Self>, event: &PointerEvent) {
        let (px, py) = event.position;
        match event.kind {
            PointerEventKind::Motion { .. } => {
                let hover = crate::screenshot_consent::hit_button(px, py);
                if hover != self.consent_hover {
                    self.consent_hover = hover;
                    self.draw_consent_modal(qh, RepaintReason::Pointer);
                }
            }
            PointerEventKind::Press { .. } => match crate::screenshot_consent::hit_button(px, py) {
                Some(crate::screenshot_consent::ConsentButton::Allow) => {
                    self.respond_consent(true);
                }
                Some(crate::screenshot_consent::ConsentButton::Deny) => {
                    self.respond_consent(false);
                }
                None => {}
            },
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use meridian_ui::{Event, PointerButton, PointerPosition, WidgetPath, WidgetState};

    use super::{apply_pointer_event, detect_click};

    fn path(v: &[usize]) -> WidgetPath {
        WidgetPath::from_vec(v.to_vec())
    }

    fn pos(x: i32, y: i32) -> PointerPosition {
        PointerPosition { x, y }
    }

    fn path_a() -> WidgetPath {
        path(&[0, 1])
    }

    fn path_b() -> WidgetPath {
        path(&[0, 2])
    }

    fn root_path() -> WidgetPath {
        path(&[])
    }

    #[test]
    fn apply_move_with_hit_returns_hovered() {
        let ev = Event::PointerMove {
            position: pos(10, 10),
        };
        let result = apply_pointer_event(None, &ev, Some(path_a()));
        assert_eq!(result, Some((path_a(), WidgetState::Hovered)));
    }

    #[test]
    fn apply_move_without_hit_returns_none() {
        let ev = Event::PointerMove {
            position: pos(10, 10),
        };
        let result = apply_pointer_event(None, &ev, None);
        assert_eq!(result, None);
    }

    #[test]
    fn apply_press_left_returns_pressed() {
        let ev = Event::PointerPress {
            position: pos(10, 10),
            button: PointerButton::Left,
        };
        let result = apply_pointer_event(None, &ev, Some(path_a()));
        assert_eq!(result, Some((path_a(), WidgetState::Pressed)));
    }

    #[test]
    fn apply_press_right_keeps_current() {
        let ev = Event::PointerPress {
            position: pos(10, 10),
            button: PointerButton::Right,
        };
        let current = Some((path_a(), WidgetState::Hovered));
        let result = apply_pointer_event(current.clone(), &ev, Some(path_b()));
        assert_eq!(result, current);
    }

    #[test]
    fn apply_release_left_on_pressed_returns_hovered() {
        let press = Event::PointerPress {
            position: pos(10, 10),
            button: PointerButton::Left,
        };
        let release = Event::PointerRelease {
            position: pos(10, 10),
            button: PointerButton::Left,
        };
        let after_press = apply_pointer_event(None, &press, Some(path_a()));
        assert_eq!(after_press, Some((path_a(), WidgetState::Pressed)));
        let after_release = apply_pointer_event(after_press, &release, Some(path_a()));
        assert_eq!(after_release, Some((path_a(), WidgetState::Hovered)));
    }

    #[test]
    fn apply_release_left_off_target_returns_none() {
        let press = Event::PointerPress {
            position: pos(10, 10),
            button: PointerButton::Left,
        };
        let release = Event::PointerRelease {
            position: pos(10, 10),
            button: PointerButton::Left,
        };
        let after_press = apply_pointer_event(None, &press, Some(path_a()));
        assert_eq!(after_press, Some((path_a(), WidgetState::Pressed)));
        let after_release = apply_pointer_event(after_press, &release, None);
        assert_eq!(after_release, None);
    }

    #[test]
    fn apply_leave_returns_none() {
        let current = Some((path_a(), WidgetState::Hovered));
        let result = apply_pointer_event(current, &Event::PointerLeave, None);
        assert_eq!(result, None);
    }

    #[test]
    fn apply_move_keeps_pressed_when_still_on_pressed_path() {
        let press = Event::PointerPress {
            position: pos(10, 10),
            button: PointerButton::Left,
        };
        let move_on = Event::PointerMove {
            position: pos(15, 15),
        };
        let after_press = apply_pointer_event(None, &press, Some(path_a()));
        assert_eq!(after_press, Some((path_a(), WidgetState::Pressed)));
        let after_move = apply_pointer_event(after_press, &move_on, Some(path_a()));
        assert_eq!(after_move, Some((path_a(), WidgetState::Pressed)));
    }

    #[test]
    fn apply_move_to_other_widget_while_pressed_switches_to_hovered_new() {
        let press = Event::PointerPress {
            position: pos(10, 10),
            button: PointerButton::Left,
        };
        let move_to_b = Event::PointerMove {
            position: pos(50, 50),
        };
        let after_press = apply_pointer_event(None, &press, Some(path_a()));
        assert_eq!(after_press, Some((path_a(), WidgetState::Pressed)));
        let after_move = apply_pointer_event(after_press, &move_to_b, Some(path_b()));
        assert_eq!(after_move, Some((path_b(), WidgetState::Hovered)));
    }

    #[test]
    fn apply_enter_on_empty_path_returns_hovered_at_root() {
        let ev = Event::PointerEnter {
            position: pos(0, 0),
        };
        let result = apply_pointer_event(None, &ev, Some(root_path()));
        assert_eq!(result, Some((root_path(), WidgetState::Hovered)));
    }

    #[test]
    fn click_on_same_widget_detected() {
        let prev = Some((path_a(), WidgetState::Pressed));
        let release = Event::PointerRelease {
            position: pos(10, 10),
            button: PointerButton::Left,
        };
        let hit = Some(path_a());
        let result = detect_click(prev.as_ref(), &release, hit.as_ref());
        assert_eq!(result, Some(path_a()));
    }

    #[test]
    fn click_on_different_widget_not_detected() {
        let prev = Some((path_a(), WidgetState::Pressed));
        let release = Event::PointerRelease {
            position: pos(10, 10),
            button: PointerButton::Left,
        };
        let hit = Some(path_b());
        let result = detect_click(prev.as_ref(), &release, hit.as_ref());
        assert_eq!(result, None);
    }

    #[test]
    fn release_without_prior_press_not_click() {
        let release = Event::PointerRelease {
            position: pos(10, 10),
            button: PointerButton::Left,
        };
        let hit = Some(path_a());
        let result = detect_click(None, &release, hit.as_ref());
        assert_eq!(result, None);
    }

    #[test]
    fn non_release_event_not_click() {
        let prev = Some((path_a(), WidgetState::Pressed));
        let move_ev = Event::PointerMove {
            position: pos(10, 10),
        };
        let hit = Some(path_a());
        let result = detect_click(prev.as_ref(), &move_ev, hit.as_ref());
        assert_eq!(result, None);
    }
}
