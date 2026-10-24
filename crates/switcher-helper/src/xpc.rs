//! Native XPC boundary. Every message obtains a fresh SecCode from its audit token.
use crate::{
    platform::Platform,
    policy::{Service, VerifiedPeer},
};
use block2::Block;
use std::{
    ffi::{CString, c_char, c_void},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use switcher_domain::system::*;
type Obj = *mut c_void;
#[link(name = "System")]
unsafe extern "C" {
    fn xpc_connection_create_mach_service(name: *const c_char, queue: Obj, flags: u64) -> Obj;
    fn xpc_connection_set_event_handler(connection: Obj, handler: &Block<dyn Fn(Obj)>);
    fn xpc_connection_activate(connection: Obj);
    fn xpc_connection_cancel(connection: Obj);
    fn xpc_connection_get_euid(connection: Obj) -> u32;
    fn xpc_connection_send_message(connection: Obj, message: Obj);
    fn xpc_dictionary_create_reply(message: Obj) -> Obj;
    fn xpc_dictionary_get_data(
        message: Obj,
        key: *const c_char,
        length: *mut usize,
    ) -> *const c_void;
    fn xpc_dictionary_set_data(
        message: Obj,
        key: *const c_char,
        value: *const c_void,
        length: usize,
    );
    fn xpc_dictionary_create(keys: *const *const c_char, values: *const Obj, count: usize) -> Obj;
    fn xpc_get_type(object: Obj) -> Obj;
    static _xpc_type_connection: u8;
    static _xpc_type_dictionary: u8;
    fn xpc_release(object: Obj);
    fn dispatch_main() -> !;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithCString(allocator: Obj, text: *const c_char, encoding: u32) -> Obj;
    fn CFRelease(object: Obj);
    fn CFGetTypeID(object: Obj) -> usize;
    fn CFDictionaryGetTypeID() -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFDictionaryGetValue(dictionary: Obj, key: Obj) -> Obj;
    fn CFStringGetCString(string: Obj, buffer: *mut c_char, size: isize, encoding: u32) -> bool;
}
#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    fn SecCodeCreateWithXPCMessage(message: Obj, flags: u32, code: *mut Obj) -> i32;
    fn SecCodeCheckValidity(code: Obj, flags: u32, requirement: Obj) -> i32;
    fn SecCodeCopySelf(flags: u32, code: *mut Obj) -> i32;
    fn SecCodeCopySigningInformation(code: Obj, flags: u32, information: *mut Obj) -> i32;
    static kSecCodeInfoPList: Obj;
    fn SecRequirementCreateWithString(text: Obj, flags: u32, requirement: *mut Obj) -> i32;
}
unsafe fn requirement(text: &str) -> Result<Obj, &'static str> {
    let text = CString::new(text).map_err(|_| "signature_requirement")?;
    let string =
        unsafe { CFStringCreateWithCString(std::ptr::null_mut(), text.as_ptr(), 0x08000100) };
    if string.is_null() {
        return Err("signature_requirement");
    }
    let mut requirement = std::ptr::null_mut();
    let status = unsafe { SecRequirementCreateWithString(string, 0, &mut requirement) };
    unsafe { CFRelease(string) };
    if status != 0 || requirement.is_null() {
        Err("signature_requirement")
    } else {
        Ok(requirement)
    }
}
fn version_parts(text: &str) -> Option<[u32; 3]> {
    let parts: Vec<_> = text.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let mut out = [0; 3];
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        out[i] = part.parse().ok()?;
    }
    Some(out)
}
unsafe fn minimum_version(code: Obj, minimum: &str) -> bool {
    let Some(minimum) = version_parts(minimum) else {
        return false;
    };
    unsafe {
        let mut info = std::ptr::null_mut();
        if SecCodeCopySigningInformation(code, 2, &mut info) != 0 || info.is_null() {
            return false;
        }
        let plist = CFDictionaryGetValue(info, kSecCodeInfoPList);
        let key = CFStringCreateWithCString(
            std::ptr::null_mut(),
            c"CFBundleShortVersionString".as_ptr(),
            0x08000100,
        );
        let value = if plist.is_null() || CFGetTypeID(plist) != CFDictionaryGetTypeID() {
            std::ptr::null_mut()
        } else {
            CFDictionaryGetValue(plist, key)
        };
        let mut buffer = [0i8; 64];
        let copied = !value.is_null()
            && CFGetTypeID(value) == CFStringGetTypeID()
            && CFStringGetCString(
                value,
                buffer.as_mut_ptr(),
                buffer.len() as isize,
                0x08000100,
            );
        let actual = if copied {
            std::ffi::CStr::from_ptr(buffer.as_ptr())
                .to_str()
                .ok()
                .and_then(version_parts)
        } else {
            None
        };
        CFRelease(key);
        CFRelease(info);
        actual.is_some_and(|version| version >= minimum)
    }
}
unsafe fn authenticate(
    message: Obj,
    connection: Obj,
    record: &InstallRecord,
    role: ClientRole,
) -> Result<VerifiedPeer, &'static str> {
    let uid = unsafe { xpc_connection_get_euid(connection) };
    if uid != record.owner_uid {
        return Err("owner_uid");
    }
    let expression = record.client_requirement(role)?;
    let mut code = std::ptr::null_mut();
    if unsafe { SecCodeCreateWithXPCMessage(message, 0, &mut code) } != 0 || code.is_null() {
        return Err("message_audit_identity");
    }
    let req = unsafe { requirement(&expression) };
    let valid = match req {
        Ok(req) => {
            let status = unsafe { SecCodeCheckValidity(code, 0, req) };
            unsafe { CFRelease(req) };
            status == 0
        }
        Err(_) => false,
    };
    let version_ok = unsafe { minimum_version(code, &record.minimum_app_version) };
    unsafe { CFRelease(code) };
    if !version_ok {
        return Err("client_version");
    }
    if !valid {
        return Err("signature_identity");
    }
    Ok(VerifiedPeer {
        uid,
        role,
        connection: connection as usize,
    })
}
unsafe fn reply(connection: Obj, message: Obj, response: HelperResponse) {
    let output = unsafe { xpc_dictionary_create_reply(message) };
    if output.is_null() {
        return;
    }
    if let Ok(bytes) = serde_json::to_vec(&response) {
        unsafe {
            xpc_dictionary_set_data(output, c"body".as_ptr(), bytes.as_ptr().cast(), bytes.len());
            xpc_connection_send_message(connection, output);
        }
    }
    unsafe { xpc_release(output) };
}
/// Runs only the fixed launchd Mach service. It never registers or installs it.
pub fn serve(record: InstallRecord) -> Result<(), &'static str> {
    if unsafe { libc::geteuid() } != 0 {
        return Err("helper_requires_root");
    }
    record.validate()?;
    let peer_requirement = CString::new(crate::policy::allowed_peer_requirement(&record)?)
        .map_err(|_| "signature_requirement")?;
    let service = Arc::new(Mutex::new(Service::new(record.clone())?));
    let listener = unsafe {
        xpc_connection_create_mach_service(
            c"local.claudeswitcher.helper".as_ptr(),
            std::ptr::null_mut(),
            1,
        )
    };
    if listener.is_null() {
        return Err("xpc_listener_create");
    }
    let listener_handler = block2::RcBlock::new(move |connection: Obj| {
        if unsafe { xpc_get_type(connection) }
            != std::ptr::addr_of!(_xpc_type_connection).cast_mut().cast()
        {
            return;
        }
        if unsafe { xpc_connection_get_euid(connection) } != record.owner_uid {
            unsafe { xpc_connection_cancel(connection) };
            return;
        }
        // The system gates the peer before connection activation. Message checks remain supplemental.
        if unsafe {
            xpc_connection_set_peer_code_signing_requirement(connection, peer_requirement.as_ptr())
        } != 0
        {
            unsafe { xpc_connection_cancel(connection) };
            return;
        }
        let service = service.clone();
        let record = record.clone();
        let identity = connection as usize;
        let handler = block2::RcBlock::new(move |message: Obj| {
            let connection = identity as Obj;
            if unsafe { xpc_get_type(message) }
                != std::ptr::addr_of!(_xpc_type_dictionary).cast_mut().cast()
            {
                if let Ok(mut service) = service.lock() {
                    service.disconnect(identity);
                }
                return;
            }
            let mut len = 0usize;
            let bytes = unsafe { xpc_dictionary_get_data(message, c"body".as_ptr(), &mut len) };
            let response = (|| {
                if bytes.is_null() || len > 16384 {
                    return Err("message_size");
                }
                let request: HelperRequest = serde_json::from_slice(unsafe {
                    std::slice::from_raw_parts(bytes.cast(), len)
                })
                .map_err(|_| "message_schema")?;
                let peer = unsafe { authenticate(message, connection, &record, request.role) }?;
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|_| "clock_unavailable")?
                    .as_secs();
                if matches!(
                    request.operation,
                    HelperAction::ReconcileManagedTimezone { .. }
                ) {
                    crate::platform::require_idle_timezone_guard(std::path::Path::new(
                        SYSTEM_ROOT,
                    ))?;
                }
                let _root_operation = if crate::root_operation::source_local_pending()?
                    && crate::installation_health::allows_pending_action(&request.operation)
                {
                    None
                } else {
                    Some(crate::root_operation::RootLock::acquire()?)
                };
                Ok(service
                    .lock()
                    .map_err(|_| "service_unavailable")?
                    .handle(&peer, request, now, &Platform))
            })()
            .unwrap_or_else(|code| HelperResponse::status(code, false));
            unsafe { reply(connection, message, response) };
        });
        unsafe {
            xpc_connection_set_event_handler(connection, &handler);
            xpc_connection_activate(connection)
        };
    });
    unsafe {
        xpc_connection_set_event_handler(listener, &listener_handler);
        xpc_connection_activate(listener);
        dispatch_main()
    }
}
/// Read-only native probe. A synthetic dictionary must never authenticate as an XPC sender.
pub fn signature_probe() -> (bool, bool, bool) {
    unsafe {
        let mut code = std::ptr::null_mut();
        let self_valid = SecCodeCopySelf(0, &mut code) == 0
            && !code.is_null()
            && SecCodeCheckValidity(code, 0, std::ptr::null_mut()) == 0;
        let apple_valid = match requirement("anchor apple generic") {
            Ok(req) => {
                let valid = !code.is_null() && SecCodeCheckValidity(code, 0, req) == 0;
                CFRelease(req);
                valid
            }
            Err(_) => false,
        };
        if !code.is_null() {
            CFRelease(code);
        }
        let message = xpc_dictionary_create(std::ptr::null(), std::ptr::null(), 0);
        let mut forged = std::ptr::null_mut();
        let forged_valid = SecCodeCreateWithXPCMessage(message, 0, &mut forged) == 0;
        if !forged.is_null() {
            CFRelease(forged);
        }
        xpc_release(message);
        (self_valid, apple_valid, forged_valid)
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn versions_are_numeric_and_fail_closed() {
        assert!(super::version_parts("1.10.0") > super::version_parts("1.2.0"));
        assert!(super::version_parts("1.2.0-beta").is_none());
    }
    #[test]
    fn synthetic_dictionary_has_no_sender_identity() {
        assert!(!super::signature_probe().2);
    }
}

#[link(name = "System")]
unsafe extern "C" {
    fn xpc_connection_set_peer_code_signing_requirement(
        connection: Obj,
        requirement: *const c_char,
    ) -> i32;
    fn xpc_connection_send_message_with_reply(
        connection: Obj,
        message: Obj,
        queue: Obj,
        handler: &Block<dyn Fn(Obj)>,
    );
}
/// A live connection owns its session. Tokens cannot be reused on another connection.
pub struct NativeClient {
    connection: usize,
    role: ClientRole,
    session: Option<String>,
    sequence: u64,
}
fn request_budget(operation: &HelperAction) -> u64 {
    match operation {
        HelperAction::ApplyRepair { .. } | HelperAction::RecoverRepair { .. } => 150,
        HelperAction::RepairStatus { .. } | HelperAction::ReconcileManagedTimezone { .. } => 20,
        _ => 5,
    }
}
impl NativeClient {
    pub fn connect(record: &InstallRecord, role: ClientRole) -> Result<Self, &'static str> {
        record.validate()?;
        record.client_requirement(role)?;
        if unsafe { libc::geteuid() } != record.owner_uid {
            return Err("owner_uid");
        }
        let connection = unsafe {
            xpc_connection_create_mach_service(
                c"local.claudeswitcher.helper".as_ptr(),
                std::ptr::null_mut(),
                2,
            )
        };
        if connection.is_null() {
            return Err("helper_unavailable");
        }
        let expression =
            CString::new(record.helper_requirement()?).map_err(|_| "signature_requirement")?;
        if unsafe {
            xpc_connection_set_peer_code_signing_requirement(connection, expression.as_ptr())
        } != 0
        {
            unsafe {
                xpc_connection_cancel(connection);
                xpc_release(connection);
            }
            return Err("helper_signature_requirement");
        }
        let handler = block2::RcBlock::new(|_: Obj| {});
        unsafe {
            xpc_connection_set_event_handler(connection, &handler);
            xpc_connection_activate(connection)
        };
        let mut client = Self {
            connection: connection as usize,
            role,
            session: None,
            sequence: 0,
        };
        let response = client.call(HelperAction::BeginSession)?;
        if !response.ok {
            return Err("helper_session_rejected");
        }
        client.session = response.session;
        if client.session.is_none() {
            return Err("helper_session_missing");
        }
        Ok(client)
    }
    pub fn call(&mut self, operation: HelperAction) -> Result<HelperResponse, &'static str> {
        // Mutation can use 25 bounded five-second commands including rollback/state reads.
        // A timeout reports an unknown outcome; cancelling this connection does not cancel root.
        let budget = request_budget(&operation);
        let begin = matches!(operation, HelperAction::BeginSession);
        if !begin {
            self.sequence = self.sequence.checked_add(1).ok_or("sequence_exhausted")?;
        }
        let request = HelperRequest {
            role: self.role,
            session: self.session.clone(),
            sequence: self.sequence,
            operation,
        };
        let bytes = serde_json::to_vec(&request).map_err(|_| "request_schema")?;
        let message = unsafe { xpc_dictionary_create(std::ptr::null(), std::ptr::null(), 0) };
        if message.is_null() {
            return Err("xpc_allocation");
        }
        unsafe {
            xpc_dictionary_set_data(
                message,
                c"body".as_ptr(),
                bytes.as_ptr().cast(),
                bytes.len(),
            )
        };
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let handler = block2::RcBlock::new(move |reply: Obj| {
            let result = (|| {
                if unsafe { xpc_get_type(reply) }
                    != std::ptr::addr_of!(_xpc_type_dictionary).cast_mut().cast()
                {
                    return Err("helper_unavailable");
                }
                let mut len = 0;
                let bytes = unsafe { xpc_dictionary_get_data(reply, c"body".as_ptr(), &mut len) };
                if bytes.is_null() || len > 16384 {
                    return Err("helper_response_size");
                }
                serde_json::from_slice(unsafe { std::slice::from_raw_parts(bytes.cast(), len) })
                    .map_err(|_| "helper_response_schema")
            })();
            let _ = tx.try_send(result);
        });
        unsafe {
            xpc_connection_send_message_with_reply(
                self.connection as Obj,
                message,
                std::ptr::null_mut(),
                &handler,
            );
            xpc_release(message);
        }
        rx.recv_timeout(std::time::Duration::from_secs(budget))
            .map_err(|_| "helper_outcome_unknown")?
    }
}
impl Drop for NativeClient {
    fn drop(&mut self) {
        unsafe {
            xpc_connection_cancel(self.connection as Obj);
            xpc_release(self.connection as Obj);
        }
    }
}
#[cfg(test)]
mod native_client_tests {
    #[test]
    fn mutation_budget_covers_full_bounded_command_and_rollback_chain() {
        use super::*;
        assert!(
            request_budget(&HelperAction::ApplyRepair {
                plan_id: "test".into()
            }) >= 25 * 5
        );
        assert_eq!(
            request_budget(&HelperAction::RecoverRepair {
                plan_id: "test".into()
            }),
            150
        );
        assert_eq!(request_budget(&HelperAction::ReadSystemState), 5);
        assert_eq!(
            request_budget(&HelperAction::RepairStatus {
                plan_id: "test".into()
            }),
            20
        );
    }

    use super::*;
    #[test]
    fn unregistered_or_wrong_signer_service_never_authenticates() {
        let record = InstallRecord {
            schema_version: 2,
            owner_uid: unsafe { libc::geteuid() },
            edition: switcher_domain::Edition::Personal,
            trust: switcher_domain::system::InstallTrust::DeveloperId {
                team_id: "TESTTEAM01".into(),
            },
            helper_version: "0.2.0".into(),
            minimum_app_version: "0.2.0".into(),
        };
        let start = std::time::Instant::now();
        assert!(NativeClient::connect(&record, ClientRole::Gui).is_err());
        assert!(start.elapsed() < std::time::Duration::from_secs(6));
    }
}
