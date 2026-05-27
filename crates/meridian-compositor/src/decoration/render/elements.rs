use smallvec::SmallVec;

use meridian_config::{Decorations, ThemeColors};
use smithay::{
    backend::renderer::{
        element::{memory::MemoryRenderBufferRenderElement, solid::SolidColorRenderElement, Kind},
        gles::{element::PixelShaderElement, GlesRenderer, Uniform, UniformName, UniformType},
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Physical, Point, Rectangle, Scale, Size},
};

use super::{
    super::{
        icons::{IconTint, WindowIcon},
        model::HoveredButton,
        DecorationManager, DecorationRenderElement, BUTTON_HEIGHT, BUTTON_ICON_PX, BUTTON_WIDTH,
        TITLE_BAR_HEIGHT,
    },
    buffers::{effective_shadow_alpha, effective_shadow_radius, update_buffers},
    geometry::{SsdChromeMetrics, SsdFrameMetrics},
};


/// Rounded-box soft drop-shadow pixel shader (GLSL ES 100). Computes the
/// signed distance to the (optionally rounded) window rect and fades the
/// shadow smoothly across `u_blur` — a seamless analytic shadow, no 9-slice
/// bitmap. Technique after Evan Wallace's rounded-rectangle shadows.
const SHADOW_SHADER_SRC: &str = r#"
precision highp float;
uniform vec2 size;
uniform float alpha;
uniform vec2 u_frame_center;
uniform vec2 u_frame_half;
uniform float u_radius;
uniform float u_blur;
uniform float u_offset_y;
uniform vec4 u_color;
varying vec2 v_coords;

float rounded_box_sdf(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + vec2(r);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2(0.0))) - r;
}

void main() {
    vec2 px = v_coords * size;
    // Discard inside the *actual* window (u_frame_center is the drop-shifted
    // shadow shape, so the window sits u_offset_y above it). Opaque clients
    // would cover it anyway; translucent ones must not be tinted grey.
    vec2 win_center = u_frame_center - vec2(0.0, u_offset_y);
    float d_win = rounded_box_sdf(px - win_center, u_frame_half, u_radius);
    if (d_win < 0.0) {
        discard;
    }
    // Coverage from the drop-shifted shape, so the shadow stays continuous
    // directly below the window (no transparent gap from the offset).
    float d = rounded_box_sdf(px - u_frame_center, u_frame_half, u_radius);
    float cov = 1.0 - smoothstep(-u_blur, u_blur, d);
    gl_FragColor = u_color * (cov * alpha);
}
"#;

fn shadow_uniform_names() -> [UniformName<'static>; 6] {
    [
        UniformName::new("u_frame_center", UniformType::_2f),
        UniformName::new("u_frame_half", UniformType::_2f),
        UniformName::new("u_radius", UniformType::_1f),
        UniformName::new("u_blur", UniformType::_1f),
        UniformName::new("u_offset_y", UniformType::_1f),
        UniformName::new("u_color", UniformType::_4f),
    ]
}

/// Rounded-rectangle fill/outline pixel shader (GLSL ES 100). With
/// `u_thickness == 0` it fills a per-corner-rounded rect (used for the
/// titlebar, rounding only its top corners); with `u_thickness > 0` it draws a
/// rounded outline ring of that thickness (used for the 1px window border,
/// rounding all four outer corners). Per-corner radii in `u_radius` are
/// (top-left, top-right, bottom-right, bottom-left), all in physical pixels.
/// Coverage math after cosmic-comp's rounded_outline.frag (orig. niri).
const ROUNDED_QUAD_SHADER_SRC: &str = r#"
precision highp float;
uniform vec2 size;
uniform float alpha;
uniform vec3 u_color;
uniform float u_thickness;
uniform vec4 u_radius;
uniform float u_scale;
varying vec2 v_coords;

float rounding_alpha(vec2 coords, vec2 sz, vec4 radius) {
    vec2 center;
    float r;
    if (coords.x < radius.x && coords.y < radius.x) {
        r = radius.x; center = vec2(r, r);
    } else if (sz.x - radius.y < coords.x && coords.y < radius.y) {
        r = radius.y; center = vec2(sz.x - r, r);
    } else if (sz.x - radius.z < coords.x && sz.y - radius.z < coords.y) {
        r = radius.z; center = vec2(sz.x - r, sz.y - r);
    } else if (coords.x < radius.w && sz.y - radius.w < coords.y) {
        r = radius.w; center = vec2(r, sz.y - r);
    } else {
        return 1.0;
    }
    float dist = distance(coords, center);
    float half_px = 0.5 / u_scale;
    return 1.0 - smoothstep(r - half_px, r + half_px, dist);
}

void main() {
    vec2 loc = v_coords * size;
    float outer = rounding_alpha(loc, size, u_radius);
    float inner = 1.0;
    if (u_thickness > 0.0) {
        vec2 iloc = loc - vec2(u_thickness);
        vec2 isize = size - vec2(u_thickness * 2.0);
        if (0.0 <= iloc.x && iloc.x <= isize.x && 0.0 <= iloc.y && iloc.y <= isize.y) {
            vec4 iradius = u_radius - vec4(u_thickness);
            inner = 1.0 - rounding_alpha(iloc, isize, iradius);
        }
    }
    float cov = outer * inner;
    gl_FragColor = vec4(u_color, 1.0) * (cov * alpha);
}
"#;

