//! Shared, versioned data. No account, network, or platform operations.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Edition {
    Personal,
    Public,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AdSlot {
    ResidentialBroadband,
    AccountService,
    KycService,
}

impl AdSlot {
    pub const ALL: [Self; 3] = [
        Self::ResidentialBroadband,
        Self::AccountService,
        Self::KycService,
    ];
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RuntimeInfo {
    pub schema_version: u32,
    pub version: String,
    pub edition: Edition,
    pub slots: [AdSlot; 3],
    pub automatic_updates: bool,
    pub ads_enabled: bool,
}

pub fn export_types(directory: impl Into<std::path::PathBuf>) -> Result<(), ts_rs::ExportError> {
    let config = ts_rs::Config::new()
        .with_out_dir(directory)
        .with_large_int("number");
    RuntimeInfo::export_all(&config)?;
    checks::CheckReport::export_all(&config)?;
    checks::DiagnosticReport::export_all(&config)?;
    ads::AdManifest::export_all(&config)?;
    ads::AdPreferences::export_all(&config)?;
    ads::MerchantConsent::export_all(&config)?;
    desktop::WorkspaceSnapshot::export_all(&config)?;
    desktop::ActivityEntry::export_all(&config)?;
    desktop::OperationOutcome::export_all(&config)?;
    desktop::AccountAction::export_all(&config)?;
    desktop::AccountPreparation::export_all(&config)?;
    desktop::UpdateAction::export_all(&config)?;
    desktop::ImportedImage::export_all(&config)?;
    desktop::RegionView::export_all(&config)?;
    desktop::RegionPreset::export_all(&config)?;
    desktop::RepairPreview::export_all(&config)?;
    desktop::PublisherView::export_all(&config)?;
    desktop::PublisherCleanupPreview::export_all(&config)?;
    desktop::PublisherGcPreview::export_all(&config)
}

pub mod ads;
pub mod checks;

pub mod desktop;
pub mod system;
