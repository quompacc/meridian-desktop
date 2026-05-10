use std::{
    ffi::{CStr, CString},
    path::PathBuf,
    ptr,
};

use meridian_config::Color;

use super::{fc, ft, painter::Painter};

pub struct TextRenderer {
    face: ft::Face,
    library: ft::Library,
}

impl TextRenderer {
    pub fn new(pattern: &str, pixels: u32) -> Option<Self> {
        let font_path = fontconfig_match(pattern).or_else(|| fontconfig_match("sans"))?;
        let library = ft::Library::new().ok()?;
        let face = ft::Face::new(&library, &font_path, pixels).ok()?;
        Some(Self { face, library })
    }

    pub fn draw_text(
        &mut self,
        painter: &mut Painter<'_>,
        text: &str,
        x: i32,
        baseline: i32,
        max_w: i32,
        color: Color,
    ) -> bool {
        let mut pen_x = x;
        let end_x = x + max_w;
        let mut drew = false;

        for ch in text.chars() {
            if pen_x >= end_x {
                break;
            }
            let Some(glyph) = self.face.load_char(ch) else {
                continue;
            };

            let draw_x = pen_x + glyph.left;
            let draw_y = baseline - glyph.top;
            for row in 0..glyph.rows {
                for col in 0..glyph.width {
                    let idx = if glyph.pitch >= 0 {
                        (row * glyph.pitch as u32 + col) as usize
                    } else {
                        ((glyph.rows - 1 - row) * (-glyph.pitch) as u32 + col) as usize
                    };
                    let alpha = glyph.buffer.get(idx).copied().unwrap_or(0);
                    painter.blend_pixel(draw_x + col as i32, draw_y + row as i32, color, alpha);
                    drew = drew || alpha != 0;
                }
            }
            pen_x += glyph.advance;
        }

        drew
    }
}

impl Drop for TextRenderer {
    fn drop(&mut self) {
        let _ = &self.library;
    }
}

fn fontconfig_match(pattern: &str) -> Option<PathBuf> {
    unsafe {
        if fc::FcInit() == 0 {
            return None;
        }
        let pattern = CString::new(pattern).ok()?;
        let fc_pattern = fc::FcNameParse(pattern.as_ptr() as *const fc::FcChar8);
        if fc_pattern.is_null() {
            return None;
        }

        fc::FcConfigSubstitute(ptr::null_mut(), fc_pattern, fc::FcMatchPattern);
        fc::FcDefaultSubstitute(fc_pattern);

        let mut result = fc::FcResultNoMatch;
        let match_pattern = fc::FcFontMatch(ptr::null_mut(), fc_pattern, &mut result);
        fc::FcPatternDestroy(fc_pattern);

        if match_pattern.is_null() || result != fc::FcResultMatch {
            if !match_pattern.is_null() {
                fc::FcPatternDestroy(match_pattern);
            }
            return None;
        }

        let mut file: *mut fc::FcChar8 = ptr::null_mut();
        let key = CString::new("file").ok()?;
        let get_result = fc::FcPatternGetString(match_pattern, key.as_ptr(), 0, &mut file);
        let path = if get_result == fc::FcResultMatch && !file.is_null() {
            CStr::from_ptr(file as *const libc::c_char)
                .to_str()
                .ok()
                .map(PathBuf::from)
        } else {
            None
        };
        fc::FcPatternDestroy(match_pattern);
        path
    }
}
