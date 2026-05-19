use taffy::prelude::{
    length, AlignContent, AlignItems, Display, FlexDirection, FlexWrap, JustifyContent, Size, Style,
};
use tiny_skia::PixmapMut;

use crate::{event::WidgetState, paint::Rect, style::Theme};

/// Trait implemented by all renderable UI widgets.
///
/// The style method is consumed during the setup/layout phase.
/// The paint method is called in the render path and must not allocate.
pub trait Widget {
    /// Return the taffy style used to build this node in the layout tree.
    fn style(&self) -> Style;

    /// Paint this widget into the assigned rectangle.
    ///
    /// Contract: must be allocation-free and side-effect free.
    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState);

    /// Optional stable identifier used for action dispatch.
    ///
    /// This field is ignored by all render paths.
    fn id(&self) -> Option<&'static str> {
        None
    }

    /// Optional launch info for widgets that represent launchable apps.
    ///
    /// Returns `(exec, args)` if this widget can be launched.
    /// Render paths ignore this field.
    fn launch_info(&self) -> Option<(&str, &[String])> {
        None
    }

    /// Optional exec path for widgets that represent launchable apps.
    ///
    /// Returns the program name if this widget can be launched.
    /// Simpler alternative to `launch_info` for widgets without args.
    /// Render paths ignore this field.
    fn launch_exec(&self) -> Option<&str> {
        None
    }

    /// Child widgets in tree order.
    fn children(&self) -> &[Box<dyn Widget>] {
        &[]
    }
}

/// Minimal container widget used for bootstrapping and tests.
pub struct Container {
    style: Style,
    children: Vec<Box<dyn Widget>>,
}

impl Container {
    pub fn new(style: Style, children: Vec<Box<dyn Widget>>) -> Self {
        Self { style, children }
    }

    pub fn leaf(style: Style) -> Self {
        Self::new(style, Vec::new())
    }

    pub fn centered_viewport(width: u32, height: u32, children: Vec<Box<dyn Widget>>) -> Self {
        Self::new(
            Style {
                justify_content: Some(JustifyContent::Center),
                align_items: Some(AlignItems::Center),
                size: Size {
                    width: length(width as f32),
                    height: length(height as f32),
                },
                ..Default::default()
            },
            children,
        )
    }

    pub fn flow(
        viewport_width: u32,
        viewport_height: u32,
        gap: i32,
        children: Vec<Box<dyn Widget>>,
    ) -> Self {
        let gap_px = gap.max(0) as f32;
        Self::new(
            Style {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: Some(JustifyContent::Center),
                align_content: Some(AlignContent::Center),
                size: Size {
                    width: length(viewport_width as f32),
                    height: length(viewport_height as f32),
                },
                gap: Size {
                    width: length(gap_px),
                    height: length(gap_px),
                },
                ..Default::default()
            },
            children,
        )
    }

    pub fn column(gap_px: i32, children: Vec<Box<dyn Widget>>) -> Self {
        let gap = gap_px.max(0) as f32;
        Self::new(
            Style {
                flex_direction: FlexDirection::Column,
                gap: Size {
                    width: length(0.0),
                    height: length(gap),
                },
                ..Default::default()
            },
            children,
        )
    }

    pub fn grid(
        cell_size_px: i32,
        columns: u32,
        gap_px: i32,
        viewport_width: u32,
        viewport_height: u32,
        children: Vec<Box<dyn Widget>>,
    ) -> Self {
        let cell_size = cell_size_px.max(0) as f32;
        let gap = gap_px.max(0) as f32;
        let template_columns = vec![length(cell_size); columns as usize];

        Self::new(
            Style {
                display: Display::Grid,
                justify_content: Some(JustifyContent::Center),
                align_content: Some(AlignContent::Center),
                size: Size {
                    width: length(viewport_width as f32),
                    height: length(viewport_height as f32),
                },
                grid_template_columns: template_columns,
                grid_auto_rows: vec![length(cell_size)],
                gap: Size {
                    width: length(gap),
                    height: length(gap),
                },
                ..Default::default()
            },
            children,
        )
    }

