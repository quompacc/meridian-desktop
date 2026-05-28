use std::cell::RefCell;

use meridian_config::ThemeConfig;

use crate::{
    network::{ConnectionKind, NetworkState},
    popup_card::{
        draw_card_body, draw_card_title, draw_footer_link, draw_kv_row, draw_status_row, BODY_TOP,
        ROW_HEIGHT,
    },
    Painter, Rect, TextRenderer, NETWORK_POPUP_HEIGHT,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkPopupHit {
    Card,
    SettingsLink,
}

thread_local! {
    static SETTINGS_LINK_RECT: std::cell::Cell<Rect> = std::cell::Cell::new(Rect { x: 0, y: 0, w: 0, h: 0 });
}

pub fn draw_network_popup(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    state: &NetworkState,
) {
    let colors = &theme.colors;
    let height = NETWORK_POPUP_HEIGHT as i32;

    draw_card_body(painter, theme);
    draw_card_title(painter, font, theme, "Netzwerk");

    let mut row_y = BODY_TOP;

    let (status_text, dot_color) = match state {
        NetworkState::Connected { .. } => ("Aktiv", colors.success),
        NetworkState::Disconnected => ("Aus", colors.error),
        NetworkState::Offline => ("Nicht verfügbar", colors.text_dim),
    };
    draw_status_row(painter, font, theme, "Status", status_text, dot_color, row_y);
    row_y += ROW_HEIGHT;

    if let NetworkState::Connected {
        kind,
        connection_name,
    } = state
    {
        let kind_label = match kind {
            ConnectionKind::Ethernet => "Ethernet",
            ConnectionKind::Wifi { .. } => "WLAN",
            ConnectionKind::Vpn => "VPN",
            ConnectionKind::Other => "Sonstige",
        };
        draw_kv_row(painter, font, theme, "Typ", kind_label, row_y);
        row_y += ROW_HEIGHT;

        draw_kv_row(painter, font, theme, "Verbindung", connection_name, row_y);
        row_y += ROW_HEIGHT;

        if let ConnectionKind::Wifi { signal } = kind {
            let signal_text = signal
                .map(|v| format!("{v}%"))
                .unwrap_or_else(|| "\u{2014}".to_string());
            draw_kv_row(painter, font, theme, "Signal", &signal_text, row_y);
        }
    }

    let link_rect = draw_footer_link(painter, font, theme, height, "Netzwerkeinstellungen");
    SETTINGS_LINK_RECT.with(|r| r.set(link_rect));
}

pub fn popup_hit_test(width: u32, height: u32, x: f64, y: f64) -> Option<NetworkPopupHit> {
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
        return Some(NetworkPopupHit::SettingsLink);
    }
    Some(NetworkPopupHit::Card)
}

#[cfg(test)]
mod tests {
    use super::{draw_network_popup, popup_hit_test, NetworkPopupHit};
    use crate::network::NetworkState;

    fn render_for_test() {
        let width = crate::NETWORK_POPUP_WIDTH as i32;
        let height = crate::NETWORK_POPUP_HEIGHT as i32;
        let mut surface = vec![0_u8; (width * height * 4) as usize];
        let mut painter = crate::Painter::new(&mut surface, width, height);
        let theme = meridian_config::ThemeConfig::default();
        let font = std::cell::RefCell::new(None);
        draw_network_popup(&mut painter, &font, &theme, &NetworkState::Offline);
    }

    #[test]
    fn popup_hit_detection_reports_inside_and_outside() {
        render_for_test();
        let w = crate::NETWORK_POPUP_WIDTH;
        let h = crate::NETWORK_POPUP_HEIGHT;
        assert_eq!(popup_hit_test(w, h, 1.0, 1.0), Some(NetworkPopupHit::Card));
        assert_eq!(popup_hit_test(w, h, -1.0, 5.0), None);
        assert_eq!(popup_hit_test(w, h, 1000.0, 5.0), None);
    }

    #[test]
    fn popup_hit_test_returns_settings_link_in_footer() {
        render_for_test();
        let w = crate::NETWORK_POPUP_WIDTH;
        let h = crate::NETWORK_POPUP_HEIGHT;
        let probe_x = (w as f64) - 30.0;
        let probe_y = (h as f64) - 18.0;
        assert_eq!(
            popup_hit_test(w, h, probe_x, probe_y),
            Some(NetworkPopupHit::SettingsLink)
        );
    }
}
