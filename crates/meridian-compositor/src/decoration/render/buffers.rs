use meridian_config::{Decorations, ThemeColors};

use super::super::model::{opaque, HoveredButton, WindowDecoration};

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

pub(super) fn effective_shadow_radius_top(theme_radius_top: i32, focused: bool) -> i32 {
    if focused {
        theme_radius_top
    } else {
        theme_radius_top / 2
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn update_buffers(
    deco: &mut WindowDecoration,
    _theme: &Decorations,
    colors: &ThemeColors,
    show_title: bool,
    bw: i32,
    total_w: i32,
    ch: i32,
    title_h: i32,
    cw: i32,
) {
    let transparent = [0.0f32; 4];

    if show_title {
        deco.buffers.titlebar.update(
            (total_w.max(1), (title_h + bw).max(1)),
            opaque(colors.surface),
        );
        if deco.is_focused {
            deco.buffers
                .title_separator
                .update((total_w.max(1), 2), opaque(colors.accent));
        } else {
            deco.buffers.title_separator.update((1, 1), transparent);
        }
    } else {
        deco.buffers.titlebar.update((1, 1), transparent);
        deco.buffers.title_separator.update((1, 1), transparent);
    }

    if bw > 0 {
        let border_f32 = opaque(colors.border);
        deco.buffers
            .border_top
            .update((total_w.max(1), bw), border_f32);
        deco.buffers
            .border_left
            .update((bw, (ch + bw).max(1)), border_f32);
        deco.buffers
            .border_right
            .update((bw, (ch + bw).max(1)), border_f32);
        deco.buffers
            .border_bottom
            .update((total_w.max(1), bw), border_f32);
    } else {
        deco.buffers.border_top.update((1, 1), transparent);
        deco.buffers.border_left.update((1, 1), transparent);
        deco.buffers.border_right.update((1, 1), transparent);
        deco.buffers.border_bottom.update((1, 1), transparent);
    }

    let close_bg = if deco.hovered_button() == Some(HoveredButton::Close) {
        opaque(colors.error)
    } else {
        transparent
    };
    let max_bg = if deco.hovered_button() == Some(HoveredButton::Maximize) {
        opaque(colors.surface)
    } else {
        transparent
    };
    let min_bg = if deco.hovered_button() == Some(HoveredButton::Minimize) {
        opaque(colors.surface)
    } else {
        transparent
    };
    deco.buffers.close_bg.update(
        (super::super::BUTTON_WIDTH, super::super::BUTTON_HEIGHT),
        close_bg,
    );
    deco.buffers.maximize_bg.update(
        (super::super::BUTTON_WIDTH, super::super::BUTTON_HEIGHT),
        max_bg,
    );
    deco.buffers.minimize_bg.update(
        (super::super::BUTTON_WIDTH, super::super::BUTTON_HEIGHT),
        min_bg,
    );

    deco.last_content_size = (cw, ch);
    deco.last_bw = bw;
    deco.dirty = false;
}

#[cfg(test)]
mod tests {
    use super::{effective_shadow_alpha, effective_shadow_radius, effective_shadow_radius_top};

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
        assert_eq!(effective_shadow_radius(40, true), 40);
        assert_eq!(effective_shadow_radius(40, false), 20);
    }

    #[test]
    fn effective_shadow_radius_top_uses_theme_for_focused() {
        assert_eq!(effective_shadow_radius_top(12, true), 12);
    }

    #[test]
    fn effective_shadow_radius_top_halves_when_unfocused() {
        assert_eq!(effective_shadow_radius_top(12, false), 6);
    }
}
