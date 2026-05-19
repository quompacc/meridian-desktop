//! UI preview sandbox renderer for `--ui-preview`.
//!
//! This path allocates a fresh `tiny_skia::Pixmap` per draw call and is intended
//! for low-frequency sandbox previewing. If this is promoted to steady-state UI,
//! pixmap reuse should be added.

use meridian_ui::{
    compute_layout, render,
    style::Palette,
    widget::{tile::TILE_BASE_SIZE, Button, Container, Widget},
    PixelSize, Theme, TileSize, WidgetState,
};
use meridian_ui::{
    effect::{paint_metro_surface, paint_text},
    paint::Rect,
    style::Color,
};
use tiny_skia::{Pixmap, PixmapMut};
use tiny_skia::{PixmapPaint, Transform};

use crate::icons::{IconCache, IconImage};
use crate::launcher::DesktopApp;

use meridian_ui::widget::tile::{
    STRIPE_HEIGHT, TILE_LABEL_BASELINE_FROM_BOTTOM, TILE_LABEL_FONT_DEFAULT_PX,
    TILE_LABEL_FONT_SMALL_PX, TILE_LABEL_PADDING_X,
};

const FOOTER_HEIGHT: i32 = 56;
const FOOTER_PADDING_X: i32 = 28;
const FOOTER_CLUSTER_GAP: i32 = 8;
const FOOTER_SWITCH_WIDTH: i32 = 144;
const FOOTER_SWITCH_HEIGHT: i32 = 48;
const FOOTER_POWER_BUTTON_SIZE: i32 = 48;

const TILE_ICON_SIZE: u32 = 96;
const POWER_ICON_SIZE: u32 = 32;

fn icon_image_to_pixmap(img: &IconImage) -> Option<Pixmap> {
    let w = img.width;
    let h = img.height;
    let mut pixmap = Pixmap::new(w, h)?;
    let data = pixmap.data_mut();
    for (i, chunk) in img.bgra.chunks_exact(4).enumerate() {
        let b = chunk[0];
        let g = chunk[1];
        let r = chunk[2];
        let a = chunk[3];
        let out_idx = i * 4;
        data[out_idx] = ((r as u16 * a as u16) / 255) as u8;
        data[out_idx + 1] = ((g as u16 * a as u16) / 255) as u8;
        data[out_idx + 2] = ((b as u16 * a as u16) / 255) as u8;
        data[out_idx + 3] = a;
    }
    Some(pixmap)
}

pub(crate) struct DynTile {
    label: Box<str>,
    exec: Box<str>,
    accent: Color,
    size: TileSize,
    icon: Option<Pixmap>,
}

