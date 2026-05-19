use tiny_skia::PixmapMut;

use crate::{event::WidgetState, paint::Rect, style::Theme, widget::Widget};

use super::{LayoutNode, LayoutTree};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderError {
    LayoutTreeShapeMismatch,
}

pub fn render(
    root: &dyn Widget,
    layout: &LayoutTree,
    canvas: &mut PixmapMut<'_>,
    theme: &Theme,
    state_fn: &dyn Fn(&[usize]) -> WidgetState,
) -> Result<(), RenderError> {
    let mut path = Vec::with_capacity(16);
    render_node(root, &layout.root, 0, 0, canvas, theme, state_fn, &mut path)
}

pub fn render_idle(
    root: &dyn Widget,
    layout: &LayoutTree,
    canvas: &mut PixmapMut<'_>,
    theme: &Theme,
) -> Result<(), RenderError> {
    render(root, layout, canvas, theme, &|_| WidgetState::Idle)
}

#[allow(clippy::too_many_arguments)]
fn render_node(
    widget: &dyn Widget,
    layout: &LayoutNode,
    parent_x: i32,
    parent_y: i32,
    canvas: &mut PixmapMut<'_>,
    theme: &Theme,
    state_fn: &dyn Fn(&[usize]) -> WidgetState,
    path: &mut Vec<usize>,
) -> Result<(), RenderError> {
    let absolute = Rect {
        x: parent_x + layout.rect.x,
        y: parent_y + layout.rect.y,
        width: layout.rect.width,
        height: layout.rect.height,
    };
    let state = state_fn(path.as_slice());
    widget.paint(absolute, canvas, theme, state);

    let children = widget.children();
    if children.len() != layout.children.len() {
        return Err(RenderError::LayoutTreeShapeMismatch);
    }

    for (i, (child_widget, child_layout)) in children.iter().zip(layout.children.iter()).enumerate()
    {
        path.push(i);
        render_node(
            child_widget.as_ref(),
            child_layout,
            absolute.x,
            absolute.y,
            canvas,
            theme,
            state_fn,
            path,
        )?;
        path.pop();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use taffy::prelude::{length, Size, Style};
    use tiny_skia::Pixmap;

    use crate::{
        event::WidgetState,
        paint::{compute_layout, render, render_idle, PixelSize},
        style::{Palette, Theme},
        widget::{Button, Container, Tile, TileSize, Widget},
    };

    // ── Pre-existing tests adapted to render_idle ──────────────────────────

    #[test]
    fn render_smoke_does_not_crash() {
        let child = Box::new(Container::leaf(Style {
            size: Size {
                width: length(50.0),
                height: length(50.0),
            },
            ..Default::default()
        }));
        let root = Container::new(
            Style {
                size: Size {
                    width: length(100.0),
                    height: length(100.0),
                },
                ..Default::default()
            },
            vec![child],
        );
        let layout = compute_layout(
            &root,
            PixelSize {
                width: 100,
                height: 100,
            },
        )
        .expect("layout computes");
        let mut pixmap = Pixmap::new(100, 100).expect("pixmap");
        let mut canvas = pixmap.as_mut();

        render_idle(&root, &layout, &mut canvas, &Theme::TOKYO_NIGHT_METRO)
            .expect("render completes");
    }

    #[test]
    fn render_accumulates_offset_across_nested_containers() {
        let spacer = Box::new(Container::leaf(Style {
            size: Size {
                width: length(100.0),
                height: length(40.0),
            },
            ..Default::default()
        }));
        let button = Box::new(Button::new("p", Palette::TOKYO_NIGHT_METRO.accent, 100, 40))
            as Box<dyn Widget>;
        let root = Container::column(0, vec![spacer, button]);

        let layout = compute_layout(
            &root,
            PixelSize {
                width: 100,
                height: 100,
            },
        )
        .expect("layout computes");

        let mut pixmap = Pixmap::new(100, 100).expect("pixmap");
        let mut canvas = pixmap.as_mut();
        render_idle(&root, &layout, &mut canvas, &Theme::TOKYO_NIGHT_METRO)
            .expect("render completes");
        drop(canvas);

        let top_px = pixmap.pixel(50, 5).expect("top pixel");
        assert_eq!(top_px.alpha(), 0, "spacer area must remain unpainted");

        let stripe_px = pixmap.pixel(50, 41).expect("stripe pixel");
        let body_px = pixmap.pixel(50, 60).expect("body pixel");
        assert!(stripe_px.alpha() > 0, "stripe must be painted at y=40+");
        assert!(body_px.alpha() > 0, "body must be painted at y=40+");
    }

    // ── New tests: widget_state ────────────────────────────────────────────

    #[test]
    fn widget_state_default_is_idle() {
        assert_eq!(WidgetState::default(), WidgetState::Idle);
    }

    #[test]
    fn widget_state_eq_per_variant() {
        assert_eq!(WidgetState::Idle, WidgetState::Idle);
        assert_eq!(WidgetState::Hovered, WidgetState::Hovered);
        assert_eq!(WidgetState::Pressed, WidgetState::Pressed);
        assert_ne!(WidgetState::Idle, WidgetState::Hovered);
        assert_ne!(WidgetState::Hovered, WidgetState::Pressed);
    }

    // ── New tests: tile/button paint state variants ────────────────────────

    #[test]
    fn tile_paint_hovered_differs_from_idle() {
        let tile = Tile::new("test", Palette::TOKYO_NIGHT_METRO.accent, TileSize::Wide);
        let (w, h) = TileSize::Wide.dimensions();

        let mut idle_pix = Pixmap::new(w as u32, h as u32).expect("pixmap");
        tile.paint(
            crate::paint::Rect {
                x: 0,
                y: 0,
                width: w,
                height: h,
            },
            &mut idle_pix.as_mut(),
            &Theme::TOKYO_NIGHT_METRO,
            WidgetState::Idle,
        );
        drop(idle_pix.as_mut());

        let mut hover_pix = Pixmap::new(w as u32, h as u32).expect("pixmap");
        tile.paint(
            crate::paint::Rect {
                x: 0,
                y: 0,
                width: w,
                height: h,
            },
            &mut hover_pix.as_mut(),
            &Theme::TOKYO_NIGHT_METRO,
            WidgetState::Hovered,
        );
        drop(hover_pix.as_mut());

        let body_x = (w / 2) as u32;
        let body_y = (h / 2) as u32;
        assert_ne!(
            idle_pix.pixel(body_x, body_y),
            hover_pix.pixel(body_x, body_y),
            "hovered body pixel must differ from idle"
        );

        let stripe_x = (w / 2) as u32;
        let stripe_y = 1_u32;
        assert_eq!(
            idle_pix.pixel(stripe_x, stripe_y),
            hover_pix.pixel(stripe_x, stripe_y),
            "stripe must be identical across idle/hovered"
        );
    }

    #[test]
    fn tile_paint_pressed_differs_from_idle() {
        let tile = Tile::new("test", Palette::TOKYO_NIGHT_METRO.accent, TileSize::Wide);
        let (w, h) = TileSize::Wide.dimensions();

        let mut idle_pix = Pixmap::new(w as u32, h as u32).expect("pixmap");
        tile.paint(
            crate::paint::Rect {
                x: 0,
                y: 0,
                width: w,
                height: h,
            },
            &mut idle_pix.as_mut(),
            &Theme::TOKYO_NIGHT_METRO,
            WidgetState::Idle,
        );
        drop(idle_pix.as_mut());

        let mut pressed_pix = Pixmap::new(w as u32, h as u32).expect("pixmap");
        tile.paint(
            crate::paint::Rect {
                x: 0,
                y: 0,
                width: w,
                height: h,
            },
            &mut pressed_pix.as_mut(),
            &Theme::TOKYO_NIGHT_METRO,
            WidgetState::Pressed,
        );
        drop(pressed_pix.as_mut());

        let body_x = (w / 2) as u32;
        let body_y = (h / 2) as u32;
        assert_ne!(
            idle_pix.pixel(body_x, body_y),
            pressed_pix.pixel(body_x, body_y),
            "pressed body pixel must differ from idle"
        );

        let stripe_x = (w / 2) as u32;
        let stripe_y = 1_u32;
        assert_eq!(
            idle_pix.pixel(stripe_x, stripe_y),
            pressed_pix.pixel(stripe_x, stripe_y),
            "stripe must be identical across idle/pressed"
        );
    }

    #[test]
    fn button_paint_hovered_differs_from_idle() {
        let button = Button::new("p", Palette::TOKYO_NIGHT_METRO.accent, 72, 40);

        let mut idle_pix = Pixmap::new(72, 40).expect("pixmap");
        button.paint(
            crate::paint::Rect {
                x: 0,
                y: 0,
                width: 72,
                height: 40,
            },
            &mut idle_pix.as_mut(),
            &Theme::TOKYO_NIGHT_METRO,
            WidgetState::Idle,
        );
        drop(idle_pix.as_mut());

        let mut hover_pix = Pixmap::new(72, 40).expect("pixmap");
        button.paint(
            crate::paint::Rect {
                x: 0,
                y: 0,
                width: 72,
                height: 40,
            },
            &mut hover_pix.as_mut(),
            &Theme::TOKYO_NIGHT_METRO,
            WidgetState::Hovered,
        );
        drop(hover_pix.as_mut());

        let body_x = 36_u32;
        let body_y = 20_u32;
        assert_ne!(
            idle_pix.pixel(body_x, body_y),
            hover_pix.pixel(body_x, body_y),
            "hovered button body pixel must differ from idle"
        );
    }

    // ── New tests: render path tracking ────────────────────────────────────

    #[test]
    fn render_node_path_root_is_empty() {
        let called = Cell::new(false);
        let child = Box::new(Container::leaf(Style {
            size: Size {
                width: length(10.0),
                height: length(10.0),
            },
            ..Default::default()
        }));
        let root = Container::new(Style::default(), vec![child]);
        let layout = compute_layout(
            &root,
            PixelSize {
                width: 100,
                height: 100,
            },
        )
        .expect("layout computes");
        let mut pixmap = Pixmap::new(100, 100).expect("pixmap");

        let _ = render(
            &root,
            &layout,
            &mut pixmap.as_mut(),
            &Theme::TOKYO_NIGHT_METRO,
            &|path| {
                if path.is_empty() {
                    called.set(true);
                }
                WidgetState::Idle
            },
        );

        assert!(
            called.get(),
            "state_fn must be called with &[] for the root"
        );
    }

    #[test]
    fn render_node_path_indices_for_child() {
        let observed = Cell::new(Vec::new());
        let child_a = Box::new(Container::leaf(Style::default()));
        let child_b = Box::new(Container::leaf(Style::default()));
        let root = Container::new(Style::default(), vec![child_a, child_b]);
        let layout = compute_layout(
            &root,
            PixelSize {
                width: 100,
                height: 100,
            },
        )
        .expect("layout computes");
        let mut pixmap = Pixmap::new(100, 100).expect("pixmap");

        let _ = render(
            &root,
            &layout,
            &mut pixmap.as_mut(),
            &Theme::TOKYO_NIGHT_METRO,
            &|path| {
                if !path.is_empty() {
                    let mut v = observed.take();
                    v.push(path.to_vec());
                    observed.set(v);
                }
                WidgetState::Idle
            },
        );

        let paths = observed.into_inner();
        assert!(paths.contains(&vec![0]), "must observe path [0]");
        assert!(paths.contains(&vec![1]), "must observe path [1]");
    }

    // ── New test: render_idle pixel-identity with old render (now also idle) ─

    #[test]
    fn render_with_idle_state_matches_legacy() {
        use taffy::prelude::FlexDirection;

        let tile = Box::new(Tile::new(
            "idle",
            Palette::TOKYO_NIGHT_METRO.accent,
            TileSize::Medium,
        )) as Box<dyn Widget>;
        let button = Box::new(Button::new("b", Palette::TOKYO_NIGHT_METRO.warning, 48, 48))
            as Box<dyn Widget>;
        let root = Container::new(
            Style {
                flex_direction: FlexDirection::Row,
                size: Size {
                    width: length(300.0),
                    height: length(200.0),
                },
                ..Default::default()
            },
            vec![tile, button],
        );
        let layout = compute_layout(
            &root,
            PixelSize {
                width: 300,
                height: 200,
            },
        )
        .expect("layout computes");

        let mut a = Pixmap::new(300, 200).expect("pixmap");
        let mut b = Pixmap::new(300, 200).expect("pixmap");
        render_idle(&root, &layout, &mut a.as_mut(), &Theme::TOKYO_NIGHT_METRO)
            .expect("render_idle");
        render(
            &root,
            &layout,
            &mut b.as_mut(),
            &Theme::TOKYO_NIGHT_METRO,
            &|_| WidgetState::Idle,
        )
        .expect("render with idle fn");

        assert_eq!(
            a.data(),
            b.data(),
            "render_idle and render with idle state_fn must produce identical pixels"
        );
    }

    // ── New tests: lerp_color ──────────────────────────────────────────────

    #[test]
    fn lerp_color_endpoints() {
        let a = crate::style::Color::rgb(0x24, 0x28, 0x3b);
        let b = crate::style::Color::rgb(0xFF, 0xFF, 0xFF);
        assert_eq!(a.lerp(b, 0.0), a);
        assert_eq!(a.lerp(b, 1.0), b);
    }

    #[test]
    fn lerp_color_preserves_alpha() {
        let a = crate::style::Color::rgba(0x24, 0x28, 0x3b, 0x80);
        let b = crate::style::Color::rgba(0xFF, 0xFF, 0xFF, 0xFF);
        let result = a.lerp(b, 0.5);
        assert_eq!(result.a, 0x80);
    }
}
