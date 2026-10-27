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
    /// Trusted implementation identity; unknown providers are never publishable.
    fn target_identity(&self) -> Option<String> {
        None
    }
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

/// Restrict object access to the channel and product-generated key shapes.
pub(crate) fn validate_key(channel: AdChannel, key: &str) -> std::result::Result<(), StoreError> {
    let prefix = format!("{}/", prefix(channel));
    let suffix = key.strip_prefix(&prefix).ok_or(StoreError::Permission)?;
    let valid = suffix == "current.json"
        || suffix.strip_prefix("assets/").is_some_and(|name| {
            name.rsplit_once('.').is_some_and(|(hash, extension)| {
                hash.len() == 64
                    && hash
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                    && matches!(extension, "png" | "jpg")
            })
        })
        || suffix
            .strip_prefix("versions/")
            .and_then(|name| name.strip_suffix(".json"))
            .is_some_and(|id| {
                !id.is_empty()
                    && id.len() <= 128
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
            });
    if valid {
        Ok(())
    } else {
        Err(StoreError::Permission)
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
