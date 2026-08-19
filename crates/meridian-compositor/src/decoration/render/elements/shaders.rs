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
#[allow(clippy::too_many_arguments)]
fn rounded_quad_element(
    prog: &smithay::backend::renderer::gles::GlesPixelProgram,
    area: Rectangle<i32, Logical>,
    color: [f32; 3],
    radius_phys: (f32, f32, f32, f32),
    thickness_phys: f32,
    alpha: f32,
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
    PixelShaderElement::new(prog.clone(), area, None, alpha, uniforms, Kind::Unspecified)
}

/// Liquid-glass titlebar shader (GLSL ES 100). A translucent, tinted pane:
/// rounded top corners, a soft vertical sheen (brighter along the top, a touch
/// darker at the bottom), and a single calligraphic specular hairline just
/// inside the top edge. No background blur yet — that lands in a later phase;
/// this gives the glass *structure* and is fully theme-tunable. Output is
/// premultiplied to match the renderer's SRC_OVER blend, like the other
/// decoration shaders.
const GLASS_SHADER_SRC: &str = r#"
precision highp float;
uniform vec2 size;
uniform float alpha;
uniform vec3 u_color;
uniform vec4 u_radius;
uniform float u_base_alpha;
uniform float u_specular;
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
    float cov = rounding_alpha(loc, size, u_radius);
    float ny = v_coords.y; // 0 = top edge, 1 = bottom edge

    // Vertical sheen: a curved pane catches a little more light along its top.
    float sheen = mix(1.16, 0.90, ny);
    vec3 col = u_color * sheen;

    // Specular hairline just inside the top edge — the calligraphic light line.
    float top_px = loc.y;
    float fwhm = 1.4 * u_scale;
    float center_px = 1.6 * u_scale;
    float d = (top_px - center_px) / fwhm;
    float line = exp(-d * d);
    col += vec3(line) * u_specular;

    // Base translucency, a hair denser near the top to seat the highlight.
    float a = u_base_alpha * mix(1.05, 0.94, ny);
    a = clamp(a + line * u_specular * 0.45, 0.0, 1.0);

    float out_a = cov * a * alpha;
    gl_FragColor = vec4(col * out_a, out_a);
}
"#;

fn glass_uniform_names() -> [UniformName<'static>; 5] {
    [
        UniformName::new("u_color", UniformType::_3f),
        UniformName::new("u_radius", UniformType::_4f),
        UniformName::new("u_base_alpha", UniformType::_1f),
        UniformName::new("u_specular", UniformType::_1f),
        UniformName::new("u_scale", UniformType::_1f),
    ]
}

/// Build a liquid-glass titlebar `PixelShaderElement` over `area` (logical).
/// `radius` values are physical pixels; normally only the top corners round.
#[allow(clippy::too_many_arguments)]
fn glass_titlebar_element(
    prog: &smithay::backend::renderer::gles::GlesPixelProgram,
    area: Rectangle<i32, Logical>,
    color: [f32; 3],
    radius_phys: (f32, f32, f32, f32),
    base_alpha: f32,
    specular: f32,
    scale: f32,
) -> PixelShaderElement {
    let uniforms = vec![
        Uniform::new("u_color", color),
        Uniform::new(
            "u_radius",
            [radius_phys.0, radius_phys.1, radius_phys.2, radius_phys.3],
        ),
        Uniform::new("u_base_alpha", base_alpha),
        Uniform::new("u_specular", specular),
        Uniform::new("u_scale", scale),
    ];
    PixelShaderElement::new(prog.clone(), area, None, 1.0, uniforms, Kind::Unspecified)
}
