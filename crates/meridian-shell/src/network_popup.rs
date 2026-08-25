use std::cell::{Cell, RefCell};

use meridian_config::ThemeConfig;

use crate::{
    audio::AudioSnapshot,
    battery::BatterySnapshot,
    network::{NetworkState, WifiNetwork},
    popup_card::{
        draw_card_body, draw_card_title, draw_footer_link, PAD_X, ROW_HEIGHT,
        ROW_TEXT_BASELINE_OFFSET,
    },
    power_profile::PowerProfile,
    ui::tokens::{glass_dim_from_config, glass_foreground_from_config},
    Painter, Rect, TextRenderer, NETWORK_POPUP_HEIGHT, NETWORK_POPUP_WIDTH,
};

/// Which tab of the network popup is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkTab {
    Status,
    Wifi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkPopupHit {
    Card,
    SettingsLink,
    Tab(NetworkTab),
    /// A Wi-Fi list row; the index is into the popup's `wifi_networks` slice.
    WifiNetwork(usize),
    Quick(crate::quick_settings_popup::QuickSettingsHit),
}

pub struct NetworkPopupState<'a> {
    pub network: &'a NetworkState,
    pub audio: &'a AudioSnapshot,
    pub battery: &'a BatterySnapshot,
    pub power_profile: Option<PowerProfile>,
    pub theme_name: &'a str,
    pub power_armed: bool,
    pub active_tab: NetworkTab,
    pub wifi_networks: &'a [WifiNetwork],
}

/// Top of the tab strip, just under the title rule.
const TABS_Y: i32 = 38;
const TAB_H: i32 = 24;
/// Where tab content (status rows / Wi-Fi list) begins.
const CONTENT_TOP: i32 = 72;
/// Cap on Wi-Fi rows shown in the narrow popup (strongest-first); the rest are
/// reachable via the Settings network page.
const MAX_WIFI_ROWS: usize = 6;

thread_local! {
    static SETTINGS_LINK_RECT: Cell<Rect> = const { Cell::new(Rect { x: 0, y: 0, w: 0, h: 0 }) };
    static TAB_STATUS_RECT: Cell<Rect> = const { Cell::new(Rect { x: 0, y: 0, w: 0, h: 0 }) };
    static TAB_WIFI_RECT: Cell<Rect> = const { Cell::new(Rect { x: 0, y: 0, w: 0, h: 0 }) };
    static WIFI_ROW_RECTS: RefCell<Vec<Rect>> = const { RefCell::new(Vec::new()) };
    static QUICK_SETTINGS_ACTIVE: Cell<bool> = const { Cell::new(false) };
}

pub fn draw_network_popup(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    state: &NetworkPopupState<'_>,
) {
    match state.active_tab {
        NetworkTab::Status => {
            QUICK_SETTINGS_ACTIVE.with(|active| active.set(true));
            // No stale Wi-Fi row hit targets while the status tab is shown.
            WIFI_ROW_RECTS.with(|r| r.borrow_mut().clear());
            crate::quick_settings_popup::draw(
                painter,
                font,
                theme,
                crate::quick_settings_popup::QuickSettingsState {
                    network: state.network,
                    audio: state.audio,
                    battery: state.battery,
                    power_profile: state.power_profile,
                    theme_name: state.theme_name,
                    power_armed: state.power_armed,
                },
            );
        }
        NetworkTab::Wifi => {
            QUICK_SETTINGS_ACTIVE.with(|active| active.set(false));
            draw_card_body(painter, theme);
            draw_card_title(painter, font, theme, "Netzwerk");
            draw_tabs(painter, font, theme, state.active_tab);
            draw_wifi_tab(painter, font, theme, state.wifi_networks);
            let link_rect = draw_footer_link(
                painter,
                font,
                theme,
                NETWORK_POPUP_HEIGHT as i32,
                "Netzwerkeinstellungen",
            );
            SETTINGS_LINK_RECT.with(|r| r.set(link_rect));
        }
    }
}

