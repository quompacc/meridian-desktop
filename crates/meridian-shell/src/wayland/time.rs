use std::{
    ffi::{CStr, CString},
    ptr,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct LocalDate {
    pub(super) year: i32,
    pub(super) month: u8,
    pub(super) day: u8,
}

pub(super) fn local_date() -> Option<LocalDate> {
    unsafe {
        let mut now = libc::time(ptr::null_mut());
        let mut tm = std::mem::zeroed::<libc::tm>();
        if libc::localtime_r(&mut now, &mut tm).is_null() {
            return None;
        }

        let year = tm.tm_year + 1900;
        let month = (tm.tm_mon + 1) as u8;
        let day = tm.tm_mday as u8;
        if !(1..=12).contains(&month) || day == 0 {
            return None;
        }

        Some(LocalDate { year, month, day })
    }
}

pub(super) fn formatted_time() -> String {
    unsafe {
        let mut now = libc::time(ptr::null_mut());
        let mut tm = std::mem::zeroed::<libc::tm>();
        if libc::localtime_r(&mut now, &mut tm).is_null() {
            return String::new();
        }
        let mut out = [0_i8; 64];
        let fmt = CString::new("%H:%M  %d.%m.%Y").expect("valid strftime format");
        let len = libc::strftime(out.as_mut_ptr(), out.len(), fmt.as_ptr(), &tm);
        if len == 0 {
            String::new()
        } else {
            CStr::from_ptr(out.as_ptr()).to_string_lossy().into_owned()
        }
    }
}
