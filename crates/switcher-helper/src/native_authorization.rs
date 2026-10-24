//! Authorization Services boundary. External forms are secrets: never persist or log them.
//! Uses the SDK Authorization.h ABI and the built-in, non-shared administrator right.
use std::{ffi::c_void, ptr};

#[repr(C)]
struct Item {
    name: *const std::ffi::c_char,
    length: usize,
    value: *mut c_void,
    flags: u32,
}
#[repr(C)]
struct Rights {
    count: u32,
    items: *mut Item,
}
#[repr(C)]
struct ExternalForm {
    bytes: [u8; 32],
}
#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    fn AuthorizationCreate(
        rights: *const Rights,
        environment: *const Rights,
        flags: u32,
        auth: *mut *mut c_void,
    ) -> i32;
    fn AuthorizationCopyRights(
        auth: *mut c_void,
        rights: *const Rights,
        environment: *const Rights,
        flags: u32,
        authorized: *mut *mut Rights,
    ) -> i32;
    fn AuthorizationMakeExternalForm(auth: *mut c_void, form: *mut ExternalForm) -> i32;
    fn AuthorizationCreateFromExternalForm(
        form: *const ExternalForm,
        auth: *mut *mut c_void,
    ) -> i32;
    fn AuthorizationFree(auth: *mut c_void, flags: u32) -> i32;
}
fn admin_right() -> Item {
    Item {
        name: c"system.privilege.admin".as_ptr(),
        length: 0,
        value: ptr::null_mut(),
        flags: 0,
    }
}
/// A fresh reference per operation. Never cache this object between repairs.
pub struct Authorization(*mut c_void);
impl Drop for Authorization {
    fn drop(&mut self) {
        // Destroy shared external-form rights as well as our reference, including error paths.
        unsafe {
            AuthorizationFree(self.0, 1 << 3);
        }
    }
}
impl Authorization {
    pub fn acquire() -> Result<Self, &'static str> {
        let mut auth = ptr::null_mut();
        let status = unsafe { AuthorizationCreate(ptr::null(), ptr::null(), 0, &mut auth) };
        if status != 0 || auth.is_null() {
            return Err("authorization_unavailable");
        }
        let owned = Self(auth);
        let mut item = admin_right();
        let rights = Rights {
            count: 1,
            items: &mut item,
        };
        let status =
            unsafe { AuthorizationCopyRights(auth, &rights, ptr::null(), 1 | 2, ptr::null_mut()) };
        match status {
            0 => Ok(owned),
            -60006 => Err("authorization_cancelled"),
            _ => Err("authorization_denied"),
        }
    }
    pub fn external_form(&self) -> Result<Vec<u8>, &'static str> {
        let mut form = ExternalForm { bytes: [0; 32] };
        if unsafe { AuthorizationMakeExternalForm(self.0, &mut form) } != 0 {
            return Err("authorization_external_form");
        }
        Ok(form.bytes.to_vec())
    }
}
/// Root must only inspect granted rights: never extend rights or allow UI here.
/// Destroying the imported reference consumes this native credential, preventing replay.
pub fn consume(form: &[u8]) -> Result<(), &'static str> {
    let bytes: [u8; 32] = form.try_into().map_err(|_| "authorization_invalid")?;
    let mut auth = ptr::null_mut();
    if unsafe { AuthorizationCreateFromExternalForm(&ExternalForm { bytes }, &mut auth) } != 0
        || auth.is_null()
    {
        return Err("authorization_invalid");
    }
    let _owned = Authorization(auth);
    let mut item = admin_right();
    let rights = Rights {
        count: 1,
        items: &mut item,
    };
    if unsafe { AuthorizationCopyRights(auth, &rights, ptr::null(), 0, ptr::null_mut()) } != 0 {
        return Err("authorization_denied");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_and_forged_external_forms_are_rejected_without_interaction() {
        assert_eq!(consume(&[]), Err("authorization_invalid"));
        assert_eq!(consume(&[0; 31]), Err("authorization_invalid"));
        // The real Security.framework import rejects a synthetic token. No acquisition/UI API.
        assert_eq!(consume(&[0; 32]), Err("authorization_invalid"));
    }
}
