use meridian_config::{Decorations, ThemeColors};

use super::super::{
    model::{opaque, WindowDecoration, SHADOW_COLOR},
    BUTTON_SIZE, TITLE_BAR_HEIGHT,
};
use super::geometry::{SsdChromeMetrics, SsdFrameMetrics};

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
    let close_f32 = opaque(colors.error);
    let maximize_f32 = opaque(colors.success);
    let minimize_f32 = opaque(colors.warning);

    if show_title {
        deco.buffers
            .titlebar
            .update((total_w, TITLE_BAR_HEIGHT + bw), title_f32);
        deco.buffers
            .close_btn
            .update((BUTTON_SIZE, BUTTON_SIZE), close_f32);
        deco.buffers
            .maximize_btn
            .update((BUTTON_SIZE, BUTTON_SIZE), maximize_f32);
        deco.buffers
            .minimize_btn
            .update((BUTTON_SIZE, BUTTON_SIZE), minimize_f32);
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
        let sr = theme.shadow_radius as i32;
        let shadow = SsdChromeMetrics::new(SsdFrameMetrics::from_frame_origin(
            (0, 0).into(),
            (cw, ch).into(),
            bw,
            title_h,
        ))
        .shadow_metrics(sr)
        .expect("shadow metrics should exist when border width is positive");
        deco.buffers
            .shadow
            .update((shadow.rect.size.w, shadow.rect.size.h), SHADOW_COLOR);
    }

    deco.last_content_size = (cw, ch);
    deco.last_bw = bw;
    deco.dirty = false;
}
