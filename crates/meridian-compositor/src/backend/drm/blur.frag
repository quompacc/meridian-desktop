// Separable 1-D Gaussian blur pass (GLSL ES 100). Run twice — once with a
// horizontal `u_step`, once vertical — to build a smooth 2-D blur of the scene
// texture behind the glass. Proper Gaussian weights (no discrete ghost copies).

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

uniform vec2 u_step;  // per-unit UV offset (direction * texel * spread)

void main() {
    vec4 c = texture2D(tex, v_coords) * 0.227027;
    c += texture2D(tex, v_coords + u_step * 1.0) * 0.194595;
    c += texture2D(tex, v_coords - u_step * 1.0) * 0.194595;
    c += texture2D(tex, v_coords + u_step * 2.0) * 0.121622;
    c += texture2D(tex, v_coords - u_step * 2.0) * 0.121622;
    c += texture2D(tex, v_coords + u_step * 3.0) * 0.070270;
    c += texture2D(tex, v_coords - u_step * 3.0) * 0.070270;
    c += texture2D(tex, v_coords + u_step * 4.0) * 0.016216;
    c += texture2D(tex, v_coords - u_step * 4.0) * 0.016216;
    gl_FragColor = vec4(c.rgb, 1.0) * alpha;
}
