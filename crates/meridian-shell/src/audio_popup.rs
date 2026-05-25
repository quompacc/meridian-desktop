use std::cell::RefCell;

use meridian_config::ThemeConfig;

use crate::{
    audio::{AudioServiceState, AudioSnapshot},
    Painter, Rect, TextRenderer, AUDIO_POPUP_HEIGHT, AUDIO_POPUP_WIDTH,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioPopupHit {
    Card,
    SettingsLink,
}

pub fn draw_audio_popup(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    snapshot: &AudioSnapshot,
) {
    let colors = &theme.colors;
    let width = AUDIO_POPUP_WIDTH as i32;
    let height = AUDIO_POPUP_HEIGHT as i32;

    painter.clear(colors.surface_alt);
    painter.stroke_rect(
        Rect {
            x: 0,
            y: 0,
            w: width,
            h: height,
        },
        colors.border,
    );

    const PAD_X: i32 = 14;
    const TITLE_TOP: i32 = 12;
    const TITLE_HEIGHT: i32 = 22;
    const SEP_HEIGHT: i32 = 2;
    const BODY_TOP_PAD: i32 = 14;
    const ROW_HEIGHT: i32 = 22;
    const DOT_SIZE: i32 = 10;

    painter.text_clipped(
        font,
        "Sound",
        PAD_X,
        TITLE_TOP + 14,
        width - 2 * PAD_X,
        colors.accent,
    );
    let sep_y = TITLE_TOP + TITLE_HEIGHT;
    painter.rect(
        Rect {
            x: 0,
            y: sep_y,
            w: width,
            h: SEP_HEIGHT,
        },
        colors.accent,
    );

    let label_x = PAD_X;
    let value_x = width / 2 - 12;
    let label_w = value_x - label_x - 8;
    let value_w = width - value_x - PAD_X;
    let mut row_y = sep_y + SEP_HEIGHT + BODY_TOP_PAD;

    let (status_text, dot_color) = match snapshot.service {
        AudioServiceState::Running => ("Active", colors.success),
        AudioServiceState::Unavailable => ("Unavailable", colors.text_dim),
    };
    draw_row(
        painter,
        font,
        "Status",
        status_text,
        label_x,
        value_x,
        row_y,
        label_w,
        value_w,
        colors.text_dim,
        colors.text,
    );
    painter.rect(
        Rect {
            x: value_x,
            y: row_y + 6,
            w: DOT_SIZE,
            h: DOT_SIZE,
        },
        dot_color,
    );
    row_y += ROW_HEIGHT;

    let output = snapshot
        .default_output
        .as_ref()
        .map(|device| fit_text(&device.name, 28))
        .unwrap_or_else(|| "None".to_string());
    draw_row(
        painter,
        font,
        "Output",
        &output,
        label_x,
        value_x,
        row_y,
        label_w,
        value_w,
        colors.text_dim,
        colors.text,
    );
    row_y += ROW_HEIGHT;

    let volume = snapshot
        .default_output
        .as_ref()
        .and_then(|device| device.volume_percent)
        .map(|value| format!("{value}%"))
        .unwrap_or_else(|| "-".to_string());
    let muted = snapshot
        .default_output
        .as_ref()
        .map(|device| device.muted)
        .unwrap_or(false);
    let volume_text = if muted {
        format!("{volume} muted")
    } else {
        volume
    };
    draw_row(
        painter,
        font,
        "Volume",
        &volume_text,
        label_x,
        value_x,
        row_y,
        label_w,
        value_w,
        colors.text_dim,
        colors.text,
    );
    row_y += ROW_HEIGHT;

    let input = snapshot
        .default_input
        .as_ref()
        .map(|device| fit_text(&device.name, 28))
        .unwrap_or_else(|| "None".to_string());
    draw_row(
        painter,
        font,
        "Input",
        &input,
        label_x,
        value_x,
        row_y,
        label_w,
        value_w,
        colors.text_dim,
        colors.text,
    );

    let link = settings_link_rect(width, height);
    painter.roundish_rect_with_radius(link, colors.surface, 3);
    painter.stroke_rect(link, colors.border);
    painter.text_clipped(
        font,
        "Settings",
        link.x + 12,
        link.y + 15,
        link.w - 24,
        colors.text,
    );

    let settings_y = height - 18;
    painter.text_clipped(
        font,
        "System -> Sound",
        PAD_X,
        settings_y,
        link.x - PAD_X - 8,
        colors.text_dim,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_row(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    label: &str,
    value: &str,
    label_x: i32,
    value_x: i32,
    row_y: i32,
    label_w: i32,
    value_w: i32,
    label_color: meridian_config::Color,
    value_color: meridian_config::Color,
) {
    let baseline = row_y + 14;
    painter.text_clipped(font, label, label_x, baseline, label_w, label_color);
    painter.text_clipped(font, value, value_x, baseline, value_w, value_color);
}

fn fit_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(3)).collect();
    out.push_str("...");
    out
}

fn settings_link_rect(width: i32, height: i32) -> Rect {
    Rect {
        x: width - 102,
        y: height - 32,
        w: 88,
        h: 22,
    }
}

pub fn popup_hit_test(width: u32, height: u32, x: f64, y: f64) -> Option<AudioPopupHit> {
    let bounds = Rect {
        x: 0,
        y: 0,
        w: width as i32,
        h: height as i32,
    };
    if !bounds.contains(x, y) {
        return None;
    }
    if settings_link_rect(width as i32, height as i32).contains(x, y) {
        return Some(AudioPopupHit::SettingsLink);
    }
    Some(AudioPopupHit::Card)
}

#[cfg(test)]
mod tests {
    use super::{popup_hit_test, AudioPopupHit};

    #[test]
    fn popup_hit_detection_reports_inside_and_outside() {
        assert_eq!(
            popup_hit_test(280, 172, 1.0, 1.0),
            Some(AudioPopupHit::Card)
        );
        assert_eq!(
            popup_hit_test(280, 172, 190.0, 145.0),
            Some(AudioPopupHit::SettingsLink)
        );
        assert_eq!(popup_hit_test(280, 172, -1.0, 5.0), None);
        assert_eq!(popup_hit_test(280, 172, 300.0, 5.0), None);
    }
}