fn draw_wifi_tab(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    wifi_networks: &[WifiNetwork],
) {
    let width = NETWORK_POPUP_WIDTH as i32;

    if wifi_networks.is_empty() {
        WIFI_ROW_RECTS.with(|r| r.borrow_mut().clear());
        painter.text_clipped(
            font,
            "Keine WLAN-Netzwerke gefunden",
            PAD_X,
            CONTENT_TOP + ROW_TEXT_BASELINE_OFFSET,
            width - 2 * PAD_X,
            glass_dim_from_config(theme),
        );
        return;
    }

    let mut rects = Vec::with_capacity(MAX_WIFI_ROWS);
    for (i, net) in wifi_networks.iter().take(MAX_WIFI_ROWS).enumerate() {
        let row_y = CONTENT_TOP + i as i32 * ROW_HEIGHT;
        let baseline = row_y + ROW_TEXT_BASELINE_OFFSET;

        // Right: signal percent (dim).
        let signal_text = format!("{}%", net.signal.min(100));
        let signal_w = measure(font, &signal_text, 32);
        let signal_x = width - PAD_X - signal_w;
        painter.text_clipped(
            font,
            &signal_text,
            signal_x,
            baseline,
            signal_w,
            glass_dim_from_config(theme),
        );

        // Left: SSID, accent-coloured when this is the connected network, plus
        // a leading check mark; a trailing lock mark when it is secured.
        let label = if net.secured {
            format!("{} \u{1f512}", net.ssid)
        } else {
            net.ssid.clone()
        };
        let label = if net.in_use {
            format!("\u{2713} {label}")
        } else {
            label
        };
        let ssid_color = if net.in_use {
            theme.colors.accent
        } else {
            glass_foreground_from_config(theme)
        };
        let ssid_max = signal_x - PAD_X - 8;
        painter.text_clipped(font, &label, PAD_X, baseline, ssid_max.max(0), ssid_color);

        rects.push(Rect {
            x: 0,
            y: row_y,
            w: width,
            h: ROW_HEIGHT,
        });
    }
    WIFI_ROW_RECTS.with(|r| *r.borrow_mut() = rects);
}

fn draw_tabs(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    active_tab: NetworkTab,
) {
    let status_x = PAD_X;
    let status_w = draw_tab(
        painter,
        font,
        theme,
        "Status",
        status_x,
        active_tab == NetworkTab::Status,
    );
    TAB_STATUS_RECT.with(|r| {
        r.set(Rect {
            x: status_x - 6,
            y: TABS_Y,
            w: status_w + 12,
            h: TAB_H,
        })
    });

    let wifi_x = status_x + status_w + 24;
    let wifi_w = draw_tab(
        painter,
        font,
        theme,
        "WLAN",
        wifi_x,
        active_tab == NetworkTab::Wifi,
    );
    TAB_WIFI_RECT.with(|r| {
        r.set(Rect {
            x: wifi_x - 6,
            y: TABS_Y,
            w: wifi_w + 12,
            h: TAB_H,
        })
    });
}

/// Draw a single tab label and (when active) a short accent underline; returns
/// the measured label width so the caller can place the next tab.
fn draw_tab(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    label: &str,
    x: i32,
    active: bool,
) -> i32 {
    let color = if active {
        glass_foreground_from_config(theme)
    } else {
        glass_dim_from_config(theme)
    };
    let w = measure(font, label, label.chars().count() as i32 * 8);
    painter.text_clipped(font, label, x, TABS_Y + 16, w, color);
    if active {
        painter.rect(
            Rect {
                x,
                y: TABS_Y + TAB_H - 3,
                w,
                h: 2,
            },
            theme.colors.accent,
        );
    }
    w
}

fn measure(font: &RefCell<Option<TextRenderer>>, text: &str, fallback: i32) -> i32 {
    font.borrow_mut()
        .as_mut()
        .map(|r| r.measure_text(text))
        .unwrap_or(fallback)
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
    if QUICK_SETTINGS_ACTIVE.with(Cell::get) {
        return crate::quick_settings_popup::hit_test(width, height, x, y)
            .map(NetworkPopupHit::Quick);
    }
    if TAB_STATUS_RECT.with(|r| r.get()).contains(x, y) {
        return Some(NetworkPopupHit::Tab(NetworkTab::Status));
    }
    if TAB_WIFI_RECT.with(|r| r.get()).contains(x, y) {
        return Some(NetworkPopupHit::Tab(NetworkTab::Wifi));
    }
    if let Some(idx) =
        WIFI_ROW_RECTS.with(|r| r.borrow().iter().position(|rect| rect.contains(x, y)))
    {
        return Some(NetworkPopupHit::WifiNetwork(idx));
    }
    let link = SETTINGS_LINK_RECT.with(|r| r.get());
    if link.w > 0 && link.h > 0 && link.contains(x, y) {
        return Some(NetworkPopupHit::SettingsLink);
    }
    Some(NetworkPopupHit::Card)
}

#[cfg(test)]
mod tests {
    use super::{
        draw_network_popup, popup_hit_test, NetworkPopupHit, NetworkPopupState, NetworkTab,
    };
    use crate::{
        audio::AudioSnapshot,
        network::{NetworkState, WifiNetwork},
    };

