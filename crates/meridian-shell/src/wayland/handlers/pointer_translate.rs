use meridian_ui::{Event, PointerButton, PointerPosition};
use smithay_client_toolkit::seat::pointer::PointerEventKind;

const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;
const BTN_MIDDLE: u32 = 0x112;

fn translate_pointer_button(code: u32) -> Option<PointerButton> {
    match code {
        BTN_LEFT => Some(PointerButton::Left),
        BTN_RIGHT => Some(PointerButton::Right),
        BTN_MIDDLE => Some(PointerButton::Middle),
        _ => None,
    }
}

pub(super) fn translate_pointer_event(
    kind: &PointerEventKind,
    position: (f64, f64),
) -> Option<Event> {
    let pos = PointerPosition {
        x: position.0 as i32,
        y: position.1 as i32,
    };
    match kind {
        PointerEventKind::Enter { .. } => Some(Event::PointerEnter { position: pos }),
        PointerEventKind::Leave { .. } => Some(Event::PointerLeave),
        PointerEventKind::Motion { .. } => Some(Event::PointerMove { position: pos }),
        PointerEventKind::Press { button, .. } => {
            translate_pointer_button(*button).map(|btn| Event::PointerPress {
                position: pos,
                button: btn,
            })
        }
        PointerEventKind::Release { button, .. } => {
            translate_pointer_button(*button).map(|btn| Event::PointerRelease {
                position: pos,
                button: btn,
            })
        }
        PointerEventKind::Axis { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use smithay_client_toolkit::seat::pointer::{AxisScroll, PointerEventKind};

    use super::{translate_pointer_button, translate_pointer_event};

    #[test]
    fn translate_button_left() {
        assert_eq!(
            translate_pointer_button(0x110),
            Some(meridian_ui::PointerButton::Left)
        );
    }

    #[test]
    fn translate_button_right() {
        assert_eq!(
            translate_pointer_button(0x111),
            Some(meridian_ui::PointerButton::Right)
        );
    }

    #[test]
    fn translate_button_middle() {
        assert_eq!(
            translate_pointer_button(0x112),
            Some(meridian_ui::PointerButton::Middle)
        );
    }

    #[test]
    fn translate_button_unknown() {
        assert_eq!(translate_pointer_button(0), None);
        assert_eq!(translate_pointer_button(0x200), None);
    }

    #[test]
    fn translate_motion_yields_pointer_move_with_position() {
        let kind = PointerEventKind::Motion { time: 0 };
        let ev = translate_pointer_event(&kind, (42.0, 17.0)).expect("motion translates");
        assert_eq!(
            ev,
            meridian_ui::Event::PointerMove {
                position: meridian_ui::PointerPosition { x: 42, y: 17 }
            }
        );
    }

    #[test]
    fn translate_enter_yields_pointer_enter_with_position() {
        let kind = PointerEventKind::Enter { serial: 1 };
        let ev = translate_pointer_event(&kind, (10.0, 20.0)).expect("enter translates");
        assert_eq!(
            ev,
            meridian_ui::Event::PointerEnter {
                position: meridian_ui::PointerPosition { x: 10, y: 20 }
            }
        );
    }

    #[test]
    fn translate_leave_yields_pointer_leave() {
        let kind = PointerEventKind::Leave { serial: 1 };
        let ev = translate_pointer_event(&kind, (0.0, 0.0)).expect("leave translates");
        assert_eq!(ev, meridian_ui::Event::PointerLeave);
    }

    #[test]
    fn translate_press_left_yields_pointer_press_left() {
        let kind = PointerEventKind::Press {
            time: 0,
            button: 0x110,
            serial: 1,
        };
        let ev = translate_pointer_event(&kind, (5.0, 5.0)).expect("left press translates");
        assert_eq!(
            ev,
            meridian_ui::Event::PointerPress {
                position: meridian_ui::PointerPosition { x: 5, y: 5 },
                button: meridian_ui::PointerButton::Left,
            }
        );
    }

    #[test]
    fn translate_press_right_yields_pointer_press_right() {
        let kind = PointerEventKind::Press {
            time: 0,
            button: 0x111,
            serial: 1,
        };
        let ev = translate_pointer_event(&kind, (0.0, 0.0)).expect("right press translates");
        assert_eq!(
            ev,
            meridian_ui::Event::PointerPress {
                position: meridian_ui::PointerPosition { x: 0, y: 0 },
                button: meridian_ui::PointerButton::Right,
            }
        );
    }

    #[test]
    fn translate_press_middle_yields_pointer_press_middle() {
        let kind = PointerEventKind::Press {
            time: 0,
            button: 0x112,
            serial: 1,
        };
        let ev = translate_pointer_event(&kind, (0.0, 0.0)).expect("middle press translates");
        assert_eq!(
            ev,
            meridian_ui::Event::PointerPress {
                position: meridian_ui::PointerPosition { x: 0, y: 0 },
                button: meridian_ui::PointerButton::Middle,
            }
        );
    }

    #[test]
    fn translate_press_unknown_button_yields_none() {
        let kind = PointerEventKind::Press {
            time: 0,
            button: 0x200,
            serial: 1,
        };
        assert_eq!(translate_pointer_event(&kind, (0.0, 0.0)), None);
    }

    #[test]
    fn translate_release_left_yields_pointer_release_left() {
        let kind = PointerEventKind::Release {
            time: 0,
            button: 0x110,
            serial: 1,
        };
        let ev = translate_pointer_event(&kind, (0.0, 0.0)).expect("left release translates");
        assert_eq!(
            ev,
            meridian_ui::Event::PointerRelease {
                position: meridian_ui::PointerPosition { x: 0, y: 0 },
                button: meridian_ui::PointerButton::Left,
            }
        );
    }

    #[test]
    fn translate_axis_yields_none() {
        let kind = PointerEventKind::Axis {
            time: 0,
            horizontal: AxisScroll::default(),
            vertical: AxisScroll::default(),
            source: None,
        };
        assert_eq!(translate_pointer_event(&kind, (0.0, 0.0)), None);
    }

    #[test]
    fn translate_position_truncates_to_i32() {
        let kind = PointerEventKind::Motion { time: 0 };
        let ev = translate_pointer_event(&kind, (10.7, 20.9)).expect("motion translates");
        assert_eq!(
            ev,
            meridian_ui::Event::PointerMove {
                position: meridian_ui::PointerPosition { x: 10, y: 20 }
            }
        );
    }
}
