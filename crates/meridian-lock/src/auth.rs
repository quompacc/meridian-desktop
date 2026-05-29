use std::ffi::CString;
use std::os::raw::{c_int, c_void};
use std::ptr;

use libc::{calloc, free, size_t, strdup};
use pam_sys::{
    pam_authenticate, pam_conv, pam_end, pam_handle_t, pam_message, pam_response, pam_start,
    PAM_BUF_ERR, PAM_CONV_ERR, PAM_ERROR_MSG, PAM_PROMPT_ECHO_OFF, PAM_PROMPT_ECHO_ON, PAM_SUCCESS,
    PAM_TEXT_INFO,
};
use zeroize::Zeroizing;

const PAM_SERVICE: &str = "meridian-lock";

struct ConvData {
    user: CString,
    pass: CString,
}

impl Drop for ConvData {
    fn drop(&mut self) {
        // Zeroize our password copy
        let pass = std::mem::replace(&mut self.pass, CString::new("x").unwrap());
        let raw = pass.into_raw();
        let len = unsafe { libc::strlen(raw) } + 1;
        for i in 0..len {
            unsafe { std::ptr::write_volatile(raw.add(i) as *mut u8, 0) };
        }
        unsafe { drop(CString::from_raw(raw)) };
    }
}

unsafe extern "C" fn conv_cb(
    num_msg: c_int,
    msg: *mut *const pam_message,
    out_resp: *mut *mut pam_response,
    appdata: *mut c_void,
) -> c_int {
    if num_msg <= 0 || appdata.is_null() || msg.is_null() || out_resp.is_null() {
        return PAM_CONV_ERR;
    }
    let resp = calloc(
        num_msg as usize,
        std::mem::size_of::<pam_response>() as size_t,
    ) as *mut pam_response;
    if resp.is_null() {
        return PAM_BUF_ERR;
    }
    let data = &*(appdata as *const ConvData);
    for i in 0..num_msg as isize {
        let m_ptr = *msg.offset(i);
        if m_ptr.is_null() {
            free(resp as *mut c_void);
            return PAM_CONV_ERR;
        }
        let m = &*m_ptr;
        let r = &mut *resp.offset(i);
        match m.msg_style {
            x if x == PAM_PROMPT_ECHO_ON => {
                r.resp = strdup(data.user.as_ptr());
            }
            x if x == PAM_PROMPT_ECHO_OFF => {
                r.resp = strdup(data.pass.as_ptr());
            }
            x if x == PAM_TEXT_INFO || x == PAM_ERROR_MSG => {}
            _ => {
                free(resp as *mut c_void);
                return PAM_CONV_ERR;
            }
        }
    }
    *out_resp = resp;
    PAM_SUCCESS
}

struct PamGuard {
    pamh: *mut pam_handle_t,
    status: c_int,
}

impl Drop for PamGuard {
    fn drop(&mut self) {
        if !self.pamh.is_null() {
            unsafe { pam_end(self.pamh, self.status) };
            self.pamh = ptr::null_mut();
        }
    }
}

/// Authenticate `username` with `password` against the system PAM service.
/// Returns `true` on success. Blocking.
pub fn authenticate(username: &str, password: &Zeroizing<String>) -> bool {
    if username.is_empty() {
        return false;
    }
    let user_c = match CString::new(username) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let pass_c = match CString::new(password.as_str()) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let service_c = CString::new(PAM_SERVICE).unwrap();

    let conv_data = Box::new(ConvData {
        user: user_c.clone(),
        pass: pass_c,
    });
    let conv_data_ptr = Box::into_raw(conv_data);

    let conversation = pam_conv {
        conv: Some(conv_cb),
        appdata_ptr: conv_data_ptr as *mut c_void,
    };

    let mut pamh: *mut pam_handle_t = ptr::null_mut();
    let rc = unsafe {
        pam_start(
            service_c.as_ptr(),
            user_c.as_ptr(),
            &conversation,
            &mut pamh,
        )
    };
    if rc != PAM_SUCCESS || pamh.is_null() {
        tracing::warn!("pam_start failed: rc={}", rc);
        unsafe { drop(Box::from_raw(conv_data_ptr)) };
        return false;
    }

    let mut guard = PamGuard {
        pamh,
        status: PAM_SUCCESS,
    };
    let rc = unsafe { pam_authenticate(guard.pamh, 0) };
    guard.status = rc;
    let ok = rc == PAM_SUCCESS;
    if !ok {
        tracing::debug!("pam_authenticate failed: rc={}", rc);
    }
    drop(guard);
    unsafe { drop(Box::from_raw(conv_data_ptr)) };
    ok
}
