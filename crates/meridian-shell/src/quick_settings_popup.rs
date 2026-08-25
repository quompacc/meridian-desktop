use std::cell::{Cell, RefCell};

use meridian_config::ThemeConfig;
use meridian_tokens::{Interaction, QuickSettings, Radius};

use crate::{
    audio::AudioSnapshot,
    battery::BatterySnapshot,
    network::{ConnectionKind, NetworkState},
    popup_card::draw_card_body,
    power_profile::PowerProfile,
    ui::tokens::{glass_border_from_config, glass_dim_from_config, glass_foreground_from_config},
    Painter, Rect, TextRenderer,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickSettingsHit {
    Card,
    Settings,
    Network,
    AudioMute,
    Volume(u8),
    Appearance,
    PowerProfile,
    Lock,
    PowerOff,
}

const ZERO_RECT: Rect = Rect {
    x: 0,
    y: 0,
    w: 0,
    h: 0,
};

thread_local! {
    static SETTINGS: Cell<Rect> = const { Cell::new(ZERO_RECT) };
    static NETWORK: Cell<Rect> = const { Cell::new(ZERO_RECT) };
    static AUDIO_MUTE: Cell<Rect> = const { Cell::new(ZERO_RECT) };
    static VOLUME: Cell<Rect> = const { Cell::new(ZERO_RECT) };
    static APPEARANCE: Cell<Rect> = const { Cell::new(ZERO_RECT) };
    static POWER_PROFILE: Cell<Rect> = const { Cell::new(ZERO_RECT) };
    static LOCK: Cell<Rect> = const { Cell::new(ZERO_RECT) };
    static POWER_OFF: Cell<Rect> = const { Cell::new(ZERO_RECT) };
}

pub struct QuickSettingsState<'a> {
    pub network: &'a NetworkState,
    pub audio: &'a AudioSnapshot,
    pub battery: &'a BatterySnapshot,
    pub power_profile: Option<PowerProfile>,
    pub theme_name: &'a str,
    pub power_armed: bool,
}

