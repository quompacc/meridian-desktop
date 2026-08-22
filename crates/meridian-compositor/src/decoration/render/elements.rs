use smallvec::SmallVec;

use meridian_config::{Decorations, ThemeColors, ThemeSurface};
use meridian_tokens::Interaction;
use smithay::{
    backend::renderer::{
        element::{memory::MemoryRenderBufferRenderElement, solid::SolidColorRenderElement, Kind},
        gles::{element::PixelShaderElement, GlesRenderer, Uniform, UniformName, UniformType},
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Physical, Point, Rectangle, Scale, Size},
};

use crate::backend::drm::glass::GlassTitlebarInfo;

include!("elements/frame.rs");
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
        let cr = deco.corner_radius(theme);
        // Maximized windows meet the output edges and visible panel island;
        // square corners prevent wallpaper pinholes at those hard boundaries.
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

            // Resting controls remain bare glyphs. Hover uses one quiet,
            // inset cushion from the shared Interaction token; avoiding a
            // second per-button glass pane keeps the titlebar calm and makes
            // invalidation a single cached shader element.
            if let Some(ref prog) = rounded_quad_shader {
                let psf = ps as f32;
                let inset = super::super::BUTTON_HOVER_INSET;
                let hover_radius = super::super::BUTTON_HOVER_RADIUS * psf;
                let hover_rect = |rect: Rectangle<i32, Logical>| {
                    Rectangle::new(
                        (rect.loc.x + inset, rect.loc.y + inset).into(),
                        (
                            (rect.size.w - inset * 2).max(1),
                            (rect.size.h - inset * 2).max(1),
                        )
                            .into(),
                    )
                };
                if let Some(h) = hovered {
                    let rect = hover_rect(match h {
                        HoveredButton::Close => buttons.close_rect,
                        HoveredButton::Maximize => buttons.maximize_rect,
                        HoveredButton::Minimize => buttons.minimize_rect,
                    });
                    let tone = if matches!(h, HoveredButton::Close) {
                        colors.error
                    } else {
                        colors.text
                    };
                    let [r, g, b, _] = tone.as_f32_array();
                    let hover_alpha = Interaction::DEFAULT.neutral_hover.as_f32_array()[3];
                    elements.push(DecorationRenderElement::PixelShader(rounded_quad_element(
                        prog,
                        rect,
                        [r, g, b],
                        (hover_radius, hover_radius, hover_radius, hover_radius),
                        0.0,
                        hover_alpha,
                        psf,
                    )));
                }
            }

            if deco.is_focused {
                elements.push(DecorationRenderElement::Solid(
                    SolidColorRenderElement::from_buffer(
                        &deco.buffers.title_separator,
                        phys(
                            x + bw,
                            y + TITLE_BAR_HEIGHT + bw - super::super::TITLE_SEPARATOR_HEIGHT,
                        ),
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
                        phys(x + bw, y),
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
