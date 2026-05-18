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

    // Metro card: flat surface_alt body + 1px outer border, no rounding.
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
        "Network",
        PAD_X,
        TITLE_TOP + 14,
        width - 2 * PAD_X,
        colors.accent,
    );

    // Full-width 2px accent separator under the title (Metro indicator).
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

    // Body layout: two columns (label dim, value text).
    let label_x = PAD_X;
    let value_x = width / 2 - 12;
    let label_w = value_x - label_x - 8;
    let value_w = width - value_x - PAD_X;
    let mut row_y = sep_y + SEP_HEIGHT + BODY_TOP_PAD;

    let (status_text, dot_color) = match state {
        NetworkState::Connected { .. } => ("Active", colors.success),
        NetworkState::Disconnected => ("Off", colors.error),
        NetworkState::Offline => ("Unavailable", colors.text_dim),
    };
    let status_baseline = row_y + 14;
    painter.text_clipped(
        font,
        "Status",
        label_x,
        status_baseline,
        label_w,
        colors.text_dim,
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
    painter.text_clipped(
        font,
        status_text,
        value_x + DOT_SIZE + 8,
        status_baseline,
        value_w - DOT_SIZE - 8,
        colors.text,
    );
    row_y += ROW_HEIGHT;

    if let NetworkState::Connected {
        kind,
        connection_name,
    } = state
    {
        let kind_label = match kind {
            ConnectionKind::Ethernet => "Ethernet",
            ConnectionKind::Wifi { .. } => "Wi-Fi",
            ConnectionKind::Vpn => "VPN",
            ConnectionKind::Other => "Other",
        };
        let type_baseline = row_y + 14;
        painter.text_clipped(
            font,
            "Type",
            label_x,
            type_baseline,
            label_w,
            colors.text_dim,
        );
        painter.text_clipped(
            font,
            kind_label,
            value_x,
            type_baseline,
            value_w,
            colors.text,
        );
        row_y += ROW_HEIGHT;

        let conn_baseline = row_y + 14;
        painter.text_clipped(
            font,
            "Connection",
            label_x,
            conn_baseline,
            label_w,
            colors.text_dim,
        );
        painter.text_clipped(
            font,
            connection_name,
            value_x,
            conn_baseline,
            value_w,
            colors.text,
        );

        if let ConnectionKind::Wifi { signal } = kind {
            let signal_text = signal
                .map(|v| format!("{v}%"))
                .unwrap_or_else(|| "—".to_string());
            let sig_row_y = row_y + ROW_HEIGHT;
            let sig_baseline = sig_row_y + 14;
            painter.text_clipped(
                font,
                "Signal",
                label_x,
                sig_baseline,
                label_w,
                colors.text_dim,
            );
            painter.text_clipped(
                font,
                &signal_text,
                value_x,
                sig_baseline,
                value_w,
                colors.text,
            );
        }
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
        assert!(popup_hit_test(280, 150, 1.0, 1.0));
        assert!(popup_hit_test(280, 150, 279.0, 149.0));
        assert!(!popup_hit_test(280, 150, -1.0, 5.0));
        assert!(!popup_hit_test(280, 150, 300.0, 5.0));
    }
}
