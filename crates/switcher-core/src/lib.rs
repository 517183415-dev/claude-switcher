//! Business operations shared by the desktop, CLI and user guardian.
pub mod checks;
pub mod config;
pub mod diagnostics;
pub mod macos;
pub mod network;
pub mod signed;

pub fn runtime_info(edition: switcher_domain::Edition) -> switcher_domain::RuntimeInfo {
    switcher_domain::RuntimeInfo {
        schema_version: 1,
        version: env!("CARGO_PKG_VERSION").into(),
        edition,
        slots: switcher_domain::AdSlot::ALL,
        automatic_updates: edition == switcher_domain::Edition::Public,
        ads_enabled: false,
    }
}

pub mod ads;
pub mod time;
pub mod updates;

pub mod guardian;
mod guardian_repair;
pub mod install;
pub mod installation_health;
mod proxy_repair;

pub mod storage;

pub mod accounts;

pub mod migration;

pub mod ad_runtime;

pub mod lifecycle;
pub mod region_detection;
pub mod regions;
pub mod repair_cli;
pub mod user_repairs;
