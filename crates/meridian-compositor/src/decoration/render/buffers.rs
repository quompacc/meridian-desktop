use meridian_config::{Decorations, ThemeColors};

use super::{
    super::model::{opaque, WindowDecoration},
    geometry::{SsdChromeMetrics, SsdFrameMetrics},
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
    theme: &Decorations,
    colors: &ThemeColors,
    show_title: bool,
    bw: i32,
    total_w: i32,
    ch: i32,
    title_h: i32,
    cw: i32,
) {
    let frame_f32 = opaque(if deco.is_focused {
        colors.accent
    } else {
        colors.border
    });
    let transparent = [0.0f32; 4];

    if bw > 0 {
        let slices = SsdChromeMetrics::new(SsdFrameMetrics::from_frame_origin(
            (0, 0).into(),
            (cw, ch).into(),
            bw,
            title_h,
        ))
        .frame_slices(theme.corner_radius as i32);

        if let Some(s) = slices {
            deco.buffers
                .top_strip
                .update((s.top_strip.size.w, s.top_strip.size.h), frame_f32);
            deco.buffers
                .bottom_strip
                .update((s.bottom_strip.size.w, s.bottom_strip.size.h), frame_f32);
            deco.buffers
                .left_strip
                .update((s.left_strip.size.w, s.left_strip.size.h), frame_f32);
            deco.buffers
                .right_strip
                .update((s.right_strip.size.w, s.right_strip.size.h), frame_f32);
            deco.buffers.middle_belt.update(
                (s.middle_belt.size.w.max(1), s.middle_belt.size.h.max(1)),
                frame_f32,
            );
        } else {
            let top_h = if show_title { title_h + bw } else { bw };
            deco.buffers
                .top_strip
                .update((total_w.max(1), top_h.max(1)), frame_f32);
            deco.buffers
                .bottom_strip
                .update((total_w.max(1), bw.max(1)), frame_f32);
            deco.buffers
                .left_strip
                .update((bw.max(1), ch.max(1)), frame_f32);
            deco.buffers
                .right_strip
                .update((bw.max(1), ch.max(1)), frame_f32);
            deco.buffers.middle_belt.update((1, 1), transparent);
        }
    } else {
        deco.buffers.top_strip.update((1, 1), transparent);
        deco.buffers.bottom_strip.update((1, 1), transparent);
        deco.buffers.left_strip.update((1, 1), transparent);
        deco.buffers.right_strip.update((1, 1), transparent);
        deco.buffers.middle_belt.update((1, 1), transparent);
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
