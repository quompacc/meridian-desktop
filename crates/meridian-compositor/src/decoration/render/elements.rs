use smallvec::SmallVec;

use meridian_config::{Decorations, ThemeColors, ThemeSurface};
use smithay::{
    backend::renderer::{
        element::{memory::MemoryRenderBufferRenderElement, solid::SolidColorRenderElement, Kind},
        gles::{element::PixelShaderElement, GlesRenderer, Uniform, UniformName, UniformType},
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Physical, Point, Rectangle, Scale, Size},
};

use crate::backend::drm::glass::GlassTitlebarInfo;

// Tuning for the hover wash on the glass-titlebar window controls. The mockup
// shows NO resting chrome — just clean grey ─□× glyphs — so the only per-button
// treatment is the hovered control's neutral wash (and its frosted veil when
// blur is on). Both are fractions of the theme's `glass_button_alpha` (capped),
// kept in one named place instead of scattered literals in the render loop.
// Final values are tuned on-screen against the reference mockup
// (GUI_CENTRALIZATION_PLAN §6 phase 1).
include!("elements/glass_buttons.rs");
include!("elements/shaders.rs");
impl DecorationManager {
    #[allow(clippy::too_many_arguments)]
    pub fn render_elements(
        &mut self,
        renderer: &mut GlesRenderer,
        surface: &WlSurface,
        window_loc: Point<i32, Logical>,
        content_size: Size<i32, Logical>,
        theme: &Decorations,
        colors: &ThemeColors,
        scale: Scale<f64>,
    ) -> SmallVec<[DecorationRenderElement; 32]> {
        if self.shadow_shader.is_none() {
            match renderer.compile_custom_pixel_shader(SHADOW_SHADER_SRC, &shadow_uniform_names()) {
                Ok(prog) => self.shadow_shader = Some(prog),
                Err(err) => tracing::warn!("shadow shader compile failed: {:?}", err),
            }
        }
        if self.rounded_quad_shader.is_none() {
            match renderer
                .compile_custom_pixel_shader(ROUNDED_QUAD_SHADER_SRC, &rounded_quad_uniform_names())
            {
                Ok(prog) => self.rounded_quad_shader = Some(prog),
                Err(err) => tracing::warn!("rounded-quad shader compile failed: {:?}", err),
            }
        }
        if self.glass_shader.is_none() {
            match renderer.compile_custom_pixel_shader(GLASS_SHADER_SRC, &glass_uniform_names()) {
                Ok(prog) => self.glass_shader = Some(prog),
                Err(err) => tracing::warn!("glass shader compile failed: {:?}", err),
            }
        }
        let shadow_shader = self.shadow_shader.clone();
        let rounded_quad_shader = self.rounded_quad_shader.clone();
        let glass_shader = self.glass_shader.clone();
        let key = Self::key(surface);
        let deco = match self.decorations.get_mut(&key) {
            Some(d) => d,
            None => {
                static MISS_LOGGED: std::sync::atomic::AtomicUsize =
                    std::sync::atomic::AtomicUsize::new(0);
                let n = MISS_LOGGED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if n < 5 {
                    tracing::warn!(
                        "decoration render: no entry for surface={:?} (known keys: {:?})",
                        key,
                        self.decorations.keys().collect::<Vec<_>>()
                    );
                }
                return SmallVec::new();
            }
        };

        if !deco.should_draw() {
            static SKIP_LOGGED: std::sync::atomic::AtomicUsize =
                std::sync::atomic::AtomicUsize::new(0);
            let n = SKIP_LOGGED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if n < 5 {
                tracing::warn!(
                    "decoration render: skip surface={:?} has_ssd={} is_fullscreen={}",
                    key,
                    deco.has_ssd,
                    deco.is_fullscreen
                );
            }
            return SmallVec::new();
        }

        let bw = deco.border_width(theme);
        let show_title = deco.should_draw_title_bar();
        let title_h = if show_title { TITLE_BAR_HEIGHT } else { 0 };
        let cw = content_size.w;
        let ch = content_size.h;
        let total_w = cw + bw * 2;

        let size_changed = deco.last_content_size != (cw, ch) || deco.last_bw != bw;
        if deco.dirty || size_changed {
            update_buffers(
                deco, theme, colors, show_title, bw, total_w, ch, title_h, cw,
            );
        }

        let x = window_loc.x;
        let y = window_loc.y;
        let ps = scale.x;
        // Window corner rounding: titlebar gets rounded *top* corners, the
        // border becomes a rounded outline, and the shadow radius follows. The
        // client content's bottom corners are clipped separately in the
        // backend (see `ClippedSurfaceRenderElement`). All disabled at radius 0.
        let cr = theme.corner_radius as i32;
        // Maximized windows are rounded too: they float as a card above the
        // panel rather than touching the bottom edge (the work area reserves
        // the panel's exclusive zone). should_draw() already excludes truly
        // undecorated/fullscreen surfaces.
        let rounded = cr > 0 && rounded_quad_shader.is_some();
        let rphys = cr as f32 * ps as f32;
        let mut elements: SmallVec<[DecorationRenderElement; 32]> = SmallVec::new();

        let phys = |lx: i32, ly: i32| -> Point<i32, Physical> {
            Point::from(((lx as f64 * ps) as i32, (ly as f64 * ps) as i32))
        };
        let phys_f64 = |lx: i32, ly: i32| phys(lx, ly).to_f64();

        let frame_metrics =
            SsdFrameMetrics::from_frame_origin(window_loc, content_size, bw, title_h);
        let chrome = SsdChromeMetrics::new(frame_metrics);

        if show_title {
            let buttons = chrome
                .button_metrics()
                .expect("titlebar buttons should exist when titlebar is shown");

            let max_kind = if deco.is_maximized {
                WindowIcon::Restore
            } else {
                WindowIcon::Maximize
            };
            let hovered = deco.hovered_button();

            let icon_pos = |rect: Rectangle<i32, Logical>| {
                let icon_x = rect.loc.x + (rect.size.w - BUTTON_ICON_PX as i32) / 2;
                let icon_y = rect.loc.y + (rect.size.h - BUTTON_ICON_PX as i32) / 2;
                (icon_x, icon_y)
            };

            // Window-control glyphs, drawn topmost over the cluster. Clean
            // line-art, always text-tinted; the hover affordance is the wash
            // behind the glyph, not a colour change on the glyph itself.
            let (close_ix, close_iy) = icon_pos(buttons.close_rect);
            if let Ok(icon) = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                phys_f64(close_ix, close_iy),
                self.icon_cache
                    .get_or_build(WindowIcon::Close, IconTint::OnSurface, colors),
                None,
                None,
                None,
                Kind::Unspecified,
            ) {
                elements.push(DecorationRenderElement::Icon(icon));
            }
            let (max_ix, max_iy) = icon_pos(buttons.maximize_rect);
            if let Ok(icon) = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                phys_f64(max_ix, max_iy),
                self.icon_cache
                    .get_or_build(max_kind, IconTint::OnSurface, colors),
                None,
                None,
                None,
                Kind::Unspecified,
            ) {
                elements.push(DecorationRenderElement::Icon(icon));
            }
            let (min_ix, min_iy) = icon_pos(buttons.minimize_rect);
            if let Ok(icon) = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                phys_f64(min_ix, min_iy),
                self.icon_cache
                    .get_or_build(WindowIcon::Minimize, IconTint::OnSurface, colors),
                None,
                None,
                None,
                Kind::Unspecified,
            ) {
                elements.push(DecorationRenderElement::Icon(icon));
            }

            // Window-control chrome behind the glyphs. The mockup shows NO
            // resting chrome — just clean grey ─□× glyphs — so at rest we draw
            // nothing here; only the hovered control lifts with a soft neutral
            // wash (frosted when glass_blur is on), close grey not red. The
            // glass+blur pane itself stays on the titlebar (drawn below).
            if let Some(ref prog) = rounded_quad_shader {
                let pill = buttons.pill_rect;
                let psf = ps as f32;
                let pr = (pill.size.h as f32 / 2.0) * psf;
                let docked_right_radius = rphys;
                // Hover rect + per-corner rounding for each control. The cluster
                // is docked to the top-right window corner: minimize rounds its
                // left edge, close follows the window's outer corner radius.
                let hover_target = |h: HoveredButton| match h {
                    HoveredButton::Close => {
                        (buttons.close_rect, (0.0, docked_right_radius, 0.0, 0.0))
                    }
                    HoveredButton::Maximize => (buttons.maximize_rect, (0.0, 0.0, 0.0, 0.0)),
                    HoveredButton::Minimize => (buttons.minimize_rect, (pr, 0.0, 0.0, 0.0)),
                };
                if theme.glass {
                    let hover_tone = colors.text;
                    let hover_a = if theme.glass_blur {
                        (theme.glass_button_alpha * glass_buttons::HOVER_FACTOR)
                            .min(glass_buttons::HOVER_CAP)
                    } else {
                        (theme.glass_button_alpha + 0.25).min(0.95)
                    };
                    if let Some(h) = hovered {
                        let (rect, radii) = hover_target(h);
                        let [zr, zg, zb, _] = hover_tone.as_f32_array();
                        elements.push(DecorationRenderElement::PixelShader(rounded_quad_element(
                            prog,
                            rect,
                            [zr, zg, zb],
                            radii,
                            0.0,
                            hover_a,
                            psf,
                        )));
                        if theme.glass_blur {
                            let button_tint = (theme.glass_button_alpha
                                * glass_buttons::TINT_FACTOR)
                                .min(glass_buttons::TINT_CAP);
                            elements.push(DecorationRenderElement::Glass(GlassTitlebarInfo {
                                rect,
                                radius: [radii.0, radii.1, radii.2, radii.3],
                                tint: [zr, zg, zb],
                                tint_amount: button_tint,
                                blur: theme.glass_blur_radius,
                                // Opaque frosted base; the colour veil strength is
                                // carried by tint_amount, not the pane opacity.
                                fill_alpha: 1.0,
                            }));
                        }
                    }
                } else {
                    // Tint-only fallback (no blur): same minimal grey controls —
                    // a soft neutral highlight on the hovered zone only, no
                    // resting pill or dividers; close kept grey like the mockup.
                    if let Some(h) = hovered {
                        let (rect, radii) = hover_target(h);
                        let alpha = if matches!(h, HoveredButton::Close) {
                            0.30f32
                        } else {
                            0.22f32
                        };
                        let [cr, cg, cb, _] = colors.text.as_f32_array();
                        elements.push(DecorationRenderElement::PixelShader(rounded_quad_element(
                            prog,
                            rect,
                            [cr, cg, cb],
                            radii,
                            0.0,
                            alpha,
                            psf,
                        )));
                    }
                }
            }

            if deco.is_focused {
                elements.push(DecorationRenderElement::Solid(
                    SolidColorRenderElement::from_buffer(
                        &deco.buffers.title_separator,
                        phys(x, y + TITLE_BAR_HEIGHT + bw - 2),
                        scale,
                        1.0,
                        Kind::Unspecified,
                    ),
                ));
            }

            let titlebar_col = theme.glass_tint_color.unwrap_or(if deco.is_focused {
                colors.surface
            } else {
                colors.surface_alt
            });
            let [r, g, b, _] = titlebar_col.as_f32_array();
            if theme.glass && theme.glass_blur {
                // Textured liquid-glass: emit a placeholder; the backend
                // builds the real element after rendering the scene texture.
                // The titlebar reads the SAME central treatment as panel/
                // launcher/popup — identical tint, blur and fill — so a window
                // frame can never drift onto its own settings again. Only the
                // top-rounded radius is titlebar-specific (layout).
                let treatment = theme.surface_treatment(ThemeSurface::Modal);
                elements.push(DecorationRenderElement::Glass(GlassTitlebarInfo {
                    rect: frame_metrics.titlebar_rect,
                    radius: [rphys, rphys, 0.0, 0.0],
                    tint: [r, g, b],
                    tint_amount: treatment.tint_amount,
                    blur: treatment.blur_radius,
                    fill_alpha: treatment.fill_alpha as f32 / 255.0,
                }));
            } else if theme.glass && glass_shader.is_some() {
                // Tint-only glass fallback (no blur).
                let prog = glass_shader.as_ref().unwrap();
                let specular = if deco.is_focused {
                    theme.glass_specular
                } else {
                    theme.glass_specular * 0.5
                };
                elements.push(DecorationRenderElement::PixelShader(
                    glass_titlebar_element(
                        prog,
                        frame_metrics.titlebar_rect,
                        [r, g, b],
                        (rphys, rphys, 0.0, 0.0),
                        theme.glass_alpha,
                        specular,
                        ps as f32,
                    ),
                ));
            } else if rounded {
                if let Some(ref prog) = rounded_quad_shader {
                    elements.push(DecorationRenderElement::PixelShader(rounded_quad_element(
                        prog,
                        frame_metrics.titlebar_rect,
                        [r, g, b],
                        (rphys, rphys, 0.0, 0.0),
                        0.0,
                        1.0,
                        ps as f32,
                    )));
                }
            } else {
                elements.push(DecorationRenderElement::Solid(
                    SolidColorRenderElement::from_buffer(
                        &deco.buffers.titlebar,
                        phys(x, y),
                        scale,
                        1.0,
                        Kind::Unspecified,
                    ),
                ));
            }
        }

        if bw > 0 && !theme.glass {
            if rounded {
                // One rounded outline ring around the whole frame replaces the
                // four straight border strips, so the outer corners round
                // cleanly with no square nubs. Drawn below the titlebar fill,
                // so the top edge stays covered by the titlebar (matches the
                // square look: no border line across the top).
                let [r, g, b, _] = colors.border.as_f32_array();
                if let Some(ref prog) = rounded_quad_shader {
                    elements.push(DecorationRenderElement::PixelShader(rounded_quad_element(
                        prog,
                        frame_metrics.frame_rect,
                        [r, g, b],
                        (rphys, rphys, rphys, rphys),
                        bw as f32 * ps as f32,
                        1.0,
                        ps as f32,
                    )));
                }
            } else {
                elements.push(DecorationRenderElement::Solid(
                    SolidColorRenderElement::from_buffer(
                        &deco.buffers.border_top,
                        phys(x, y),
                        scale,
                        1.0,
                        Kind::Unspecified,
                    ),
                ));
                elements.push(DecorationRenderElement::Solid(
                    SolidColorRenderElement::from_buffer(
                        &deco.buffers.border_left,
                        phys(x, y + title_h),
                        scale,
                        1.0,
                        Kind::Unspecified,
                    ),
                ));
                elements.push(DecorationRenderElement::Solid(
                    SolidColorRenderElement::from_buffer(
                        &deco.buffers.border_right,
                        phys(x + bw + cw, y + title_h),
                        scale,
                        1.0,
                        Kind::Unspecified,
                    ),
                ));
                elements.push(DecorationRenderElement::Solid(
                    SolidColorRenderElement::from_buffer(
                        &deco.buffers.border_bottom,
                        phys(x, y + title_h + bw + ch),
                        scale,
                        1.0,
                        Kind::Unspecified,
                    ),
                ));
            }
        }

        // Analytic soft drop shadow via the rounded-box SDF pixel shader.
        // Replaces the old 9-slice bitmap (seams/notches at the corners).
        if theme.shadow && bw > 0 {
            if let Some(ref prog) = shadow_shader {
                let spread =
                    effective_shadow_radius(theme.shadow_radius as i32, deco.is_focused).max(1);
                let oy = theme.shadow_offset_y;
                let shadow_alpha = effective_shadow_alpha(theme.shadow_alpha, deco.is_focused);

                let fox = frame_metrics.frame_origin.x;
                let foy = frame_metrics.frame_origin.y;
                let fw = frame_metrics.frame_size.w;
                let fh = frame_metrics.frame_size.h;

                // Inflate enough for the blur spread plus the drop offset.
                let margin = spread + oy.abs();
                let area = Rectangle::<i32, Logical>::new(
                    Point::from((fox - margin, foy - margin)),
                    Size::from((fw + 2 * margin, fh + 2 * margin)),
                );

                // Uniforms in physical pixels — v_coords * size is physical.
                let psf = ps as f32;
                // Shadow shape centre = window centre dropped by `oy`.
                let cx = (margin as f32 + fw as f32 / 2.0) * psf;
                let cy = (margin as f32 + fh as f32 / 2.0 + oy as f32) * psf;
                let hx = (fw as f32 / 2.0) * psf;
                let hy = (fh as f32 / 2.0) * psf;
                let blur = spread as f32 * psf;
                // Match the shadow's rounding to the window corners so the soft
                // edge hugs the rounded frame instead of a square silhouette.
                let radius = if rounded { rphys } else { 0.0f32 };

                let uniforms = vec![
                    Uniform::new("u_frame_center", (cx, cy)),
                    Uniform::new("u_frame_half", (hx, hy)),
                    Uniform::new("u_radius", radius),
                    Uniform::new("u_blur", blur),
                    Uniform::new("u_offset_y", oy as f32 * psf),
                    Uniform::new("u_color", (0.0f32, 0.0, 0.0, 1.0)),
                ];

                let element = PixelShaderElement::new(
                    prog.clone(),
                    area,
                    None,
                    shadow_alpha,
                    uniforms,
                    Kind::Unspecified,
                );
                elements.push(DecorationRenderElement::DropShadow(element));
            }
        }

        elements
    }
}

impl From<SolidColorRenderElement> for DecorationRenderElement {
    fn from(value: SolidColorRenderElement) -> Self {
        Self::Solid(value)
    }
}
