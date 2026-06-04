// Liquid-glass titlebar fragment shader (GLSL ES 100).
//
// Samples the *already blurred* scene behind the titlebar (`tex` is the
// full-output scene texture after the separable Gaussian pre-pass; `u_src` is
// the titlebar's slice of it in normalised UV), pulls it toward the theme tint
// and adds a soft vertical sheen, then rounds the top corners. Output is
// premultiplied to match the renderer's SRC_OVER blend.

#version 100

//_DEFINES_

#if defined(EXTERNAL)
#extension GL_OES_EGL_image_external : require
#endif

precision highp float;
#if defined(EXTERNAL)
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif

uniform float alpha;
varying vec2 v_coords;

uniform vec2 geo_size;       // titlebar size in physical px
uniform vec4 corner_radius;  // (top-left, top-right, bottom-right, bottom-left) px
uniform vec3 u_tint;
uniform float u_tint_amount;
uniform vec4 u_src;          // titlebar slice in tex: (u0, v0, uw, vh)

float rounding_alpha(vec2 coords, vec2 size) {
    vec2 center;
    float radius;
    if (coords.x < corner_radius.x && coords.y < corner_radius.x) {
        radius = corner_radius.x; center = vec2(radius, radius);
    } else if (size.x - corner_radius.y < coords.x && coords.y < corner_radius.y) {
        radius = corner_radius.y; center = vec2(size.x - radius, radius);
    } else if (size.x - corner_radius.z < coords.x && size.y - corner_radius.z < coords.y) {
        radius = corner_radius.z; center = vec2(size.x - radius, size.y - radius);
    } else if (coords.x < corner_radius.w && size.y - corner_radius.w < coords.y) {
        radius = corner_radius.w; center = vec2(radius, size.y - radius);
    } else {
        return 1.0;
    }
    float dist = distance(coords, center);
    return 1.0 - smoothstep(radius - 0.5, radius + 0.5, dist);
}

void main() {
    vec2 local = (v_coords - u_src.xy) / u_src.zw;
    vec2 px = local * geo_size;

    // Background is already blurred by the separable pre-pass; sample directly.
    vec3 bg = texture2D(tex, v_coords).rgb;

    // Tint veil + a soft vertical sheen (gradient, no bright edge line).
    vec3 col = mix(bg, u_tint, u_tint_amount);
    col *= mix(1.06, 0.96, local.y);

    float cov = rounding_alpha(px, geo_size);
    float a = cov * alpha;
    gl_FragColor = vec4(col * a, a);
}
