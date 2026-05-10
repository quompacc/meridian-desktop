use std::{fs::File, io::Read};

use tracing::info;
use xcursor::{
    parser::{parse_xcursor, Image},
    CursorTheme,
};

use super::CursorImage;

fn build_xcursor_path() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    [
        format!("{}/.local/share/icons", home),
        "/usr/share/icons".to_string(),
        "/usr/share/cursors/xorg-x11".to_string(),
    ]
    .join(":")
}

pub(super) fn load_xcursor(theme_name: &str, requested_size: u32) -> Result<CursorImage, String> {
    if std::env::var_os("XCURSOR_PATH").is_none() {
        std::env::set_var("XCURSOR_PATH", build_xcursor_path());
    }
    let theme = CursorTheme::load(theme_name);

    const CURSOR_NAMES: &[&str] = &["left_ptr", "default", "arrow"];
    let (cursor_name, icon_path) = CURSOR_NAMES
        .iter()
        .find_map(|name| theme.load_icon(name).map(|path| (*name, path)))
        .ok_or_else(|| {
            format!(
                "theme has none of the cursor icons: {}",
                CURSOR_NAMES.join(", ")
            )
        })?;

    let mut file =
        File::open(&icon_path).map_err(|e| format!("failed to open {:?}: {}", icon_path, e))?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)
        .map_err(|e| format!("failed to read {:?}: {}", icon_path, e))?;

    let images = parse_xcursor(&data).ok_or_else(|| "failed to parse xcursor file".to_string())?;
    let image = nearest_image(requested_size, &images)
        .ok_or_else(|| "xcursor file contains no images".to_string())?;

    info!(
        "Loaded xcursor theme={} icon={} size={} image={}x{} hotspot={},{} path={:?}",
        theme_name,
        cursor_name,
        requested_size,
        image.width,
        image.height,
        image.xhot,
        image.yhot,
        icon_path
    );

    Ok(CursorImage {
        theme: theme_name.to_string(),
        name: cursor_name.to_string(),
        width: image.width,
        height: image.height,
        xhot: image.xhot,
        yhot: image.yhot,
        pixels_rgba: image.pixels_rgba.clone(),
    })
}

fn nearest_image(size: u32, images: &[Image]) -> Option<&Image> {
    images
        .iter()
        .min_by_key(|img| (size as i32 - img.size as i32).abs())
}
