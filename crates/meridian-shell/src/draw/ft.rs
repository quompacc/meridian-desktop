#![allow(non_camel_case_types, non_snake_case)]

use std::{
    ffi::CString,
    os::raw::{c_char, c_int, c_long, c_uint, c_ulong, c_void},
    path::Path,
    ptr, slice,
};

pub struct Library(FT_Library);
pub struct Face(FT_Face);

pub struct GlyphBitmap {
    pub buffer: Vec<u8>,
    pub width: u32,
    pub rows: u32,
    pub pitch: i32,
    pub left: i32,
    pub top: i32,
    pub advance: i32,
}

impl Library {
    pub fn new() -> Result<Self, c_int> {
        let mut library = ptr::null_mut();
        // SAFETY: `library` is a valid out-pointer; FreeType initializes it when this call succeeds.
        let err = unsafe { FT_Init_FreeType(&mut library) };
        if err == 0 {
            Ok(Self(library))
        } else {
            Err(err)
        }
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        // SAFETY: `self.0` was returned by `FT_Init_FreeType` and is dropped exactly once here.
        unsafe {
            FT_Done_FreeType(self.0);
        }
    }
}

impl Face {
    pub fn new(library: &Library, path: &Path, pixels: u32) -> Result<Self, c_int> {
        let path = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| -1)?;
        let mut face = ptr::null_mut();
        // SAFETY: `library.0` is a live FreeType library and all pointers are valid for this call.
        let err = unsafe { FT_New_Face(library.0, path.as_ptr(), 0, &mut face) };
        if err != 0 {
            return Err(err);
        }

        // SAFETY: `face` is initialized by `FT_New_Face` on success and is valid here.
        let err = unsafe { FT_Set_Pixel_Sizes(face, 0, pixels) };
        if err != 0 {
            // SAFETY: `face` was created by `FT_New_Face`; freeing it on error prevents leaks.
            unsafe {
                FT_Done_Face(face);
            }
            return Err(err);
        }

        Ok(Self(face))
    }

    pub fn load_char(&mut self, ch: char) -> Option<GlyphBitmap> {
        // SAFETY: `self.0` is a valid face; FreeType handles the provided character code.
        let err = unsafe { FT_Load_Char(self.0, ch as c_ulong, FT_LOAD_RENDER) };
        if err != 0 {
            return None;
        }

        // SAFETY: after successful `FT_Load_Char`, `self.0` points to a valid glyph slot and bitmap data.
        unsafe {
            let slot = (*self.0).glyph;
            if slot.is_null() {
                return None;
            }
            let bitmap = &(*slot).bitmap;
            let len = bitmap.rows as usize * bitmap.pitch.unsigned_abs() as usize;
            let buffer = if bitmap.buffer.is_null() || len == 0 {
                Vec::new()
            } else {
                slice::from_raw_parts(bitmap.buffer, len).to_vec()
            };

            Some(GlyphBitmap {
                buffer,
                width: bitmap.width,
                rows: bitmap.rows,
                pitch: bitmap.pitch,
                left: (*slot).bitmap_left,
                top: (*slot).bitmap_top,
                advance: ((*slot).advance.x >> 6) as i32,
            })
        }
    }
}

impl Drop for Face {
    fn drop(&mut self) {
        // SAFETY: `self.0` was created by `FT_New_Face` and is released once during drop.
        unsafe {
            FT_Done_Face(self.0);
        }
    }
}

const FT_LOAD_RENDER: c_int = 4;

type FT_Library = *mut c_void;
type FT_Face = *mut FT_FaceRec;
type FT_GlyphSlot = *mut FT_GlyphSlotRec;
type FT_Size = *mut c_void;
type FT_CharMap = *mut c_void;
type FT_Driver = *mut c_void;
type FT_Memory = *mut c_void;
type FT_Stream = *mut c_void;
type FT_Face_Internal = *mut c_void;
type FT_SubGlyph = *mut c_void;
type FT_Slot_Internal = *mut c_void;
type FT_Pos = c_long;
type FT_Fixed = c_long;

#[repr(C)]
struct FT_Generic {
    data: *mut c_void,
    finalizer: *mut c_void,
}

