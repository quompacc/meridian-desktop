use std::{
    ffi::{c_char, c_int, c_long, c_uchar, c_uint, c_ulong, c_ushort, c_void},
    ptr,
};

const FT_LOAD_RENDER: c_int = 0x4;
const FT_PIXEL_MODE_GRAY: c_uchar = 2;

type FtError = c_int;
type FtLibrary = *mut c_void;
type FtFace = *mut FtFaceRec;
type FtGlyphSlot = *mut FtGlyphSlotRec;

#[repr(C)]
struct FtGeneric {
    data: *mut c_void,
    finalizer: Option<unsafe extern "C" fn(*mut c_void)>,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FtVector {
    x: c_long,
    y: c_long,
}

#[repr(C)]
struct FtBbox {
    x_min: c_long,
    y_min: c_long,
    x_max: c_long,
    y_max: c_long,
}

#[repr(C)]
struct FtBitmap {
    rows: c_uint,
    width: c_uint,
    pitch: c_int,
    buffer: *mut c_uchar,
    num_grays: c_ushort,
    pixel_mode: c_uchar,
    palette_mode: c_uchar,
    palette: *mut c_void,
}

#[repr(C)]
struct FtGlyphMetrics {
    width: c_long,
    height: c_long,
    hori_bearing_x: c_long,
    hori_bearing_y: c_long,
    hori_advance: c_long,
    vert_bearing_x: c_long,
    vert_bearing_y: c_long,
    vert_advance: c_long,
}

#[repr(C)]
struct FtGlyphSlotRec {
    library: FtLibrary,
    face: FtFace,
    next: FtGlyphSlot,
    glyph_index: c_uint,
    generic: FtGeneric,
    metrics: FtGlyphMetrics,
    linear_hori_advance: c_long,
    linear_vert_advance: c_long,
    advance: FtVector,
    format: c_uint,
    bitmap: FtBitmap,
    bitmap_left: c_int,
    bitmap_top: c_int,
}

#[repr(C)]
struct FtFaceRec {
    num_faces: c_long,
    face_index: c_long,
    face_flags: c_long,
    style_flags: c_long,
    num_glyphs: c_long,
    family_name: *const c_char,
    style_name: *const c_char,
    num_fixed_sizes: c_int,
    available_sizes: *mut c_void,
    num_charmaps: c_int,
    charmaps: *mut c_void,
    generic: FtGeneric,
    bbox: FtBbox,
    units_per_em: c_ushort,
    ascender: c_ushort,
    descender: c_ushort,
    height: c_ushort,
    max_advance_width: c_ushort,
    max_advance_height: c_ushort,
    underline_position: c_ushort,
    underline_thickness: c_ushort,
    glyph: FtGlyphSlot,
}

#[link(name = "freetype")]
extern "C" {
    fn FT_Init_FreeType(alibrary: *mut FtLibrary) -> FtError;
    fn FT_Done_FreeType(library: FtLibrary) -> FtError;
    fn FT_New_Memory_Face(
        library: FtLibrary,
        file_base: *const c_uchar,
        file_size: c_long,
        face_index: c_long,
        aface: *mut FtFace,
    ) -> FtError;
    fn FT_Done_Face(face: FtFace) -> FtError;
    fn FT_Set_Pixel_Sizes(face: FtFace, pixel_width: c_uint, pixel_height: c_uint) -> FtError;
    fn FT_Load_Char(face: FtFace, char_code: c_ulong, load_flags: c_int) -> FtError;
}

pub struct Glyph {
    pub width: usize,
    pub height: usize,
    pub left: i32,
    pub top: i32,
    pub advance_x: f32,
    pub bitmap: Vec<u8>,
}

pub struct Font {
    library: FtLibrary,
    face: FtFace,
    _bytes: &'static [u8],
}

unsafe impl Send for Font {}

impl Font {
    pub fn from_static_bytes(bytes: &'static [u8]) -> Option<Self> {
        let mut library: FtLibrary = ptr::null_mut();
        if unsafe { FT_Init_FreeType(&mut library) } != 0 || library.is_null() {
            return None;
        }
        let mut face: FtFace = ptr::null_mut();
        let file_size: c_long = bytes.len().try_into().ok()?;
        let err = unsafe { FT_New_Memory_Face(library, bytes.as_ptr(), file_size, 0, &mut face) };
        if err != 0 || face.is_null() {
            unsafe {
                FT_Done_FreeType(library);
            }
            return None;
        }
        Some(Self {
            library,
            face,
            _bytes: bytes,
        })
    }

    fn set_size(&mut self, size_px: f32) -> bool {
        let px = size_px.round().clamp(1.0, 512.0) as c_uint;
        unsafe { FT_Set_Pixel_Sizes(self.face, 0, px) == 0 }
    }

    pub fn rasterize(&mut self, ch: char, size_px: f32) -> Option<Glyph> {
        if !self.set_size(size_px) {
            return None;
        }
        if unsafe { FT_Load_Char(self.face, ch as c_ulong, FT_LOAD_RENDER) } != 0 {
            return None;
        }
        let slot = unsafe { (*self.face).glyph };
        if slot.is_null() {
            return None;
        }
        let slot_ref = unsafe { &*slot };
        let bitmap = &slot_ref.bitmap;
        let width = bitmap.width as usize;
        let height = bitmap.rows as usize;
        let mut pixels = vec![0u8; width.saturating_mul(height)];
        if width > 0
            && height > 0
            && !bitmap.buffer.is_null()
            && bitmap.pixel_mode == FT_PIXEL_MODE_GRAY
        {
            let pitch = bitmap.pitch;
            let abs_pitch = pitch.unsigned_abs() as usize;
            for row in 0..height {
                let src_row = if pitch >= 0 { row } else { height - 1 - row };
                let src = unsafe { bitmap.buffer.add(src_row * abs_pitch) };
                let dst = row * width;
                unsafe {
                    ptr::copy_nonoverlapping(src, pixels[dst..].as_mut_ptr(), width);
                }
            }
        }
        Some(Glyph {
            width,
            height,
            left: slot_ref.bitmap_left,
            top: slot_ref.bitmap_top,
            advance_x: slot_ref.advance.x as f32 / 64.0,
            bitmap: pixels,
        })
    }

    pub fn measure_text(&mut self, text: &str, size_px: f32) -> Option<(i32, i32)> {
        let mut width = 0.0f32;
        let mut above = 0i32;
        let mut below = 0i32;
        for ch in text.chars() {
            let glyph = self.rasterize(ch, size_px)?;
            width += glyph.advance_x;
            above = above.max(glyph.top.max(0));
            below = below.max((glyph.height as i32 - glyph.top).max(0));
        }
        Some((width.round() as i32, above + below))
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        unsafe {
            FT_Done_Face(self.face);
            FT_Done_FreeType(self.library);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Font;

    #[test]
    fn freetype_rasterizes_embedded_font() {
        let bytes = meridian_tokens::font::ADWAITA_SANS_REGULAR;
        let Some(mut font) = Font::from_static_bytes(bytes) else {
            panic!("FreeType failed to load embedded font");
        };
        let glyph = font.rasterize('H', 14.0).expect("glyph");
        assert!(glyph.width > 0);
        assert!(glyph.height > 0);
        assert!(glyph.bitmap.iter().any(|a| *a > 0));
    }
}