impl Widget for DynTile {
    fn style(&self) -> meridian_ui::WidgetStyle {
        let (col_span, row_span) = self.size.cell_span();
        meridian_ui::WidgetStyle {
            grid_column: meridian_ui::grid_span(col_span as u16),
            grid_row: meridian_ui::grid_span(row_span as u16),
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let body_color = match state {
            WidgetState::Idle => theme.palette.surface,
            WidgetState::Hovered => theme
                .palette
                .surface
                .lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.15),
            WidgetState::Pressed => theme.palette.surface.lerp(Color::rgb(0, 0, 0), 0.18),
        };
        paint_metro_surface(canvas, area, body_color, self.accent, theme, STRIPE_HEIGHT);
        let font_size = match self.size {
            TileSize::Small => TILE_LABEL_FONT_SMALL_PX,
            _ => TILE_LABEL_FONT_DEFAULT_PX,
        };
        paint_text(
            canvas,
            &self.label,
            area.x + TILE_LABEL_PADDING_X,
            area.y + area.height - TILE_LABEL_BASELINE_FROM_BOTTOM,
            font_size,
            theme.palette.text,
        );

        if let Some(ref icon) = self.icon {
            let iw = icon.width() as i32;
            let ih = icon.height() as i32;
            let x = area.x + (area.width - iw) / 2;
            let icon_center_y = area.y + (area.height as f32 * 0.35) as i32;
            let y = icon_center_y - ih / 2;
            canvas.draw_pixmap(
                x,
                y,
                icon.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
        }
    }

    fn launch_exec(&self) -> Option<&str> {
        Some(&self.exec)
    }
}

pub(crate) fn build_ui_preview_widget_tree(
    width: u32,
    height: u32,
    apps: &[DesktopApp],
    icon_cache: &IconCache,
) -> Box<dyn Widget> {
    let theme = Theme::TOKYO_NIGHT_METRO;
    let gap = theme.spacing.md;
    let pal = Palette::TOKYO_NIGHT_METRO;
    let accent_cycle = [
        pal.accent_alt,
        pal.accent,
        pal.warning,
        pal.accent,
        pal.accent_alt,
        pal.success,
        pal.warning,
        pal.error,
        pal.accent,
        pal.accent_alt,
        pal.success,
    ];
    let size_cycle = [
        TileSize::Large,
        TileSize::Wide,
        TileSize::Wide,
        TileSize::Small,
        TileSize::Small,
        TileSize::Small,
        TileSize::Small,
        TileSize::Small,
        TileSize::Small,
        TileSize::Small,
        TileSize::Small,
    ];
    let max_apps = apps.len().min(11);
    let tiles: Vec<Box<dyn Widget>> = apps[..max_apps]
        .iter()
        .enumerate()
        .map(|(i, app)| {
            let accent = accent_cycle[i];
            let size = size_cycle[i];
            let icon_name = app.icon_name.as_deref().unwrap_or("");
            let maybe_pixmap = icon_cache
                .lookup(icon_name, TILE_ICON_SIZE)
                .and_then(icon_image_to_pixmap);
            Box::new(DynTile {
                label: app.name.clone().into_boxed_str(),
                exec: app.program.clone().into_boxed_str(),
                accent,
                size,
                icon: maybe_pixmap,
            }) as Box<dyn Widget>
        })
        .collect();
    let mosaic_height = height.saturating_sub(FOOTER_HEIGHT.max(0) as u32);
    let mosaic_grid = Container::grid(TILE_BASE_SIZE, 8, gap, width, mosaic_height, tiles);
    let mosaic_section = Container::centered_viewport(
        width,
        mosaic_height,
        vec![Box::new(mosaic_grid) as Box<dyn Widget>],
    );

    let footer_left = vec![Box::new(Button::with_id(
        "apps-switch",
        "Apps",
        pal.accent,
        FOOTER_SWITCH_WIDTH,
        FOOTER_SWITCH_HEIGHT,
    )) as Box<dyn Widget>];

    let power_off_icon = icon_cache
        .lookup("system-shutdown", POWER_ICON_SIZE)
        .and_then(icon_image_to_pixmap);
    let power_restart_icon = icon_cache
        .lookup("system-reboot", POWER_ICON_SIZE)
        .and_then(icon_image_to_pixmap);
    let power_sleep_icon = icon_cache
        .lookup("system-suspend", POWER_ICON_SIZE)
        .and_then(icon_image_to_pixmap);
    let power_lock_icon = icon_cache
        .lookup("system-lock-screen", POWER_ICON_SIZE)
        .and_then(icon_image_to_pixmap);
    let power_logout_icon = icon_cache
        .lookup("system-log-out", POWER_ICON_SIZE)
        .and_then(icon_image_to_pixmap);

    let footer_right = vec![
        Box::new(Button::with_id_and_icon(
            "power-off",
            "Off",
            pal.error,
            FOOTER_POWER_BUTTON_SIZE,
            FOOTER_POWER_BUTTON_SIZE,
            power_off_icon,
        )) as Box<dyn Widget>,
        Box::new(Button::with_id_and_icon(
            "power-restart",
            "Rst",
            pal.warning,
            FOOTER_POWER_BUTTON_SIZE,
            FOOTER_POWER_BUTTON_SIZE,
            power_restart_icon,
        )) as Box<dyn Widget>,
        Box::new(Button::with_id_and_icon(
            "power-sleep",
            "Zzz",
            pal.accent,
            FOOTER_POWER_BUTTON_SIZE,
            FOOTER_POWER_BUTTON_SIZE,
            power_sleep_icon,
        )) as Box<dyn Widget>,
        Box::new(Button::with_id_and_icon(
            "power-lock",
            "Lock",
            pal.accent_alt,
            FOOTER_POWER_BUTTON_SIZE,
            FOOTER_POWER_BUTTON_SIZE,
            power_lock_icon,
        )) as Box<dyn Widget>,
        Box::new(Button::with_id_and_icon(
            "power-logout",
            "Out",
            pal.success,
            FOOTER_POWER_BUTTON_SIZE,
            FOOTER_POWER_BUTTON_SIZE,
            power_logout_icon,
        )) as Box<dyn Widget>,
    ];
    let footer = Container::footer_row(
        width,
        FOOTER_HEIGHT,
        FOOTER_PADDING_X,
        FOOTER_CLUSTER_GAP,
        footer_left,
        footer_right,
    );

    Box::new(Container::column(
        0,
        vec![
            Box::new(mosaic_section) as Box<dyn Widget>,
            Box::new(footer) as Box<dyn Widget>,
        ],
    ))
}

pub(crate) fn draw_ui_preview_sandbox(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    apps: &[DesktopApp],
    icon_cache: &IconCache,
    state_fn: &dyn Fn(&[usize]) -> WidgetState,
) {
    let expected_len = (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4);
    if canvas.len() != expected_len {
        return;
    }

    let Some(mut pixmap) = Pixmap::new(width, height) else {
        return;
    };

    let theme = Theme::TOKYO_NIGHT_METRO;
    pixmap.fill(to_tiny_skia_color(theme.palette.background));

    let root = build_ui_preview_widget_tree(width, height, apps, icon_cache);

    if let Ok(layout) = compute_layout(&*root, PixelSize { width, height }) {
        let mut pixmap_canvas = pixmap.as_mut();
        let _ = render(&*root, &layout, &mut pixmap_canvas, &theme, state_fn);
    }

    blit_rgba_to_argb(pixmap.data(), canvas);
}

fn blit_rgba_to_argb(src: &[u8], dst: &mut [u8]) {
    if src.len() != dst.len() || !src.len().is_multiple_of(4) {
        return;
    }

    for (rgba, argb) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
        argb[0] = rgba[2];
        argb[1] = rgba[1];
        argb[2] = rgba[0];
        argb[3] = rgba[3];
    }
}

