//! Versioned UI data. Privileged operations remain in the core and helper.
use crate::{
    RuntimeInfo,
    ads::{AdManifest, AdPreferences, ImageAsset},
    checks::CheckReport,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProfileState {
    Uninitialized,
    Saved,
    Current,
    RecoveryPending,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AccountProfile {
    pub id: String,
    pub state: ProfileState,
    pub login_evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AccountOverview {
    pub current: Option<String>,
    pub profiles: Vec<AccountProfile>,
    pub claude_running: bool,
    pub action_blocker: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    pub ads: AdPreferences,
    pub updates_paused_until: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UpdateView {
    /// Actual schedule setting; None means the schedule could not be read safely.
    pub paused: Option<bool>,
    pub state: String,
    pub message: String,
    pub next_check_at: Option<u64>,
    pub candidate_version: Option<String>,
    pub can_download: bool,
    pub can_install: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceSnapshot {
    pub runtime: RuntimeInfo,
    pub accounts: AccountOverview,
    pub preferences: Preferences,
    pub check_report: Option<CheckReport>,
    pub ads_manifest: Option<AdManifest>,
    pub update: UpdateView,
    pub pending_materials: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ActivityEntry {
    pub observed_at: u64,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct OperationOutcome {
    pub completed: bool,
    pub message: String,
    pub recovery: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AccountAction {
    InitializeA,
    AssociateA,
    SelectA,
    SelectB,
    Recover,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum UpdateAction {
    Check,
    Download,
    Install,
    Pause,
    Resume,
    Defer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RegionPreset {
    Us,
    Jp,
    Tw,
    Hk,
    Sg,
    Gb,
    De,
    Fr,
    Kr,
    Au,
    Ca,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RegionView {
    pub country: String,
    pub timezone: String,
    pub locale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RepairPreview {
    pub can_execute: bool,
    pub message: String,
    pub original: RegionView,
    pub target: RegionView,
    pub network_service: String,
    pub original_ipv6: String,
    pub target_ipv6: String,
    pub user_changes: Vec<RepairChange>,
    pub recovery_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RepairChange {
    pub id: String,
    pub state: String,
    pub original: String,
    pub target: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ImportedImage {
    pub asset: ImageAsset,
    pub preview_data_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PublisherView {
    pub source: Option<String>,
    pub status: String,
    pub message: String,
    pub history: Vec<PublishRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PublishRecord {
    pub release_id: String,
    pub revision: u64,
    pub channel: String,
    pub published_at: u64,
    pub payload_hash: String,
    pub verified: bool,
}

/// Safe readiness summary; no account paths, credentials or Keychain selectors.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AccountPreparation {
    pub ready: bool,
    pub backup_allowed: bool,
    pub blockers: Vec<PreparationBlocker>,
    pub in_place_action: Option<InPlaceAction>,
    pub in_place_ready: bool,
    pub in_place_blockers: Vec<PreparationBlocker>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PreparationBlocker {
    pub code: String,
    pub message: String,
    pub recovery: String,
}

/// A fixed in-place action, never a directory exchange or client-supplied assessment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum InPlaceAction {
    InitializeA,
    AssociateA,
}
