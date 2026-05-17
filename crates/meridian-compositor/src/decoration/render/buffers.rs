use meridian_config::{Decorations, ThemeColors};

use super::super::{
    model::{opaque, HoveredButton, WindowDecoration},
    BUTTON_HEIGHT, BUTTON_WIDTH, TITLE_BAR_HEIGHT,
};

const INACTIVE_SHADOW_ALPHA: f32 = 0.3;

pub(super) fn effective_shadow_alpha(theme_alpha: f32, focused: bool) -> f32 {
    if focused {
        theme_alpha
    } else {
        INACTIVE_SHADOW_ALPHA
    }
}

pub(super) fn effective_shadow_radius(theme_radius: i32, focused: bool) -> i32 {
    if focused {
        theme_radius
    } else {
        theme_radius / 2
    }
}

// Keep explicit decoration/render parameters to avoid risky context bundling in render code.
#[allow(clippy::too_many_arguments)]
pub(super) fn update_buffers(
    deco: &mut WindowDecoration,
    _theme: &Decorations,
    colors: &ThemeColors,
    show_title: bool,
    bw: i32,
    total_w: i32,
    ch: i32,
    _title_h: i32,
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

    deco.last_content_size = (cw, ch);
    deco.last_bw = bw;
    deco.dirty = false;
}

#[cfg(test)]
mod tests {
    use super::{effective_shadow_alpha, effective_shadow_radius};

    #[test]
    fn effective_shadow_alpha_uses_theme_for_focused_window() {
        assert_eq!(effective_shadow_alpha(0.5, true), 0.5);
    }

    #[test]
    fn effective_shadow_alpha_drops_to_inactive_when_unfocused() {
        assert_eq!(effective_shadow_alpha(0.5, false), 0.3);
    }

    #[test]
    fn effective_shadow_radius_halves_when_unfocused() {
        assert_eq!(effective_shadow_radius(24, true), 24);
        assert_eq!(effective_shadow_radius(24, false), 12);
    }
}