pub fn draw(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    state: QuickSettingsState<'_>,
) {
    let q = QuickSettings::DEFAULT;
    draw_card_body(painter, theme);
    text(
        painter,
        font,
        theme,
        "Schnelleinstellungen",
        q.outer_pad,
        31,
        false,
    );
    text(painter, font, theme, "System", q.outer_pad, 47, true);

    let settings = Rect {
        x: q.width - q.outer_pad - 96,
        y: 12,
        w: 96,
        h: 34,
    };
    draw_control(painter, font, theme, settings, "Einstellungen", false);
    SETTINGS.with(|slot| slot.set(settings));

    let content_w = q.width - q.outer_pad * 2;
    let tile_w = (content_w - q.tile_gap) / 2;
    let tile_y = q.header_height + q.section_gap;
    let network = Rect {
        x: q.outer_pad,
        y: tile_y,
        w: tile_w,
        h: q.tile_height,
    };
    let profile = Rect {
        x: network.x + tile_w + q.tile_gap,
        ..network
    };
    draw_network_tile(painter, font, theme, network, state.network);
    draw_profile_tile(painter, font, theme, profile, state.power_profile);
    NETWORK.with(|slot| slot.set(network));
    POWER_PROFILE.with(|slot| slot.set(profile));

    let audio_y = tile_y + q.tile_height + q.section_gap;
    let audio_rect = Rect {
        x: q.outer_pad,
        y: audio_y,
        w: content_w,
        h: q.audio_height,
    };
    draw_panel(painter, theme, audio_rect);
    let muted = state
        .audio
        .default_output
        .as_ref()
        .map(|output| output.muted)
        .unwrap_or(true);
    let mute = Rect {
        x: audio_rect.x + 12,
        y: audio_rect.y + 12,
        w: 42,
        h: 34,
    };
    draw_control(
        painter,
        font,
        theme,
        mute,
        if muted { "MUT" } else { "AUD" },
        muted,
    );
    AUDIO_MUTE.with(|slot| slot.set(mute));
    text(
        painter,
        font,
        theme,
        "Audio",
        audio_rect.x + 66,
        audio_rect.y + 27,
        false,
    );
    let output = state
        .audio
        .default_output
        .as_ref()
        .map(|device| fit(&device.name, 34))
        .unwrap_or_else(|| "Nicht verfügbar".to_string());
    text(
        painter,
        font,
        theme,
        &output,
        audio_rect.x + 66,
        audio_rect.y + 44,
        true,
    );

    let volume = state
        .audio
        .default_output
        .as_ref()
        .and_then(|output| output.volume_percent)
        .unwrap_or(0);
    let slider = Rect {
        x: audio_rect.x + 18,
        y: audio_rect.y + 76,
        w: audio_rect.w - 76,
        h: 28,
    };
    draw_slider(painter, theme, slider, volume, muted);
    VOLUME.with(|slot| slot.set(slider));
    text(
        painter,
        font,
        theme,
        &format!("{volume}%"),
        audio_rect.x + audio_rect.w - 50,
        audio_rect.y + 94,
        false,
    );

    let status_y = audio_y + q.audio_height + q.section_gap;
    let status = Rect {
        x: q.outer_pad,
        y: status_y,
        w: content_w,
        h: q.status_height,
    };
    draw_panel(painter, theme, status);
    let half = status.w / 2;
    let appearance = Rect { w: half, ..status };
    let battery = Rect {
        x: status.x + half,
        w: status.w - half,
        ..status
    };
    draw_status_cell(
        painter,
        font,
        theme,
        appearance,
        "Darstellung",
        if state.theme_name.to_ascii_lowercase().contains("dark") {
            "Dunkel"
        } else {
            "Hell"
        },
    );
    let battery_value = if state.battery.present {
        format!("{}%", state.battery.capacity)
    } else {
        "Netzbetrieb".to_string()
    };
    draw_status_cell(painter, font, theme, battery, "Akku", &battery_value);
    APPEARANCE.with(|slot| slot.set(appearance));

    let footer_y = status_y + q.status_height + q.section_gap;
    let footer_w = (content_w - q.tile_gap) / 2;
    let lock = Rect {
        x: q.outer_pad,
        y: footer_y,
        w: footer_w,
        h: q.footer_height,
    };
    let power = Rect {
        x: lock.x + footer_w + q.tile_gap,
        ..lock
    };
    draw_control(painter, font, theme, lock, "Sperren", false);
    draw_control(
        painter,
        font,
        theme,
        power,
        if state.power_armed {
            "Bestätigen"
        } else {
            "Ausschalten"
        },
        state.power_armed,
    );
    LOCK.with(|slot| slot.set(lock));
    POWER_OFF.with(|slot| slot.set(power));
}

fn draw_network_tile(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    rect: Rect,
    network: &NetworkState,
) {
    draw_panel(painter, theme, rect);
    let (label, detail, active) = match network {
        NetworkState::Connected {
            kind,
            connection_name,
        } => {
            let label = match kind {
                ConnectionKind::Ethernet => "Ethernet",
                ConnectionKind::Wifi { .. } => "WLAN",
                ConnectionKind::Vpn => "VPN",
                ConnectionKind::Other => "Netzwerk",
            };
            (label, fit(connection_name, 19), true)
        }
        NetworkState::Disconnected => ("Netzwerk", "Getrennt".to_string(), false),
        NetworkState::Offline => ("Netzwerk", "Nicht verfügbar".to_string(), false),
    };
    draw_badge(painter, font, theme, rect.x + 12, rect.y + 12, "N", active);
    text(painter, font, theme, label, rect.x + 52, rect.y + 31, false);
    text(
        painter,
        font,
        theme,
        &detail,
        rect.x + 52,
        rect.y + 50,
        true,
    );
}

fn draw_profile_tile(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    rect: Rect,
    profile: Option<PowerProfile>,
) {
    draw_panel(painter, theme, rect);
    draw_badge(painter, font, theme, rect.x + 12, rect.y + 12, "E", false);
    text(
        painter,
        font,
        theme,
        "Energie",
        rect.x + 52,
        rect.y + 31,
        false,
    );
    text(
        painter,
        font,
        theme,
        profile.map(PowerProfile::label).unwrap_or("Standard"),
        rect.x + 52,
        rect.y + 50,
        true,
    );
}

fn draw_status_cell(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    rect: Rect,
    label: &str,
    value: &str,
) {
    text(painter, font, theme, label, rect.x + 14, rect.y + 28, false);
    text(painter, font, theme, value, rect.x + 14, rect.y + 49, true);
}

