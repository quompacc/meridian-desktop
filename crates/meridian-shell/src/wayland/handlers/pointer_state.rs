use meridian_ui::{Event, PointerButton, WidgetPath, WidgetState};

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

#[cfg(test)]
mod tests {
    use meridian_ui::{Event, PointerButton, PointerPosition, WidgetPath, WidgetState};

    use super::apply_pointer_event;

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
}