    fn render(active_tab: NetworkTab, nets: &[WifiNetwork]) {
        let width = crate::NETWORK_POPUP_WIDTH as i32;
        let height = crate::NETWORK_POPUP_HEIGHT as i32;
        let mut surface = vec![0_u8; (width * height * 4) as usize];
        let mut painter = crate::Painter::new(&mut surface, width, height);
        let theme = meridian_config::ThemeConfig::default();
        let font = std::cell::RefCell::new(None);
        draw_network_popup(
            &mut painter,
            &font,
            &theme,
            &NetworkPopupState {
                network: &NetworkState::Offline,
                audio: &AudioSnapshot::unavailable(),
                battery: &crate::battery::BatterySnapshot::default(),
                power_profile: None,
                theme_name: "meridian-dark",
                power_armed: false,
                active_tab,
                wifi_networks: nets,
            },
        );
    }

    fn net(ssid: &str, signal: u8, secured: bool, in_use: bool) -> WifiNetwork {
        WifiNetwork {
            ssid: ssid.to_string(),
            signal,
            secured,
            in_use,
        }
    }

    #[test]
    fn popup_hit_detection_reports_inside_and_outside() {
        render(NetworkTab::Status, &[]);
        let w = crate::NETWORK_POPUP_WIDTH;
        let h = crate::NETWORK_POPUP_HEIGHT;
        assert!(matches!(
            popup_hit_test(w, h, 5.0, 5.0),
            Some(NetworkPopupHit::Quick(
                crate::quick_settings_popup::QuickSettingsHit::Card
            ))
        ));
        assert_eq!(popup_hit_test(w, h, -1.0, 5.0), None);
        assert_eq!(popup_hit_test(w, h, 1000.0, 5.0), None);
    }

    #[test]
    fn popup_hit_test_returns_settings_link_in_network_detail_footer() {
        render(NetworkTab::Wifi, &[]);
        let h = crate::NETWORK_POPUP_HEIGHT;
        let probe_x = (crate::popup_card::POPUP_WIDTH as f64) - 30.0;
        let probe_y = (h as f64) - 18.0;
        assert_eq!(
            popup_hit_test(crate::NETWORK_POPUP_WIDTH, h, probe_x, probe_y),
            Some(NetworkPopupHit::SettingsLink)
        );
    }

    #[test]
    fn tabs_are_hit_testable() {
        render(NetworkTab::Wifi, &[]);
        let w = crate::NETWORK_POPUP_WIDTH;
        let h = crate::NETWORK_POPUP_HEIGHT;
        // The Status tab sits at the left of the tab strip; the WLAN tab to its
        // right. Probe both at the strip's vertical centre.
        assert_eq!(
            popup_hit_test(w, h, 24.0, 48.0),
            Some(NetworkPopupHit::Tab(NetworkTab::Status))
        );
        assert_eq!(
            popup_hit_test(w, h, 95.0, 48.0),
            Some(NetworkPopupHit::Tab(NetworkTab::Wifi))
        );
    }

    #[test]
    fn wifi_rows_hit_test_to_their_index() {
        let nets = [
            net("HomeNet", 80, true, true),
            net("Cafe", 60, false, false),
            net("Office", 40, true, false),
        ];
        render(NetworkTab::Wifi, &nets);
        let w = crate::NETWORK_POPUP_WIDTH;
        let h = crate::NETWORK_POPUP_HEIGHT;
        // First row.
        assert_eq!(
            popup_hit_test(w, h, 40.0, 80.0),
            Some(NetworkPopupHit::WifiNetwork(0))
        );
        // Second row, one ROW_HEIGHT lower.
        assert_eq!(
            popup_hit_test(w, h, 40.0, 80.0 + crate::popup_card::ROW_HEIGHT as f64),
            Some(NetworkPopupHit::WifiNetwork(1))
        );
    }

    #[test]
    fn switching_to_status_tab_clears_wifi_row_targets() {
        let nets = [net("HomeNet", 80, true, true)];
        render(NetworkTab::Wifi, &nets);
        // Re-render on the status tab: the previous Wi-Fi row must no longer
        // register as a network hit.
        render(NetworkTab::Status, &nets);
        let w = crate::NETWORK_POPUP_WIDTH;
        let h = crate::NETWORK_POPUP_HEIGHT;
        assert!(!matches!(
            popup_hit_test(w, h, 40.0, 80.0),
            Some(NetworkPopupHit::WifiNetwork(_))
        ));
    }
}