fn draw_badge(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    x: i32,
    y: i32,
    label: &str,
    active: bool,
) {
    let q = QuickSettings::DEFAULT;
    let rect = Rect { x, y, w: 32, h: 32 };
    painter.roundish_rect_with_radius(
        rect,
        if active {
            Interaction::DEFAULT.accent_idle(theme.colors.accent)
        } else {
            Interaction::DEFAULT.neutral_hover
        },
        q.control_radius,
    );
    text(painter, font, theme, label, x + 11, y + 21, false);
}

fn draw_panel(painter: &mut Painter<'_>, theme: &ThemeConfig, rect: Rect) {
    painter.roundish_rect_with_radius(rect, Interaction::DEFAULT.neutral_hover, Radius::DEFAULT.lg);
    let border = glass_border_from_config(theme);
    painter.rect(
        Rect {
            x: rect.x + Radius::DEFAULT.lg,
            y: rect.y,
            w: rect.w - Radius::DEFAULT.lg * 2,
            h: 1,
        },
        border,
    );
}

fn draw_control(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    rect: Rect,
    label: &str,
    active: bool,
) {
    painter.roundish_rect_with_radius(
        rect,
        if active {
            Interaction::DEFAULT.accent_idle(theme.colors.accent)
        } else {
            Interaction::DEFAULT.neutral_hover
        },
        QuickSettings::DEFAULT.control_radius,
    );
    let width = measure(font, label);
    text(
        painter,
        font,
        theme,
        label,
        rect.x + (rect.w - width) / 2,
        rect.y + (rect.h + 10) / 2,
        false,
    );
}

fn draw_slider(
    painter: &mut Painter<'_>,
    theme: &ThemeConfig,
    hit_rect: Rect,
    volume: u8,
    muted: bool,
) {
    let q = QuickSettings::DEFAULT;
    let bar = Rect {
        x: hit_rect.x,
        y: hit_rect.y + (hit_rect.h - q.slider_height) / 2,
        w: hit_rect.w,
        h: q.slider_height,
    };
    painter.roundish_rect_with_radius(bar, glass_border_from_config(theme), q.slider_height);
    let fill = Rect {
        w: bar.w * i32::from(volume.min(100)) / 100,
        ..bar
    };
    if fill.w > 0 {
        painter.roundish_rect_with_radius(
            fill,
            if muted {
                glass_dim_from_config(theme)
            } else {
                theme.colors.accent
            },
            q.slider_height,
        );
    }
}

fn text(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    value: &str,
    x: i32,
    baseline: i32,
    dim: bool,
) {
    let color = if dim {
        glass_dim_from_config(theme)
    } else {
        glass_foreground_from_config(theme)
    };
    painter.text_clipped(font, value, x, baseline, painter.width - x - 8, color);
}

fn measure(font: &RefCell<Option<TextRenderer>>, value: &str) -> i32 {
    font.borrow_mut()
        .as_mut()
        .map(|renderer| renderer.measure_text(value))
        .unwrap_or(value.chars().count() as i32 * 8)
}

fn fit(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let mut out: String = value.chars().take(max.saturating_sub(1)).collect();
    out.push('\u{2026}');
    out
}

pub fn hit_test(width: u32, height: u32, x: f64, y: f64) -> Option<QuickSettingsHit> {
    if !(Rect {
        x: 0,
        y: 0,
        w: width as i32,
        h: height as i32,
    })
    .contains(x, y)
    {
        return None;
    }
    for (slot, hit) in [
        (&SETTINGS, QuickSettingsHit::Settings),
        (&NETWORK, QuickSettingsHit::Network),
        (&AUDIO_MUTE, QuickSettingsHit::AudioMute),
        (&APPEARANCE, QuickSettingsHit::Appearance),
        (&POWER_PROFILE, QuickSettingsHit::PowerProfile),
        (&LOCK, QuickSettingsHit::Lock),
        (&POWER_OFF, QuickSettingsHit::PowerOff),
    ] {
        if slot.with(Cell::get).contains(x, y) {
            return Some(hit);
        }
    }
    let volume = VOLUME.with(Cell::get);
    if volume.contains(x, y) {
        let fraction = ((x - f64::from(volume.x)) / f64::from(volume.w)).clamp(0.0, 1.0);
        return Some(QuickSettingsHit::Volume((fraction * 100.0).round() as u8));
    }
    Some(QuickSettingsHit::Card)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outside_is_not_a_hit() {
        assert_eq!(hit_test(384, 468, -1.0, 10.0), None);
    }
}
