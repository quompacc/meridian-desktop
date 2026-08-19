fn draw_menu_icon(
    canvas: &mut tiny_skia::PixmapMut<'_>,
    ox: f32,
    oy: f32,
    sz: f32,
    icon: MenuIcon,
    color: meridian_ui::style::Color,
) {
    use tiny_skia::{FillRule, LineCap, LineJoin, Paint, PathBuilder, Stroke, Transform};
    let mut paint = Paint {
        anti_alias: true,
        ..Default::default()
    };
    paint.set_color_rgba8(color.r, color.g, color.b, color.a);
    let sw = (sz / 16.0 * 1.5).max(1.0);
    let stroke = Stroke {
        width: sw,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Default::default()
    };
    let m = |x: f32, y: f32| (ox + x / 16.0 * sz, oy + y / 16.0 * sz);
    let stroke_pb = |canvas: &mut tiny_skia::PixmapMut<'_>, pb: PathBuilder| {
        if let Some(path) = pb.finish() {
            canvas.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    };
    match icon {
        MenuIcon::Terminal => {
            let mut pb = PathBuilder::new();
            let (l, t) = m(2.0, 3.5);
            let (r, b) = m(14.0, 12.5);
            pb.move_to(l, t);
            pb.line_to(r, t);
            pb.line_to(r, b);
            pb.line_to(l, b);
            pb.close();
            stroke_pb(canvas, pb);
            let mut pb2 = PathBuilder::new();
            let (a, c) = m(4.5, 6.0);
            let (d, e) = m(6.8, 8.0);
            let (f, g) = m(4.5, 10.0);
            pb2.move_to(a, c);
            pb2.line_to(d, e);
            pb2.line_to(f, g);
            stroke_pb(canvas, pb2);
            let mut pb3 = PathBuilder::new();
            let (u0, uy) = m(8.2, 10.0);
            let (u1, _) = m(11.5, 10.0);
            pb3.move_to(u0, uy);
            pb3.line_to(u1, uy);
            stroke_pb(canvas, pb3);
        }
        MenuIcon::Launcher => {
            for &(cx, cy) in &[(4.6, 4.6), (11.4, 4.6), (4.6, 11.4), (11.4, 11.4)] {
                let (px, py) = m(cx, cy);
                let half = sz / 16.0 * 2.3;
                if let Some(rect) =
                    tiny_skia::Rect::from_xywh(px - half, py - half, half * 2.0, half * 2.0)
                {
                    let mut pb = PathBuilder::new();
                    pb.push_rect(rect);
                    if let Some(path) = pb.finish() {
                        canvas.fill_path(
                            &path,
                            &paint,
                            FillRule::Winding,
                            Transform::identity(),
                            None,
                        );
                    }
                }
            }
        }
        MenuIcon::FileManager => {
            // Folder icon: body + tab
            let mut pb = PathBuilder::new();
            let (l, t) = m(2.0, 6.0);
            let (r, b) = m(14.0, 13.0);
            // folder body
            pb.move_to(l, t);
            pb.line_to(r, t);
            pb.line_to(r, b);
            pb.line_to(l, b);
            pb.close();
            stroke_pb(canvas, pb);
            // folder tab (top-left)
            let mut pb2 = PathBuilder::new();
            let (tl, tt) = m(2.0, 4.0);
            let (tr, _) = m(7.5, 4.0);
            let (_, tb) = m(0.0, 6.0);
            pb2.move_to(tl, tb);
            pb2.line_to(tl, tt);
            pb2.line_to(tr, tt);
            pb2.line_to(tr, tb);
            stroke_pb(canvas, pb2);
        }
        MenuIcon::Settings => {
            let knobs = [(4.0f32, 11.0f32), (8.0, 5.0), (12.0, 9.5)];
            for &(yy, kx) in &knobs {
                let mut pb = PathBuilder::new();
                let (l, y) = m(2.5, yy);
                let (r, _) = m(13.5, yy);
                pb.move_to(l, y);
                pb.line_to(r, y);
                stroke_pb(canvas, pb);
                let (cx, cy) = m(kx, yy);
                let mut pbk = PathBuilder::new();
                pbk.push_circle(cx, cy, sz / 16.0 * 1.7);
                if let Some(path) = pbk.finish() {
                    canvas.fill_path(
                        &path,
                        &paint,
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
            }
        }
        MenuIcon::Lock => {
            // Padlock body
            let mut pb = PathBuilder::new();
            let (bx, by) = m(4.0, 8.0);
            let (bw, bh) = (sz / 16.0 * 8.0, sz / 16.0 * 6.0);
            pb.move_to(bx, by);
            pb.line_to(bx + bw, by);
            pb.line_to(bx + bw, by + bh);
            pb.line_to(bx, by + bh);
            pb.close();
            stroke_pb(canvas, pb);
            // Shackle (top arc of padlock)
            let (cx2, cy2) = m(8.0, 8.0);
            let r2 = sz / 16.0 * 3.0;
            let mut pb2 = PathBuilder::new();
            pb2.move_to(cx2 - r2, cy2);
            pb2.cubic_to(
                cx2 - r2,
                cy2 - r2 * 1.2,
                cx2 + r2,
                cy2 - r2 * 1.2,
                cx2 + r2,
                cy2,
            );
            stroke_pb(canvas, pb2);
        }
    }
}

/// Draw a small right-pointing solid triangle at the right edge of a menu item,
/// indicating that hovering/clicking opens a submenu.
fn draw_submenu_arrow_indicator(
    canvas: &mut tiny_skia::PixmapMut<'_>,
    menu_w: i32,
    item_top: i32,
    color: meridian_ui::style::Color,
) {
    use tiny_skia::{FillRule, Paint, PathBuilder, Transform};
    let mut paint = Paint {
        anti_alias: true,
        ..Default::default()
    };
    paint.set_color_rgba8(
        color.r,
        color.g,
        color.b,
        (color.a as u32 * 160 / 255) as u8,
    );
    let cx = (menu_w - PADDING_X + 2) as f32;
    let cy = (item_top + ITEM_H / 2) as f32;
    let h = 6.0f32;
    let w = 4.0f32;
    let mut pb = PathBuilder::new();
    pb.move_to(cx - w / 2.0, cy - h / 2.0);
    pb.line_to(cx + w / 2.0, cy);
    pb.line_to(cx - w / 2.0, cy + h / 2.0);
    pb.close();
    if let Some(path) = pb.finish() {
        canvas.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

/// Render the context menu as an overlay onto the existing BGRA `canvas`.
/// `icons` is parallel to `items` (empty = no icon column).
/// `submenu_arrows`: parallel to `items`; if `true` for index `i`, a right-pointing
/// arrow is drawn at the right edge of that item to indicate a flyout.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_overlay(
    canvas: &mut [u8],
    canvas_w: u32,
    canvas_h: u32,
    state: &ContextMenuState,
    items: &[(&str, ContextMenuAction)],
    icons: &[MenuIcon],
    submenu_arrows: &[bool],
    theme_config: &ThemeConfig,
) {
    draw_overlay_with_background(
        canvas,
        canvas_w,
        canvas_h,
        state,
        items,
        icons,
        submenu_arrows,
        theme_config,
        true,
    );
}
