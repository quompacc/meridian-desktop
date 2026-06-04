//! Minimal fontconfig family resolver. Maps a font-family name (from
//! `theme.fonts.ui`) to a font file path so the active UI font can follow the
//! theme. No rasterising happens here — glyphs are still rendered by fontdue
//! (`meridian_ui`); this only locates the file. Re-introduced after the
//! FreeType/fontconfig removal in the typography consolidation, because
//! resolving a configured family still needs the system font database.
#![allow(non_camel_case_types, non_upper_case_globals)]

use std::{
    ffi::{CStr, CString},
    path::PathBuf,
    ptr,
};

use libc::{c_char, c_int, c_uchar};
use meridian_config::ThemeConfig;
use tracing::{info, warn};

type FcChar8 = c_uchar;
type FcBool = c_int;
enum FcConfig {}
enum FcPattern {}
type FcResult = c_int;
const FcMatchPattern: c_int = 0;
const FcResultMatch: FcResult = 0;
const FcResultNoMatch: FcResult = 1;

#[link(name = "fontconfig")]
extern "C" {
    fn FcInit() -> FcBool;
    fn FcNameParse(name: *const FcChar8) -> *mut FcPattern;
    fn FcConfigSubstitute(config: *mut FcConfig, pattern: *mut FcPattern, kind: c_int) -> FcBool;
    fn FcDefaultSubstitute(pattern: *mut FcPattern);
    fn FcFontMatch(
        config: *mut FcConfig,
        pattern: *mut FcPattern,
        result: *mut FcResult,
    ) -> *mut FcPattern;
    fn FcPatternGetString(
        pattern: *const FcPattern,
        object: *const c_char,
        n: c_int,
        s: *mut *mut FcChar8,
    ) -> FcResult;
    fn FcPatternDestroy(pattern: *mut FcPattern);
}

fn resolve_family(family: &str) -> Option<PathBuf> {
    // SAFETY: every fontconfig pointer is created and checked here and
    // destroyed on every exit path.
    unsafe {
        if FcInit() == 0 {
            return None;
        }
        let pattern = CString::new(family).ok()?;
        let fc_pattern = FcNameParse(pattern.as_ptr() as *const FcChar8);
        if fc_pattern.is_null() {
            return None;
        }
        FcConfigSubstitute(ptr::null_mut(), fc_pattern, FcMatchPattern);
        FcDefaultSubstitute(fc_pattern);
        let mut result = FcResultNoMatch;
        let match_pattern = FcFontMatch(ptr::null_mut(), fc_pattern, &mut result);
        FcPatternDestroy(fc_pattern);
        if match_pattern.is_null() || result != FcResultMatch {
            if !match_pattern.is_null() {
                FcPatternDestroy(match_pattern);
            }
            return None;
        }
        let mut file: *mut FcChar8 = ptr::null_mut();
        let key = CString::new("file").ok()?;
        let get_result = FcPatternGetString(match_pattern, key.as_ptr(), 0, &mut file);
        let path = if get_result == FcResultMatch && !file.is_null() {
            CStr::from_ptr(file as *const c_char)
                .to_str()
                .ok()
                .map(PathBuf::from)
        } else {
            None
        };
        FcPatternDestroy(match_pattern);
        path
    }
}

/// Apply `theme.fonts.ui` as the active UI font. The default family
/// ("Adwaita Sans") and an empty family keep the embedded font; any other
/// installed family is resolved via fontconfig and loaded. On any failure the
/// embedded font is kept. The point size in the string is not used (the render
/// scale is fixed for now).
pub(crate) fn apply_theme_ui_font(theme: &ThemeConfig) {
    let family = theme.fonts.ui_family();
    if family.is_empty() || family == "Adwaita Sans" {
        meridian_ui::clear_ui_font();
        return;
    }
    match resolve_family(family) {
        Some(path) => match std::fs::read(&path) {
            Ok(bytes) if meridian_ui::set_ui_font(&bytes) => {
                info!("UI font set from family {:?} ({})", family, path.display());
            }
            Ok(_) => {
                warn!(
                    "UI font {:?} ({}) did not parse; keeping embedded",
                    family,
                    path.display()
                );
                meridian_ui::clear_ui_font();
            }
            Err(err) => {
                warn!(
                    "UI font {:?} ({}) read failed: {}; keeping embedded",
                    family,
                    path.display(),
                    err
                );
                meridian_ui::clear_ui_font();
            }
        },
        None => {
            warn!(
                "UI font family {:?} not found via fontconfig; keeping embedded",
                family
            );
            meridian_ui::clear_ui_font();
        }
    }
}
