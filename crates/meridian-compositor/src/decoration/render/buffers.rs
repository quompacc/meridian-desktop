use meridian_config::{Decorations, ThemeColors};

use super::super::{
    model::{
        opaque, HoveredButton, WindowDecoration, SHADOW_LAYER_COUNT, STRIP_BOTTOM, STRIP_LEFT,
        STRIP_RIGHT, STRIP_TOP,
    },
    BUTTON_HEIGHT, BUTTON_WIDTH, TITLE_BAR_HEIGHT,
};
use super::geometry::{SsdChromeMetrics, SsdFrameMetrics};

const INACTIVE_SHADOW_ALPHA: f32 = 0.3;

#[derive(Debug, Clone, Copy)]
pub(super) struct ShadowLayer {
    pub(super) radius_scale: f32,
    pub(super) alpha_scale: f32,
}

pub(super) const SHADOW_LAYERS: [ShadowLayer; SHADOW_LAYER_COUNT] = [
    ShadowLayer {
        radius_scale: 0.20,
        alpha_scale: 0.85,
    },
    ShadowLayer {
        radius_scale: 0.40,
        alpha_scale: 0.55,
    },
    ShadowLayer {
        radius_scale: 0.60,
        alpha_scale: 0.35,
    },
    ShadowLayer {
        radius_scale: 0.80,
        alpha_scale: 0.20,
    },
    ShadowLayer {
        radius_scale: 1.00,
        alpha_scale: 0.10,
    },
];

pub(super) fn effective_shadow_alpha(theme: &Decorations, focused: bool) -> f32 {
    if focused {
        theme.shadow_alpha
    } else {
        INACTIVE_SHADOW_ALPHA
    }
}

pub(super) fn effective_shadow_radius(theme: &Decorations, focused: bool) -> i32 {
    if focused {
        theme.shadow_radius as i32
    } else {
        (theme.shadow_radius as i32) / 2
    }
}

pub(super) fn layer_radius(theme: &Decorations, focused: bool, scale: f32) -> i32 {
    (effective_shadow_radius(theme, focused) as f32 * scale)
        .round()
        .max(0.0) as i32
}

pub(super) fn layer_alpha(theme: &Decorations, focused: bool, scale: f32) -> f32 {
    (effective_shadow_alpha(theme, focused) * scale).clamp(0.0, 1.0)
}

// Keep explicit decoration/render parameters to avoid risky context bundling in render code.
#[allow(clippy::too_many_arguments)]
pub(super) fn update_buffers(
    deco: &mut WindowDecoration,
    theme: &Decorations,
    colors: &ThemeColors,
    show_title: bool,
    bw: i32,
    total_w: i32,
    ch: i32,
    title_h: i32,
    cw: i32,
) {
    let border_f32 = opaque(if deco.is_focused {
        colors.accent
    } else {
        colors.border
    });
    let title_f32 = opaque(if deco.is_focused {
        colors.accent
    } else {
        colors.border
    });
    let transparent = [0.0f32; 4];
    let close_f32 = if deco.hovered_button() == Some(HoveredButton::Close) {
        opaque(colors.error)
    } else {
        transparent
    };
    let maximize_f32 = if deco.hovered_button() == Some(HoveredButton::Maximize) {
        opaque(colors.surface)
    } else {
        transparent
    };
    let minimize_f32 = if deco.hovered_button() == Some(HoveredButton::Minimize) {
        opaque(colors.surface)
    } else {
        transparent
    };

    if show_title {
        deco.buffers
            .titlebar
            .update((total_w, TITLE_BAR_HEIGHT + bw), title_f32);
        deco.buffers
            .close_bg
            .update((BUTTON_WIDTH, BUTTON_HEIGHT), close_f32);
        deco.buffers
            .maximize_bg
            .update((BUTTON_WIDTH, BUTTON_HEIGHT), maximize_f32);
        deco.buffers
            .minimize_bg
            .update((BUTTON_WIDTH, BUTTON_HEIGHT), minimize_f32);
    }
    if bw > 0 {
        if !show_title {
            deco.buffers
                .border_top
                .update((total_w.max(1), bw), border_f32);
        }
        deco.buffers.border_left.update((bw, ch.max(1)), border_f32);
        deco.buffers
            .border_right
            .update((bw, ch.max(1)), border_f32);
        deco.buffers
            .border_bottom
            .update((total_w.max(1), bw), border_f32);
    }
    if theme.shadow && bw > 0 {
        let frame = SsdFrameMetrics::from_frame_origin((0, 0).into(), (cw, ch).into(), bw, title_h);
        let chrome = SsdChromeMetrics::new(frame);
        for (layer_idx, layer) in SHADOW_LAYERS.iter().enumerate() {
            let sr = layer_radius(theme, deco.is_focused, layer.radius_scale);
            let color = [
                0.0f32,
                0.0,
                0.0,
                layer_alpha(theme, deco.is_focused, layer.alpha_scale),
            ];
            if let Some(donut) = chrome.shadow_donut_metrics(sr) {
                let strips = &mut deco.buffers.shadow_strips[layer_idx];
                strips[STRIP_TOP].update((donut.top.size.w, donut.top.size.h), color);
                strips[STRIP_BOTTOM].update((donut.bottom.size.w, donut.bottom.size.h), color);
                strips[STRIP_LEFT].update((donut.left.size.w, donut.left.size.h), color);
                strips[STRIP_RIGHT].update((donut.right.size.w, donut.right.size.h), color);
            }
        }
    }

    deco.last_content_size = (cw, ch);
    deco.last_bw = bw;
    deco.dirty = false;
}

