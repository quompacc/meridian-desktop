use meridian_config::{Decorations, ThemeColors};

use super::super::model::{opaque, WindowDecoration};

/// Unfocused windows keep a softer shadow than focused ones. This is derived
/// from the theme's (focused) `shadow_alpha` rather than a fixed literal, so a
/// theme that dials its shadow down/up scales the inactive shadow with it —
/// the shadow opacity has a single source (GUI_CENTRALIZATION_PLAN §6 phase 1).
const INACTIVE_SHADOW_FACTOR: f32 = 0.85;

pub(super) fn effective_shadow_alpha(theme_alpha: f32, focused: bool) -> f32 {
    if focused {
        theme_alpha
    } else {
        theme_alpha * INACTIVE_SHADOW_FACTOR
    }
}

pub(super) fn effective_shadow_radius(theme_radius: i32, focused: bool) -> i32 {
    if focused {
        theme_radius
    } else {
        theme_radius / 2
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
        // Focused windows get the brighter surface titlebar plus a quiet
        // neutral hairline; unfocused windows dim to surface_alt.
        let titlebar_col = if deco.is_focused {
            colors.surface
        } else {
            colors.surface_alt
        };
        deco.buffers
            .titlebar
            .update((cw.max(1), (title_h + bw).max(1)), opaque(titlebar_col));
        if deco.is_focused {
            deco.buffers.title_separator.update(
                (cw.max(1), super::super::TITLE_SEPARATOR_HEIGHT),
                opaque(colors.border),
            );
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
        // Inactive shadow is now a fraction of the theme's focused alpha.
        assert_eq!(
            effective_shadow_alpha(0.5, false),
            0.5 * super::INACTIVE_SHADOW_FACTOR
        );
        // ...and always lighter than the focused shadow.
        assert!(effective_shadow_alpha(0.5, false) < effective_shadow_alpha(0.5, true));
    }

    #[test]
    fn effective_shadow_radius_halves_when_unfocused() {
        assert_eq!(effective_shadow_radius(40, true), 40);
        assert_eq!(effective_shadow_radius(40, false), 20);
    }
}
