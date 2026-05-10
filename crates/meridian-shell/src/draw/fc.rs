#![allow(non_camel_case_types, non_upper_case_globals)]

use libc::{c_char, c_int, c_uchar, c_void};

pub type FcChar8 = c_uchar;
pub type FcBool = c_int;
pub enum FcConfig {}
pub enum FcPattern {}
pub type FcResult = c_int;

pub const FcMatchPattern: c_int = 0;
pub const FcResultMatch: FcResult = 0;
pub const FcResultNoMatch: FcResult = 1;

#[link(name = "fontconfig")]
extern "C" {
    pub fn FcInit() -> FcBool;
    pub fn FcNameParse(name: *const FcChar8) -> *mut FcPattern;
    pub fn FcConfigSubstitute(
        config: *mut FcConfig,
        pattern: *mut FcPattern,
        kind: c_int,
    ) -> FcBool;
    pub fn FcDefaultSubstitute(pattern: *mut FcPattern);
    pub fn FcFontMatch(
        config: *mut FcConfig,
        pattern: *mut FcPattern,
        result: *mut FcResult,
    ) -> *mut FcPattern;
    pub fn FcPatternGetString(
        pattern: *const FcPattern,
        object: *const c_char,
        n: c_int,
        s: *mut *mut FcChar8,
    ) -> FcResult;
    pub fn FcPatternDestroy(pattern: *mut FcPattern);
}

#[allow(dead_code)]
type _KeepVoid = c_void;