#[cfg(test)]
mod tests {
    use meridian_config::Decorations;

    use super::{
        effective_shadow_alpha, effective_shadow_radius, layer_alpha, layer_radius, SHADOW_LAYERS,
    };

    #[test]
    fn effective_shadow_alpha_uses_theme_for_focused_window() {
        let theme = Decorations {
            shadow_alpha: 0.5,
            ..Default::default()
        };
        assert_eq!(effective_shadow_alpha(&theme, true), 0.5);
    }

    #[test]
    fn effective_shadow_alpha_drops_to_inactive_when_unfocused() {
        let theme = Decorations {
            shadow_alpha: 0.5,
            ..Default::default()
        };
        assert_eq!(effective_shadow_alpha(&theme, false), 0.3);
    }

    #[test]
    fn effective_shadow_radius_halves_when_unfocused() {
        let theme = Decorations {
            shadow_radius: 24,
            ..Default::default()
        };
        assert_eq!(effective_shadow_radius(&theme, true), 24);
        assert_eq!(effective_shadow_radius(&theme, false), 12);
    }

    #[test]
    fn layer_radius_zero_when_unfocused_inner_scale_round_down() {
        let theme = Decorations {
            shadow_radius: 1,
            ..Default::default()
        };
        assert_eq!(
            layer_radius(&theme, false, SHADOW_LAYERS[0].radius_scale),
            0
        );
    }

    #[test]
    fn layer_alpha_clamps_to_one_when_overscaled() {
        let theme = Decorations {
            shadow_alpha: 0.8,
            ..Default::default()
        };
        assert_eq!(layer_alpha(&theme, true, 2.0), 1.0);
    }

    #[test]
    fn shadow_layers_radius_scales_strictly_ascending() {
        let theme = Decorations {
            shadow_radius: 24,
            ..Default::default()
        };
        let inner = layer_radius(&theme, true, SHADOW_LAYERS[0].radius_scale);
        let mid = layer_radius(&theme, true, SHADOW_LAYERS[1].radius_scale);
        let outer = layer_radius(&theme, true, SHADOW_LAYERS[4].radius_scale);
        assert!(inner < mid);
        assert!(mid < outer);
    }

    #[test]
    fn shadow_layers_total_alpha_at_window_edge_below_full_opacity() {
        let theme = Decorations {
            shadow_alpha: 0.5,
            ..Default::default()
        };
        let composed = 1.0f32
            - SHADOW_LAYERS
                .iter()
                .map(|layer| layer_alpha(&theme, true, layer.alpha_scale))
                .fold(1.0f32, |acc, alpha| acc * (1.0 - alpha));
        assert!(composed < 1.0);
    }
}
