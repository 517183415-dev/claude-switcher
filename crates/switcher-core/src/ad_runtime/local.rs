//! Read-only access to Personal's fixed, user-private signed local publication.
use super::*;
use std::{io, os::unix::fs::MetadataExt, path::Path};
const ROUTING_BASE: &str = "https://local-ads.example.com/";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalTrust {
    pub schema: u32,
    pub key_id: String,
    pub public_key: [u8; 32],
}
impl LocalTrust {
    pub fn validate(&self) -> Result<()> {
        if self.schema != 1 || self.key_id != "ads-personal-v1" {
            return Err("local_ad_trust_scope");
        }
        VerifyingKey::from_bytes(&self.public_key).map_err(|_| "local_ad_trust_key")?;
        Ok(())
    }
}
pub fn directory(application_root: &Path) -> PathBuf {
    application_root.join("v2/local-ads")
}
pub fn validate_directory(path: &Path) -> Result<()> {
    storage::validate_no_symlinks(path).map_err(|_| "local_ad_directory")?;
    let metadata = std::fs::metadata(path).map_err(|_| "local_ad_directory")?;
    if !metadata.is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err("local_ad_directory");
    }
    Ok(())
}
fn read(root: &Path, key: &str, limit: usize) -> Result<Vec<u8>> {
    validate_directory(root)?;
    validate_directory(root.parent().ok_or("local_ad_directory")?)?;
    let path = root.join(key);
    if let Some(channel) = key.split('/').next().filter(|_| key.contains('/')) {
        validate_directory(&root.join(channel))?;
    }
    validate_directory(path.parent().ok_or("local_ad_directory")?)?;
    storage::read_private_bytes(&path, limit).map_err(|_| "local_ad_read")
}
pub(super) fn pins(application_root: &Path) -> Result<Option<ContentPins>> {
    let root = directory(application_root);
    match std::fs::symlink_metadata(&root) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("local_ad_directory"),
        Ok(_) => {}
    }
    validate_directory(&root)?;
    let bytes = match storage::read_private_bytes(&root.join("trust.json"), 4096) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("local_ad_trust_read"),
    };
    let trust: LocalTrust = signed::strict_json(&bytes, 4096)?;
    trust.validate()?;
    Ok(Some(ContentPins {
        source: ContentSource::new(ROUTING_BASE)?,
        key: VerifyingKey::from_bytes(&trust.public_key).map_err(|_| "local_ad_trust_key")?,
        key_id: trust.key_id,
        channel: AdChannel::Production,
        local: true,
    }))
}
pub(super) struct LocalFetcher {
    pub root: PathBuf,
}
impl Fetcher for LocalFetcher {
    fn get(&mut self, url: &Url, limit: usize) -> Result<Download> {
        let source = ContentSource::new(ROUTING_BASE)?;
        if !source.permits_response_url(url, url) {
            return Err("local_ad_key");
        }
        let key = url
            .as_str()
            .strip_prefix(ROUTING_BASE)
            .ok_or("local_ad_key")?;
        if !key.starts_with("production/") {
            return Err("local_ad_environment");
        }
        let mime = if key.ends_with(".json") {
            "application/json"
        } else if key.ends_with(".png") {
            "image/png"
        } else {
            "image/jpeg"
        };
        Ok(Download {
            bytes: read(&self.root, key, limit)?,
            mime: Some(mime.into()),
            final_url: url.clone(),
        })
    }
}
/// No writes and no network access. Missing publication is an ordinary local WAIT.
pub fn configured(application_root: &Path) -> bool {
    matches!(pins(application_root), Ok(Some(_)))
}
pub(super) fn current_digest(root: &Path) -> Result<Option<String>> {
    let root = directory(root);
    validate_directory(&root)?;
    validate_directory(&root.join("production"))?;
    match storage::read_private_bytes(
        &root.join("production/current.json"),
        ads::MAX_MANIFEST_BYTES,
    ) {
        Ok(bytes) => Ok(Some(signed::digest(&bytes))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("local_ad_current_read"),
    }
}
