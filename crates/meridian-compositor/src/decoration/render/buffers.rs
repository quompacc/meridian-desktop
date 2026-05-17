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

    let set_strip = |buffer: &mut smithay::backend::renderer::element::solid::SolidColorBuffer,
                     width: i32,
                     height: i32| {
        if width > 0 && height > 0 {
            buffer.update((width, height), frame_f32);
        } else {
            buffer.update((1, 1), transparent);
        }
    };

    if show_title || bw > 0 {
        if let Some(s) = SsdChromeMetrics::new(SsdFrameMetrics::from_frame_origin(
            (0, 0).into(),
            (cw, ch).into(),
            bw,
            title_h,
        ))
        .frame_slices(theme.corner_radius as i32)
        {
            set_strip(
                &mut deco.buffers.top_strip,
                s.top_strip.size.w,
                s.top_strip.size.h,
            );
            set_strip(
                &mut deco.buffers.middle_belt,
                s.middle_belt.size.w,
                s.middle_belt.size.h,
            );
            set_strip(
                &mut deco.buffers.left_strip,
                s.left_strip.size.w,
                s.left_strip.size.h,
            );
            set_strip(
                &mut deco.buffers.right_strip,
                s.right_strip.size.w,
                s.right_strip.size.h,
            );
            set_strip(
                &mut deco.buffers.bottom_border,
                s.bottom_border.size.w,
                s.bottom_border.size.h,
            );
        } else {
            let top_h = if show_title { title_h + bw } else { bw };
            set_strip(&mut deco.buffers.top_strip, total_w, top_h);
            set_strip(&mut deco.buffers.middle_belt, 0, 0);
            set_strip(&mut deco.buffers.left_strip, bw, ch);
            set_strip(&mut deco.buffers.right_strip, bw, ch);
            set_strip(&mut deco.buffers.bottom_border, total_w, bw);
        }
    } else {
        set_strip(&mut deco.buffers.top_strip, 0, 0);
        set_strip(&mut deco.buffers.middle_belt, 0, 0);
        set_strip(&mut deco.buffers.left_strip, 0, 0);
        set_strip(&mut deco.buffers.right_strip, 0, 0);
        set_strip(&mut deco.buffers.bottom_border, 0, 0);
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
