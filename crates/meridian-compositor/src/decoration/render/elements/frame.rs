use super::{
    super::{
        icons::{IconTint, WindowIcon},
        model::HoveredButton,
        DecorationManager, DecorationRenderElement, BUTTON_ICON_PX, TITLE_BAR_HEIGHT,
    },
    buffers::{effective_shadow_alpha, effective_shadow_radius, update_buffers},
    geometry::{SsdChromeMetrics, SsdFrameMetrics},
};

/// Rounded-box soft drop-shadow pixel shader (GLSL ES 100). Computes the
/// signed distance to the (optionally rounded) window rect and fades the
/// shadow smoothly across `u_blur` -- a seamless analytic shadow, no 9-slice
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
    // Discard inside the actual window. Opaque clients would cover it anyway;
    // translucent clients must not be tinted grey by their own shadow.
    vec2 win_center = u_frame_center - vec2(0.0, u_offset_y);
    float d_win = rounded_box_sdf(px - win_center, u_frame_half, u_radius);
    if (d_win < 0.0) {
        discard;
    }
    // Coverage from the drop-shifted shape keeps the shadow continuous below
    // the window instead of introducing a transparent offset gap.
    float d = rounded_box_sdf(px - u_frame_center, u_frame_half, u_radius);
    float cov = 1.0 - smoothstep(-u_blur, u_blur, d);
    gl_FragColor = u_color * (cov * alpha);
}
"#;