    pub fn footer_row(
        viewport_width: u32,
        height: i32,
        padding_px: i32,
        cluster_gap_px: i32,
        left_children: Vec<Box<dyn Widget>>,
        right_children: Vec<Box<dyn Widget>>,
    ) -> Self {
        let padding = padding_px.max(0) as f32;
        let left_cluster = Box::new(Self::horizontal_cluster(cluster_gap_px, left_children));
        let right_cluster = Box::new(Self::horizontal_cluster(cluster_gap_px, right_children));

        Self::new(
            Style {
                flex_direction: FlexDirection::Row,
                justify_content: Some(JustifyContent::SpaceBetween),
                align_items: Some(AlignItems::Center),
                size: Size {
                    width: length(viewport_width as f32),
                    height: length(height.max(0) as f32),
                },
                padding: taffy::prelude::Rect {
                    left: length(padding),
                    right: length(padding),
                    top: length(0.0),
                    bottom: length(0.0),
                },
                ..Default::default()
            },
            vec![
                left_cluster as Box<dyn Widget>,
                right_cluster as Box<dyn Widget>,
            ],
        )
    }

    fn horizontal_cluster(gap_px: i32, children: Vec<Box<dyn Widget>>) -> Self {
        let gap = gap_px.max(0) as f32;
        Self::new(
            Style {
                flex_direction: FlexDirection::Row,
                align_items: Some(AlignItems::Center),
                gap: Size {
                    width: length(gap),
                    height: length(0.0),
                },
                ..Default::default()
            },
            children,
        )
    }
}

impl Widget for Container {
    fn style(&self) -> Style {
        self.style.clone()
    }

    fn paint(&self, _area: Rect, _canvas: &mut PixmapMut<'_>, _theme: &Theme, _state: WidgetState) {
    }

    fn children(&self) -> &[Box<dyn Widget>] {
        &self.children
    }
}

#[cfg(test)]
mod tests {
    use taffy::prelude::{length, Size, Style};

    use super::{Container, Widget};
    use crate::paint::{compute_layout, PixelSize};
    use crate::style::Palette;
    use crate::widget::{Button, Tile, TileSize};

    #[test]
    fn container_leaf_has_no_children() {
        let leaf = Container::leaf(Style::default());
        assert!(leaf.children().is_empty());
    }

    #[test]
    fn container_style_roundtrips() {
        let widget = Container::leaf(Style {
            size: Size {
                width: length(42.0),
                height: length(24.0),
            },
            ..Default::default()
        });
        let style = widget.style();
        assert_eq!(style.size.width, length(42.0));
        assert_eq!(style.size.height, length(24.0));
    }

    #[test]
    fn centered_viewport_sets_center_alignment_and_size() {
        let container = Container::centered_viewport(880, 620, Vec::new());
        let style = container.style();
        assert_eq!(
            style.justify_content,
            Some(taffy::prelude::JustifyContent::Center)
        );
        assert_eq!(style.align_items, Some(taffy::prelude::AlignItems::Center));
        assert_eq!(style.size.width, length(880.0));
        assert_eq!(style.size.height, length(620.0));
    }

    #[test]
    fn flow_wraps_mixed_child_sizes() {
        let children: Vec<Box<dyn Widget>> = vec![
            Box::new(Container::leaf(Style {
                size: Size {
                    width: length(200.0),
                    height: length(80.0),
                },
                ..Default::default()
            })),
            Box::new(Container::leaf(Style {
                size: Size {
                    width: length(140.0),
                    height: length(90.0),
                },
                ..Default::default()
            })),
            Box::new(Container::leaf(Style {
                size: Size {
                    width: length(120.0),
                    height: length(70.0),
                },
                ..Default::default()
            })),
        ];
        let root = Container::flow(360, 260, 8, children);
        let layout = compute_layout(
            &root,
            PixelSize {
                width: 360,
                height: 260,
            },
        )
        .expect("layout computes");

        assert_eq!(layout.root.children.len(), 3);
        let first = &layout.root.children[0].rect;
        let second = &layout.root.children[1].rect;
        let third = &layout.root.children[2].rect;

        assert_eq!(first.width, 200);
        assert_eq!(second.width, 140);
        assert_eq!(third.width, 120);
        assert_eq!(first.height, 80);
        assert_eq!(second.height, 90);
        assert_eq!(third.height, 70);

        assert_eq!(first.y, second.y);
        assert!(third.y > second.y);
    }