fn rounded_quad_uniform_names() -> [UniformName<'static>; 4] {
    [
        UniformName::new("u_color", UniformType::_3f),
        UniformName::new("u_thickness", UniformType::_1f),
        UniformName::new("u_radius", UniformType::_4f),
        UniformName::new("u_scale", UniformType::_1f),
    ]
}

/// Build a rounded fill/outline `PixelShaderElement` over `area` (logical).
/// `radius`/`thickness` are physical pixels; `thickness == 0.0` fills.
fn rounded_quad_element(
    prog: &smithay::backend::renderer::gles::GlesPixelProgram,
    area: Rectangle<i32, Logical>,
    color: [f32; 3],
    radius_phys: (f32, f32, f32, f32),
    thickness_phys: f32,
    scale: f32,
) -> PixelShaderElement {
    let uniforms = vec![
        Uniform::new("u_color", color),
        Uniform::new("u_thickness", thickness_phys),
        Uniform::new(
            "u_radius",
            [radius_phys.0, radius_phys.1, radius_phys.2, radius_phys.3],
        ),
        Uniform::new("u_scale", scale),
    ];
    PixelShaderElement::new(prog.clone(), area, None, 1.0, uniforms, Kind::Unspecified)
}

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
        let shadow_shader = self.shadow_shader.clone();
        let rounded_quad_shader = self.rounded_quad_shader.clone();
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

            let close_tint = if deco.hovered_button() == Some(HoveredButton::Close) {
                IconTint::OnAccentRed
            } else {
                IconTint::OnSurface
            };
            let max_kind = if deco.is_maximized {
                WindowIcon::Restore
            } else {
                WindowIcon::Maximize
            };

            let icon_pos = |rect: smithay::utils::Rectangle<i32, Logical>| {
                let icon_x = rect.loc.x + (BUTTON_WIDTH - BUTTON_ICON_PX as i32) / 2;
                let icon_y = rect.loc.y + (BUTTON_HEIGHT - BUTTON_ICON_PX as i32) / 2;
                (icon_x, icon_y)
            };

            let (close_icon_x, close_icon_y) = icon_pos(buttons.close_rect);
            if let Ok(icon) = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                phys_f64(close_icon_x, close_icon_y),
                self.icon_cache
                    .get_or_build(WindowIcon::Close, close_tint, colors),
                None,
                None,
                None,
                Kind::Unspecified,
            ) {
                elements.push(DecorationRenderElement::Icon(icon));
            }
            elements.push(DecorationRenderElement::Solid(
                SolidColorRenderElement::from_buffer(
                    &deco.buffers.close_bg,
                    phys(buttons.close_rect.loc.x, buttons.close_rect.loc.y),
                    scale,
                    1.0,
                    Kind::Unspecified,
                ),
            ));

            let (maximize_icon_x, maximize_icon_y) = icon_pos(buttons.maximize_rect);
            if let Ok(icon) = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                phys_f64(maximize_icon_x, maximize_icon_y),
                self.icon_cache
                    .get_or_build(max_kind, IconTint::OnSurface, colors),
                None,
                None,
                None,
                Kind::Unspecified,
            ) {
                elements.push(DecorationRenderElement::Icon(icon));
            }
            elements.push(DecorationRenderElement::Solid(
                SolidColorRenderElement::from_buffer(
                    &deco.buffers.maximize_bg,
                    phys(buttons.maximize_rect.loc.x, buttons.maximize_rect.loc.y),
                    scale,
                    1.0,
                    Kind::Unspecified,
                ),
            ));

            let (minimize_icon_x, minimize_icon_y) = icon_pos(buttons.minimize_rect);
            if let Ok(icon) = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                phys_f64(minimize_icon_x, minimize_icon_y),
                self.icon_cache
                    .get_or_build(WindowIcon::Minimize, IconTint::OnSurface, colors),
                None,
                None,
                None,
                Kind::Unspecified,
            ) {
                elements.push(DecorationRenderElement::Icon(icon));
            }
            elements.push(DecorationRenderElement::Solid(
                SolidColorRenderElement::from_buffer(
                    &deco.buffers.minimize_bg,
                    phys(buttons.minimize_rect.loc.x, buttons.minimize_rect.loc.y),
                    scale,
                    1.0,
                    Kind::Unspecified,
                ),
            ));

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

            if rounded {
                let titlebar_col = if deco.is_focused {
                    colors.surface
                } else {
                    colors.surface_alt
                };
                let [r, g, b, _] = titlebar_col.as_f32_array();
                if let Some(ref prog) = rounded_quad_shader {
                    elements.push(DecorationRenderElement::PixelShader(rounded_quad_element(
                        prog,
                        frame_metrics.titlebar_rect,
                        [r, g, b],
                        (rphys, rphys, 0.0, 0.0),
                        0.0,
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

        if bw > 0 {
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
                let spread = effective_shadow_radius(theme.shadow_radius as i32, deco.is_focused)
                    .max(1);
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
                elements.push(DecorationRenderElement::PixelShader(element));
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
