pub(super) fn valid_wallpaper_path(path: &str) -> bool {
    let path = std::path::Path::new(path);
    path.is_absolute()
        && path.as_os_str().len() <= 4096
        && !path.to_string_lossy().chars().any(char::is_control)
        && path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "jpg" | "jpeg" | "png" | "webp"
                )
            })
}

#[cfg(test)]
mod tests {
    use super::valid_wallpaper_path;

    #[test]
    fn wallpaper_path_must_be_absolute_and_use_supported_image_extension() {
        assert!(valid_wallpaper_path("/home/user/Pictures/meridian.png"));
        assert!(valid_wallpaper_path("/tmp/WALLPAPER.JPEG"));
        assert!(!valid_wallpaper_path("Pictures/meridian.png"));
        assert!(!valid_wallpaper_path("/tmp/meridian.svg"));
        assert!(!valid_wallpaper_path("/tmp/bad\nname.png"));
    }
}
