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
    /// Click landed on the volume bar; payload is the target volume (0..=100)
    /// derived from the click x position.
    Volume(u8),
}

thread_local! {
    static SETTINGS_LINK_RECT: std::cell::Cell<Rect> = const { std::cell::Cell::new(Rect { x: 0, y: 0, w: 0, h: 0 }) };
    static VOLUME_BAR_RECT: std::cell::Cell<Rect> = const { std::cell::Cell::new(Rect { x: 0, y: 0, w: 0, h: 0 }) };
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
    draw_status_row(
        painter,
        font,
        theme,
        "Status",
        status_text,
        dot_color,
        row_y,
    );
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
    let volume_bar = draw_volume_row(painter, font, theme, "Lautstärke", percent, muted, row_y);
    VOLUME_BAR_RECT.with(|r| r.set(volume_bar));
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

/// Compact volume OSD: a single centred volume row (label + draggable bar + %).
/// Shown on volume-key activity. Stores the bar rect for drag hit-testing.
pub fn draw_volume_osd(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    snapshot: &AudioSnapshot,
) {
    draw_card_body(painter, theme);
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
    let row_y = (crate::VOLUME_OSD_HEIGHT as i32 - ROW_HEIGHT) / 2;
    let bar = draw_volume_row(painter, font, theme, "Lautstärke", percent, muted, row_y);
    VOLUME_BAR_RECT.with(|r| r.set(bar));
}

/// Compact text OSD: a single centred key/value row. Reuses the volume OSD
/// surface to flash e.g. the active power profile ("Energieprofil → Standard").
pub fn draw_text_osd(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    key: &str,
    value: &str,
) {
    draw_card_body(painter, theme);
    let row_y = (crate::VOLUME_OSD_HEIGHT as i32 - ROW_HEIGHT) / 2;
    draw_kv_row(painter, font, theme, key, value, row_y);
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
    let bar = VOLUME_BAR_RECT.with(|r| r.get());
    if bar.w > 0 && bar.contains(x, y) {
        return Some(AudioPopupHit::Volume(volume_from_bar_x(bar, x)));
    }
    Some(AudioPopupHit::Card)
}

/// During a drag, map a card-relative x to a volume percent using the last
/// drawn bar, ignoring y (so the level keeps tracking even if the pointer
/// strays vertically). Returns None when no bar is currently shown.
pub fn volume_from_x(x: f64) -> Option<u8> {
    let bar = VOLUME_BAR_RECT.with(|r| r.get());
    if bar.w <= 0 {
        return None;
    }
    Some(volume_from_bar_x(bar, x))
}

/// Map a click x position over the volume bar `bar` to a 0..=100 percent.
fn volume_from_bar_x(bar: Rect, x: f64) -> u8 {
    if bar.w <= 0 {
        return 0;
    }
    let fraction = ((x - bar.x as f64) / bar.w as f64).clamp(0.0, 1.0);
    (fraction * 100.0).round() as u8
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
    fn volume_from_bar_x_maps_position_to_percent() {
        let bar = crate::Rect {
            x: 100,
            y: 0,
            w: 200,
            h: 20,
        };
        assert_eq!(super::volume_from_bar_x(bar, 100.0), 0);
        assert_eq!(super::volume_from_bar_x(bar, 200.0), 50);
        assert_eq!(super::volume_from_bar_x(bar, 300.0), 100);
        // Positions outside the bar clamp to the ends.
        assert_eq!(super::volume_from_bar_x(bar, 40.0), 0);
        assert_eq!(super::volume_from_bar_x(bar, 500.0), 100);
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
