use std::{
    ffi::{CStr, CString},
    ptr,
};

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
