use std::cell::RefCell;

use meridian_config::ThemeConfig;

use crate::{
    network::{ConnectionKind, NetworkState},
    Painter, Rect, TextRenderer, NETWORK_POPUP_HEIGHT, NETWORK_POPUP_WIDTH,
};

pub fn draw_network_popup(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    state: &NetworkState,
) {
    let colors = &theme.colors;
    let width = NETWORK_POPUP_WIDTH as i32;
    let height = NETWORK_POPUP_HEIGHT as i32;

    painter.clear(colors.surface_alt);
    painter.rect(
        Rect {
            x: 0,
            y: 0,
            w: width,
            h: 1,
        },
        colors.border,
    );
    painter.rect(
        Rect {
            x: 0,
            y: height - 1,
            w: width,
            h: 1,
        },
        colors.border,
    );
    painter.rect(
        Rect {
            x: 0,
            y: 0,
            w: 1,
            h: height,
        },
        colors.border,
    );
    painter.rect(
        Rect {
            x: width - 1,
            y: 0,
            w: 1,
            h: height,
        },
        colors.border,
    );

    let card = Rect {
        x: 8,
        y: 8,
        w: width - 16,
        h: height - 16,
    };
    painter.roundish_rect_with_radius(card, colors.surface, 10);
    painter.stroke_rect(card, colors.border);

    painter.text_clipped(
        font,
        "Network",
        card.x + 12,
        card.y + 22,
        card.w - 24,
        theme.colors.accent,
    );

    let mut lines = vec![state.summary()];
    match state {
        NetworkState::Connected {
            kind,
            connection_name,
        } => {
            let kind_label = match kind {
                ConnectionKind::Ethernet => "Ethernet",
                ConnectionKind::Wifi { .. } => "WiFi",
                ConnectionKind::Vpn => "VPN",
                ConnectionKind::Other => "Other",
            };
            lines.push(format!("Type: {kind_label}"));
            lines.push(format!("Connection: {connection_name}"));
            lines.push("Status: connected".to_string());
            if let ConnectionKind::Wifi { signal } = kind {
                let signal_line = signal
                    .map(|value| format!("Signal: {value}%"))
                    .unwrap_or_else(|| "Signal: unknown".to_string());
                lines.push(signal_line);
            }
        }
        NetworkState::Disconnected => {
            lines.push("Status: disconnected".to_string());
        }
        NetworkState::Offline => {
            lines.push("NetworkManager unavailable".to_string());
        }
    }

    let mut y = card.y + 46;
    for line in lines {
        painter.text_clipped(font, &line, card.x + 12, y, card.w - 24, colors.text);
        y += 18;
    }
}

pub fn popup_hit_test(width: u32, height: u32, x: f64, y: f64) -> bool {
    Rect {
        x: 0,
        y: 0,
        w: width as i32,
        h: height as i32,
    }
    .contains(x, y)
}

#[cfg(test)]
mod tests {
    use super::popup_hit_test;

    #[test]
    fn popup_hit_detection_reports_inside_and_outside() {
        assert!(popup_hit_test(240, 120, 1.0, 1.0));
        assert!(popup_hit_test(240, 120, 239.0, 119.0));
        assert!(!popup_hit_test(240, 120, -1.0, 5.0));
        assert!(!popup_hit_test(240, 120, 260.0, 5.0));
    }
}