fn to_tiny_skia_color(color: meridian_ui::style::Color) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(color.r, color.g, color.b, color.a)
}

#[cfg(test)]
mod tests {
    use meridian_ui::WidgetState;

    use super::{
        blit_rgba_to_argb, build_ui_preview_widget_tree, draw_ui_preview_sandbox, DynTile,
    };
    use crate::icons::IconCache;

    #[test]
    fn blit_rgba_to_argb_swaps_red_and_blue() {
        let src = [0x12, 0x34, 0x56, 0x78];
        let mut dst = [0_u8; 4];

        blit_rgba_to_argb(&src, &mut dst);

        assert_eq!(dst, [0x56, 0x34, 0x12, 0x78]);
    }

    #[test]
    fn blit_twice_roundtrips_to_original() {
        let src = [0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0];
        let mut mid = [0_u8; 8];
        let mut dst = [0_u8; 8];

        blit_rgba_to_argb(&src, &mut mid);
        blit_rgba_to_argb(&mid, &mut dst);

        assert_eq!(dst, src);
    }

    #[test]
    fn draw_ui_preview_sandbox_smoke_modifies_canvas() {
        let width = 128;
        let height = 96;
        let mut canvas = vec![0_u8; (width * height * 4) as usize];
        let icon_cache = IconCache::new();

        draw_ui_preview_sandbox(&mut canvas, width, height, &[], &icon_cache, &|_| {
            WidgetState::Idle
        });

        assert!(canvas.iter().any(|byte| *byte != 0));
    }

    #[test]
    fn build_ui_preview_widget_tree_has_root_column_with_two_sections() {
        let icon_cache = IconCache::new();
        let tree = build_ui_preview_widget_tree(880, 620, &[], &icon_cache);
        let children = tree.children();
        assert_eq!(children.len(), 2, "root column should have 2 children");
        // Mosaic section
        let mosaic = &children[0];
        assert!(!mosaic.children().is_empty(), "mosaic should contain grid");
        // Footer
        let footer = &children[1];
        assert!(
            !footer.children().is_empty(),
            "footer should contain left/right clusters"
        );
    }

    #[test]
    fn icon_image_to_pixmap_bgra_to_premul() {
        use super::icon_image_to_pixmap;
        use crate::icons::IconImage;

        let img = IconImage {
            width: 1,
            height: 1,
            bgra: vec![0, 0, 255, 128],
        };
        let pixmap = icon_image_to_pixmap(&img).expect("pixmap");
        assert_eq!(pixmap.width(), 1);
        assert_eq!(pixmap.height(), 1);
        let px = pixmap.pixel(0, 0).expect("pixel");
        // BGRA [0,0,255,128]: R=255*128/255=128, G=0, B=0, A=128
        assert_eq!(px.red(), 128);
        assert_eq!(px.green(), 0);
        assert_eq!(px.blue(), 0);
        assert_eq!(px.alpha(), 128);
    }

    #[test]
    fn dyn_tile_launch_exec_returns_program() {
        use meridian_ui::Widget;
        let tile = DynTile {
            label: "Firefox".into(),
            exec: "firefox".into(),
            accent: meridian_ui::style::Color::rgb(0, 0, 0),
            size: meridian_ui::TileSize::Small,
            icon: None,
        };
        assert_eq!(tile.launch_exec(), Some("firefox"));
    }
}
