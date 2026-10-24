//! P0: read-only, process-local macOS FFI checks. No service activation or registration.
use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr::{addr_of, null, null_mut};
type Obj = *mut c_void;

#[link(name = "System")]
unsafe extern "C" {
    fn xpc_dictionary_create(keys: *const *const c_char, values: *const Obj, count: usize) -> Obj;
    fn xpc_dictionary_set_string(object: Obj, key: *const c_char, value: *const c_char);
    fn xpc_dictionary_get_string(object: Obj, key: *const c_char) -> *const c_char;
    fn xpc_connection_create_mach_service(name: *const c_char, queue: Obj, flags: u64) -> Obj;
    fn xpc_connection_set_peer_code_signing_requirement(
        connection: Obj,
        requirement: *const c_char,
    ) -> c_int;
    fn xpc_release(object: Obj);
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithCString(allocator: Obj, text: *const c_char, encoding: u32) -> Obj;
    fn CFRelease(object: Obj);
}
#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    fn SecCodeCopySelf(flags: u32, result: *mut Obj) -> i32;
    fn SecCodeCheckValidity(code: Obj, flags: u32, requirement: Obj) -> i32;
    fn SecCodeCreateWithXPCMessage(message: Obj, flags: u32, result: *mut Obj) -> i32;
    fn SecRequirementCreateWithString(text: Obj, flags: u32, result: *mut Obj) -> i32;
}
#[link(name = "ServiceManagement", kind = "framework")]
unsafe extern "C" {
    #[link_name = "OBJC_CLASS_$_SMAppService"]
    static SM_APP_SERVICE_CLASS: u8;
}
#[link(name = "Foundation", kind = "framework")]
unsafe extern "C" {
    #[link_name = "OBJC_CLASS_$_NSXPCConnection"]
    static NS_XPC_CONNECTION_CLASS: u8;
}
#[link(name = "objc")]
unsafe extern "C" {
    fn sel_registerName(name: *const c_char) -> Obj;
    fn class_getClassMethod(class: *const c_void, selector: Obj) -> Obj;
    fn class_getInstanceMethod(class: *const c_void, selector: Obj) -> Obj;
}

fn main() {
    unsafe {
        let dict = xpc_dictionary_create(null(), null(), 0);
        assert!(!dict.is_null());
        xpc_dictionary_set_string(dict, c"probe".as_ptr(), c"p0-local-only".as_ptr());
        assert_eq!(
            CStr::from_ptr(xpc_dictionary_get_string(dict, c"probe".as_ptr())),
            c"p0-local-only"
        );
        println!("xpc_dictionary_roundtrip=PASS");

        // Keep these never-activated objects until process exit. No launchd connection is resumed.
        for (label, requirement, expected_success) in [
            ("valid", c"anchor apple", true),
            ("invalid", c"not a requirement", false),
        ] {
            let connection = xpc_connection_create_mach_service(
                c"org.local.p0.unregistered".as_ptr(),
                null_mut(),
                0,
            );
            assert!(!connection.is_null());
            let rc =
                xpc_connection_set_peer_code_signing_requirement(connection, requirement.as_ptr());
            println!("xpc_requirement_{label}_rc={rc}");
            assert_eq!(rc == 0, expected_success);
        }
        let mut identity = null_mut();
        let synthetic_rc = SecCodeCreateWithXPCMessage(dict, 0, &mut identity);
        println!("synthetic_xpc_identity_rc={synthetic_rc}");
        assert_ne!(synthetic_rc, 0);
        assert!(identity.is_null());
        xpc_release(dict);

        let mut code = null_mut();
        assert_eq!(SecCodeCopySelf(0, &mut code), 0);
        let self_rc = SecCodeCheckValidity(code, 0, null_mut());
        println!("adhoc_self_validity_rc={self_rc}");
        assert_eq!(self_rc, 0);
        let text = CFStringCreateWithCString(null_mut(), c"anchor apple".as_ptr(), 0x08000100);
        let mut requirement = null_mut();
        assert_eq!(SecRequirementCreateWithString(text, 0, &mut requirement), 0);
        let anchor_rc = SecCodeCheckValidity(code, 0, requirement);
        println!("adhoc_rejected_by_apple_anchor_rc={anchor_rc}");
        assert_ne!(anchor_rc, 0);
        CFRelease(requirement);
        CFRelease(text);
        CFRelease(code);

        let sm_class = addr_of!(SM_APP_SERVICE_CLASS).cast();
        let main_method =
            class_getClassMethod(sm_class, sel_registerName(c"mainAppService".as_ptr()));
        let status_method = class_getInstanceMethod(sm_class, sel_registerName(c"status".as_ptr()));
        assert!(!main_method.is_null() && !status_method.is_null());
        println!("smappservice_readonly_selectors=PASS");
        let connection_class = addr_of!(NS_XPC_CONNECTION_CLASS).cast();
        assert!(
            !class_getInstanceMethod(
                connection_class,
                sel_registerName(c"setCodeSigningRequirement:".as_ptr())
            )
            .is_null()
        );
        println!("nsxpc_signing_requirement_selector=PASS");
        println!(
            "services_activated=0 services_registered=0 privileged_actions=0 keychain_reads=0"
        );
    }
}