#[repr(C)]
struct FT_BBox {
    x_min: FT_Pos,
    y_min: FT_Pos,
    x_max: FT_Pos,
    y_max: FT_Pos,
}

#[repr(C)]
struct FT_Vector {
    x: FT_Pos,
    y: FT_Pos,
}

#[repr(C)]
struct FT_Bitmap {
    rows: c_uint,
    width: c_uint,
    pitch: c_int,
    buffer: *mut u8,
    num_grays: u16,
    pixel_mode: u8,
    palette_mode: u8,
    palette: *mut c_void,
}

#[repr(C)]
struct FT_Glyph_Metrics {
    width: FT_Pos,
    height: FT_Pos,
    hori_bearing_x: FT_Pos,
    hori_bearing_y: FT_Pos,
    hori_advance: FT_Pos,
    vert_bearing_x: FT_Pos,
    vert_bearing_y: FT_Pos,
    vert_advance: FT_Pos,
}

#[repr(C)]
struct FT_Bitmap_Size {
    height: i16,
    width: i16,
    size: FT_Pos,
    x_ppem: FT_Pos,
    y_ppem: FT_Pos,
}

#[repr(C)]
struct FT_ListRec {
    head: *mut c_void,
    tail: *mut c_void,
}

#[repr(C)]
struct FT_Outline {
    n_contours: i16,
    n_points: i16,
    points: *mut FT_Vector,
    tags: *mut c_char,
    contours: *mut i16,
    flags: c_int,
}

#[repr(C)]
struct FT_GlyphSlotRec {
    library: FT_Library,
    face: FT_Face,
    next: FT_GlyphSlot,
    glyph_index: c_uint,
    generic: FT_Generic,
    metrics: FT_Glyph_Metrics,
    linear_hori_advance: FT_Fixed,
    linear_vert_advance: FT_Fixed,
    advance: FT_Vector,
    format: c_uint,
    bitmap: FT_Bitmap,
    bitmap_left: c_int,
    bitmap_top: c_int,
    outline: FT_Outline,
    num_subglyphs: c_uint,
    subglyphs: FT_SubGlyph,
    control_data: *mut c_void,
    control_len: c_long,
    lsb_delta: FT_Pos,
    rsb_delta: FT_Pos,
    other: *mut c_void,
    internal: FT_Slot_Internal,
}

#[repr(C)]
struct FT_FaceRec {
    num_faces: c_long,
    face_index: c_long,
    face_flags: c_long,
    style_flags: c_long,
    num_glyphs: c_long,
    family_name: *mut c_char,
    style_name: *mut c_char,
    num_fixed_sizes: c_int,
    available_sizes: *mut FT_Bitmap_Size,
    num_charmaps: c_int,
    charmaps: *mut FT_CharMap,
    generic: FT_Generic,
    bbox: FT_BBox,
    units_per_em: u16,
    ascender: i16,
    descender: i16,
    height: i16,
    max_advance_width: i16,
    max_advance_height: i16,
    underline_position: i16,
    underline_thickness: i16,
    glyph: FT_GlyphSlot,
    size: FT_Size,
    charmap: FT_CharMap,
    driver: FT_Driver,
    memory: FT_Memory,
    stream: FT_Stream,
    sizes_list: FT_ListRec,
    autohint: FT_Generic,
    extensions: *mut c_void,
    internal: FT_Face_Internal,
}

#[link(name = "freetype")]
extern "C" {
    fn FT_Init_FreeType(alibrary: *mut FT_Library) -> c_int;
    fn FT_Done_FreeType(library: FT_Library) -> c_int;
    fn FT_New_Face(
        library: FT_Library,
        filepathname: *const c_char,
        face_index: c_long,
        aface: *mut FT_Face,
    ) -> c_int;
    fn FT_Done_Face(face: FT_Face) -> c_int;
    fn FT_Set_Pixel_Sizes(face: FT_Face, pixel_width: c_uint, pixel_height: c_uint) -> c_int;
    fn FT_Load_Char(face: FT_Face, char_code: c_ulong, load_flags: c_int) -> c_int;
}
