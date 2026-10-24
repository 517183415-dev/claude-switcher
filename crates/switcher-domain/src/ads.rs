//! Advertisement data shared by the editor, preview and client.
use crate::AdSlot;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AdChannel {
    Preview,
    Production,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ImageAsset {
    pub sha256: String,
    pub mime: String,
    pub bytes: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Campaign {
    pub campaign_id: String,
    pub merchant_id: String,
    pub merchant_name: String,
    pub title: String,
    pub description: String,
    pub image: ImageAsset,
    pub button_text: String,
    pub target_url: String,
    pub starts_at: u64,
    pub ends_at: u64,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SlotContent {
    pub slot: AdSlot,
    pub enabled: bool,
    pub campaigns: Vec<Campaign>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AdManifest {
    pub schema_version: u32,
    pub channel: AdChannel,
    pub content_revision: u64,
    pub key_id: String,
    pub issued_at: u64,
    pub expires_at: u64,
    pub release_id: String,
    pub slots: Vec<SlotContent>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AdPreferences {
    pub slots_enabled: [bool; 3],
    pub request_purpose_acknowledged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct MerchantConsent {
    pub slot: AdSlot,
    pub merchant_id: String,
    pub normalized_host: String,
}
