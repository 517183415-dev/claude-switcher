//! Privileged entry points authenticate native XPC messages before fixed actions.
pub mod platform;
pub mod policy;
#[cfg(target_os = "macos")]
pub mod xpc;

pub mod root_operation;

#[cfg(target_os = "macos")]
pub mod native_authorization;

pub mod installation_health;
