//! One-provider publication protocol; the actual provider is configured separately.
use super::{Error, Result};
use serde::{Deserialize, Serialize};
use switcher_domain::ads::{AdChannel, AdManifest};

#[derive(Clone)]
pub struct Object {
    pub bytes: Vec<u8>,
    pub etag: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreError {
    Unavailable,
    Conflict,
    OutcomeUnknown,
    Permission,
}

/// All keys are product-relative and environment-prefixed, never user URLs.
/// A provider must enforce bounded reads, anonymous public readback, immutable
/// object create-if-absent, and CAS via If-Match / If-None-Match. Its credential
/// must have write access only to this environment's advertisement directory.
pub trait ObjectStore {
    fn channel(&self) -> AdChannel;
    fn requires_credentials(&self) -> bool {
        true
    }
    fn current(&mut self) -> std::result::Result<Option<Object>, StoreError>;
    fn public_read(
        &mut self,
        key: &str,
        limit: usize,
    ) -> std::result::Result<Option<Object>, StoreError>;
    fn put_immutable(&mut self, key: &str, bytes: &[u8]) -> std::result::Result<(), StoreError>;
    fn commit_current(
        &mut self,
        expected_etag: Option<&str>,
        bytes: &[u8],
    ) -> std::result::Result<(), StoreError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Prepared,
    Uploading,
    CommitUnknown,
    Conflict,
    ReadbackFailed,
    Published,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pending {
    pub manifest: AdManifest,
    pub wire: Vec<u8>,
    pub expected_etag: Option<String>,
    pub phase: Phase,
}

pub fn prefix(channel: AdChannel) -> &'static str {
    match channel {
        AdChannel::Preview => "preview",
        AdChannel::Production => "production",
    }
}

pub fn current_key(channel: AdChannel) -> String {
    format!("{}/current.json", prefix(channel))
}
pub fn version_key(manifest: &AdManifest) -> String {
    format!(
        "{}/versions/{}.json",
        prefix(manifest.channel),
        manifest.release_id
    )
}
pub fn asset_key(channel: AdChannel, asset: &switcher_domain::ads::ImageAsset) -> String {
    format!(
        "{}/assets/{}.{}",
        prefix(channel),
        asset.sha256,
        if asset.mime == "image/png" {
            "png"
        } else {
            "jpg"
        }
    )
}

impl From<StoreError> for Error {
    fn from(value: StoreError) -> Self {
        Error::Store(value)
    }
}

pub(crate) fn same_object(actual: &Option<Object>, expected: &[u8]) -> bool {
    actual
        .as_ref()
        .is_some_and(|object| object.bytes == expected)
}

pub(crate) fn require_scope(store: &dyn ObjectStore, channel: AdChannel) -> Result<()> {
    if store.channel() == channel {
        Ok(())
    } else {
        Err(Error::Invalid("publisher_environment"))
    }
}
