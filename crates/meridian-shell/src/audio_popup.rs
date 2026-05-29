use std::cell::RefCell;

use meridian_config::ThemeConfig;

use crate::{
    audio::{AudioServiceState, AudioSnapshot},
    popup_card::{
        draw_card_body, draw_card_title, draw_footer_link, draw_kv_row, draw_status_row,
        draw_volume_row, BODY_TOP, ROW_HEIGHT,
    },
    Painter, Rect, TextRenderer, AUDIO_POPUP_HEIGHT,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioPopupHit {
    Card,
    SettingsLink,
}

thread_local! {
    static SETTINGS_LINK_RECT: std::cell::Cell<Rect> = const { std::cell::Cell::new(Rect { x: 0, y: 0, w: 0, h: 0 }) };
}

pub fn draw_audio_popup(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    snapshot: &AudioSnapshot,
) {
    let colors = &theme.colors;
    let height = AUDIO_POPUP_HEIGHT as i32;

    draw_card_body(painter, theme);
    draw_card_title(painter, font, theme, "Audio");

    let mut row_y = BODY_TOP;

    let (status_text, dot_color) = match snapshot.service {
        AudioServiceState::Running => ("Aktiv", colors.success),
        AudioServiceState::Unavailable => ("Nicht verfügbar", colors.text_dim),
    };
    draw_status_row(painter, font, theme, "Status", status_text, dot_color, row_y);
    row_y += ROW_HEIGHT;

    let output = snapshot
        .default_output
        .as_ref()
        .map(|device| fit_text(&device.name, 24))
        .unwrap_or_else(|| "Keine".to_string());
    draw_kv_row(painter, font, theme, "Ausgabe", &output, row_y);
    row_y += ROW_HEIGHT;

    let percent = snapshot
        .default_output
        .as_ref()
        .and_then(|device| device.volume_percent)
        .map(u32::from);
    let muted = snapshot
        .default_output
        .as_ref()
        .map(|device| device.muted)
        .unwrap_or(false);
    draw_volume_row(painter, font, theme, "Lautstärke", percent, muted, row_y);
    row_y += ROW_HEIGHT;

    let input = snapshot
        .default_input
        .as_ref()
        .map(|device| fit_text(&device.name, 24))
        .unwrap_or_else(|| "Kein".to_string());
    draw_kv_row(painter, font, theme, "Mikrofon", &input, row_y);

    let link_rect = draw_footer_link(painter, font, theme, height, "Soundeinstellungen");
    SETTINGS_LINK_RECT.with(|r| r.set(link_rect));
}

fn fit_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('\u{2026}');
    out
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
    let link = SETTINGS_LINK_RECT.with(|r| r.get());
    if link.w > 0 && link.h > 0 && link.contains(x, y) {
        return Some(AudioPopupHit::SettingsLink);
    }
    Some(AudioPopupHit::Card)
}

#[cfg(test)]
mod tests {
    use super::{draw_audio_popup, popup_hit_test, AudioPopupHit};
    use crate::audio::AudioSnapshot;

    fn render_for_test() {
        let width = crate::AUDIO_POPUP_WIDTH as i32;
        let height = crate::AUDIO_POPUP_HEIGHT as i32;
        let mut surface = vec![0_u8; (width * height * 4) as usize];
        let mut painter = crate::Painter::new(&mut surface, width, height);
        let theme = meridian_config::ThemeConfig::default();
        let font = std::cell::RefCell::new(None);
        draw_audio_popup(&mut painter, &font, &theme, &AudioSnapshot::unavailable());
    }

    #[test]
    fn popup_hit_detection_reports_inside_and_outside() {
        render_for_test();
        let w = crate::AUDIO_POPUP_WIDTH;
        let h = crate::AUDIO_POPUP_HEIGHT;
        assert_eq!(popup_hit_test(w, h, 1.0, 1.0), Some(AudioPopupHit::Card));
        assert_eq!(popup_hit_test(w, h, -1.0, 5.0), None);
        assert_eq!(popup_hit_test(w, h, 1000.0, 5.0), None);
    }

    #[test]
    fn popup_hit_test_returns_settings_link_in_footer() {
        render_for_test();
        let w = crate::AUDIO_POPUP_WIDTH;
        let h = crate::AUDIO_POPUP_HEIGHT;
        // Settings link sits in the bottom-right area; the bitmap fallback
        // measures slightly differently from a real font but the link rect
        // is always inside the bottom-right quadrant.
        let probe_x = (w as f64) - 30.0;
        let probe_y = (h as f64) - 18.0;
        assert_eq!(
            popup_hit_test(w, h, probe_x, probe_y),
            Some(AudioPopupHit::SettingsLink)
        );
    }
}
