use meridian_ui::{
    style::Palette,
    widget::{Button, Widget},
};

use crate::icons::{icon_image_to_pixmap, IconCache};

struct PowerButtonSpec {
    id: &'static str,
    label: &'static str,
    icon_name: &'static str,
    color: fn(&Palette) -> meridian_ui::style::Color,
}

const POWER_BUTTONS: &[PowerButtonSpec] = &[
    PowerButtonSpec {
        id: "power-off",
        label: "Aus",
        icon_name: "system-shutdown",
        color: |pal| pal.error,
    },
    PowerButtonSpec {
        id: "power-restart",
        label: "Neu",
        icon_name: "system-reboot",
        color: |pal| pal.warning,
    },
    PowerButtonSpec {
        id: "power-sleep",
        label: "Zzz",
        icon_name: "system-suspend",
        color: |pal| pal.accent,
    },
    PowerButtonSpec {
        id: "power-lock",
        label: "Lock",
        icon_name: "system-lock-screen",
        color: |pal| pal.accent_alt,
    },
    PowerButtonSpec {
        id: "power-logout",
        label: "Out",
        icon_name: "system-log-out",
        color: |pal| pal.success,
    },
];

pub(crate) fn build_power_footer_buttons(
    icon_cache: &IconCache,
    palette: &Palette,
    button_size: i32,
    icon_size: u32,
    armed_power: Option<(&str, f32)>,
) -> Vec<Box<dyn Widget>> {
    POWER_BUTTONS
        .iter()
        .map(|spec| {
            let icon = icon_cache
                .lookup(spec.icon_name, icon_size)
                .and_then(icon_image_to_pixmap);
            let armed_progress =
                armed_power.and_then(|(id, progress)| (id == spec.id).then_some(progress));

            Box::new(
                Button::with_id_and_icon(
                    spec.id,
                    spec.label,
                    (spec.color)(palette),
                    button_size,
                    button_size,
                    icon,
                )
                .with_armed_progress(armed_progress)
                .with_armed_label("OK?"),
            ) as Box<dyn Widget>
        })
        .collect()
}