    #[test]
    fn grid_places_tiles_on_cell_boundaries() {
        let cell_size = 96;
        let gap = 8;
        let pitch = cell_size + gap;
        let children: Vec<Box<dyn Widget>> = vec![
            Box::new(Tile::new(
                "large",
                Palette::TOKYO_NIGHT_METRO.accent_alt,
                TileSize::Large,
            )),
            Box::new(Tile::new(
                "wide",
                Palette::TOKYO_NIGHT_METRO.accent,
                TileSize::Wide,
            )),
            Box::new(Tile::new(
                "medium-a",
                Palette::TOKYO_NIGHT_METRO.success,
                TileSize::Medium,
            )),
            Box::new(Tile::new(
                "medium-b",
                Palette::TOKYO_NIGHT_METRO.warning,
                TileSize::Medium,
            )),
            Box::new(Tile::new(
                "small-a",
                Palette::TOKYO_NIGHT_METRO.error,
                TileSize::Small,
            )),
            Box::new(Tile::new(
                "small-b",
                Palette::TOKYO_NIGHT_METRO.accent,
                TileSize::Small,
            )),
        ];

        let root = Container::grid(cell_size, 8, gap, 1200, 900, children);
        let layout = compute_layout(
            &root,
            PixelSize {
                width: 1200,
                height: 900,
            },
        )
        .expect("layout computes");

        assert_eq!(layout.root.children.len(), 6);
        let rects = &layout.root.children;
        let origin_x = rects
            .iter()
            .map(|node| node.rect.x)
            .min()
            .expect("has children");
        let origin_y = rects
            .iter()
            .map(|node| node.rect.y)
            .min()
            .expect("has children");

        let relative = |index: usize| {
            let rect = rects[index].rect;
            (rect.x - origin_x, rect.y - origin_y)
        };

        assert_eq!(relative(0), (0, 0));
        assert_eq!(relative(1), (pitch * 4, 0));
        assert_eq!(relative(2), (pitch * 4, pitch * 2));
        assert_eq!(relative(3), (pitch * 6, pitch * 2));
        assert_eq!(relative(4), (0, pitch * 4));
        assert_eq!(relative(5), (pitch, pitch * 4));
    }

    #[test]
    fn column_stacks_children_vertically_with_gap() {
        let children: Vec<Box<dyn Widget>> = vec![
            Box::new(Container::leaf(Style {
                size: Size {
                    width: length(100.0),
                    height: length(80.0),
                },
                ..Default::default()
            })),
            Box::new(Container::leaf(Style {
                size: Size {
                    width: length(100.0),
                    height: length(40.0),
                },
                ..Default::default()
            })),
        ];
        let root = Container::column(12, children);
        let layout = compute_layout(
            &root,
            PixelSize {
                width: 400,
                height: 400,
            },
        )
        .expect("layout computes");

        let first = layout.root.children[0].rect;
        let second = layout.root.children[1].rect;
        assert_eq!(second.y - first.y, 92);
    }

    #[test]
    fn footer_row_places_left_and_right_clusters() {
        let left = vec![Box::new(Button::new(
            "switch",
            Palette::TOKYO_NIGHT_METRO.accent,
            144,
            48,
        )) as Box<dyn Widget>];
        let right = vec![
            Box::new(Button::new("a", Palette::TOKYO_NIGHT_METRO.error, 48, 48)) as Box<dyn Widget>,
            Box::new(Button::new("b", Palette::TOKYO_NIGHT_METRO.warning, 48, 48))
                as Box<dyn Widget>,
            Box::new(Button::new("c", Palette::TOKYO_NIGHT_METRO.accent, 48, 48))
                as Box<dyn Widget>,
        ];
        let root = Container::footer_row(880, 56, 28, 8, left, right);
        let layout = compute_layout(
            &root,
            PixelSize {
                width: 880,
                height: 56,
            },
        )
        .expect("layout computes");

        assert_eq!(layout.root.children.len(), 2);
        let left_cluster = layout.root.children[0].rect;
        let right_cluster = layout.root.children[1].rect;
        assert_eq!(left_cluster.x, 28);
        assert_eq!(right_cluster.x + right_cluster.width, 880 - 28);
        assert!(right_cluster.x > left_cluster.x + left_cluster.width);
    }
}
