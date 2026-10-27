//! Personal operator settings share the atomic, encrypted Vault secret bundle.
use super::{
    Error, Publication, Publisher, Result,
    local_store::LocalObjectStore,
    protocol::ObjectStore,
    r2_store::{R2Config, R2Store},
    security::{Secrets, Vault},
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use switcher_domain::ads::AdChannel;
use zeroize::{Zeroize, Zeroizing};

const CONNECTION_LIMIT: usize = 16 * 1024;
mod maintenance;
mod registry;
use registry::Registry;

#[derive(Serialize)]
pub struct RemoteTarget {
    pub target: String,
    pub config: R2Config,
    pub selected: bool,
}

#[derive(Default, Serialize)]
pub struct RemoteSettings {
    pub preview: Option<R2Config>,
    pub production: Option<R2Config>,
    pub targets: Vec<RemoteTarget>,
    pub unavailable_targets: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Connection {
    schema: u8,
    config: R2Config,
    access_key: String,
    secret_key: String,
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.access_key.zeroize();
        self.secret_key.zeroize();
    }
}

impl Connection {
    fn validate(&self, channel: AdChannel) -> Result<()> {
        if self.schema != 1
            || self.config.channel != channel
            || !valid_secret(&self.access_key, 256)
            || !valid_secret(&self.secret_key, 4096)
        {
            return Err(Error::Invalid("r2_configuration"));
        }
        self.config.validate()?;
        Ok(())
    }
}

fn valid_secret(value: &str, limit: usize) -> bool {
    !value.is_empty() && value.len() <= limit && value.bytes().all(|b| b.is_ascii_graphic())
}

fn credential(secrets: &Secrets, channel: AdChannel) -> &[u8] {
    match channel {
        AdChannel::Preview => &secrets.preview_credential,
        AdChannel::Production => &secrets.production_credential,
    }
}

fn registries(secrets: &Secrets) -> Result<[Registry; 2]> {
    let preview = Registry::read(&secrets.preview_credential, AdChannel::Preview)?;
    let production = Registry::read(&secrets.production_credential, AdChannel::Production)?;
    preview.check_separation(&production)?;
    Ok([preview, production])
}

impl Publisher {
    fn settings(&self, secrets: &Secrets) -> Result<RemoteSettings> {
        let [preview, production] = registries(secrets)?;
        let unavailable_targets = self
            .state
            .target
            .iter()
            .chain(self.state.other_targets.keys())
            .filter(|target| {
                target.starts_with("r2:")
                    && !preview.contains(target)
                    && !production.contains(target)
            })
            .cloned()
            .collect();
        let mut targets = preview.public_targets();
        targets.extend(production.public_targets());
        Ok(RemoteSettings {
            preview: preview.active().map(|c| c.config.clone()),
            production: production.active().map(|c| c.config.clone()),
            targets,
            unavailable_targets,
        })
    }
    pub fn active_target(&self) -> Option<&str> {
        self.state.target.as_deref()
    }

    pub fn remote_settings(&self, vault: &dyn Vault) -> Result<RemoteSettings> {
        let Some(secrets) = vault.load_optional(&self.generation)? else {
            return Ok(RemoteSettings::default());
        };
        self.check_signer(&secrets)?;
        self.settings(&secrets)
    }

    /// None explicitly returns this channel to local publication. Pending targets
    /// retain their settings until reconciled; credential rotation remains possible.
    pub fn configure_r2(
        &mut self,
        channel: AdChannel,
        config: Option<R2Config>,
        access_key: Zeroizing<String>,
        secret_key: Zeroizing<String>,
        vault: &mut dyn Vault,
    ) -> Result<RemoteSettings> {
        self.verifying_key()?;
        let mut secrets = vault.load(&self.generation)?;
        self.check_signer(&secrets)?;
        let mut registry = Registry::read(credential(&secrets, channel), channel)?;
        let next = config.map(|config| Connection {
            schema: 1,
            config,
            access_key: access_key.to_string(),
            secret_key: secret_key.to_string(),
        });
        if let Some(next) = &next {
            next.validate(channel)?;
        } else if !access_key.is_empty() || !secret_key.is_empty() {
            return Err(Error::Invalid("r2_configuration"));
        }
        match next {
            Some(next) => registry.insert(next)?,
            None => registry.select(None)?,
        }
        self.save_remote_registry(channel, registry, &mut secrets, vault)
    }

    /// Selecting a retained target reuses its Vault credential without returning it over IPC.
    pub fn select_retained_r2(
        &mut self,
        channel: AdChannel,
        target: &str,
        vault: &mut dyn Vault,
    ) -> Result<RemoteSettings> {
        self.verifying_key()?;
        let mut secrets = vault.load(&self.generation)?;
        self.check_signer(&secrets)?;
        let mut registry = Registry::read(credential(&secrets, channel), channel)?;
        registry.select(Some(target))?;
        self.save_remote_registry(channel, registry, &mut secrets, vault)
    }

    fn save_remote_registry(
        &mut self,
        channel: AdChannel,
        registry: Registry,
        secrets: &mut Secrets,
        vault: &mut dyn Vault,
    ) -> Result<RemoteSettings> {
        let previous = Registry::read(credential(secrets, channel), channel)?;
        if let Some(target) = previous.selected() {
            let pending = if self.state.target.as_deref() == Some(target) {
                self.state.pending.is_some()
            } else {
                self.state
                    .other_targets
                    .get(target)
                    .is_some_and(|s| s.pending.is_some())
            };
            if pending && registry.selected() != Some(target) {
                return Err(Error::PendingOperation);
            }
        }
        let bytes = registry.encode(channel)?;
        let destination = match channel {
            AdChannel::Preview => &mut secrets.preview_credential,
            AdChannel::Production => &mut secrets.production_credential,
        };
        destination.zeroize();
        *destination = bytes.to_vec();
        let view = self.settings(secrets)?;
        self.credentials_save(vault, secrets)?;
        Ok(view)
    }

    pub fn publish_configured(
        &mut self,
        root: &Path,
        channel: AdChannel,
        now: u64,
        vault: &dyn Vault,
    ) -> Result<Publication> {
        self.publish_configured_with(root, channel, now, vault, |config, access, secret| {
            Ok(Box::new(R2Store::open(config, access, secret)?))
        })
    }

    fn publish_configured_with(
        &mut self,
        root: &Path,
        channel: AdChannel,
        now: u64,
        vault: &dyn Vault,
        remote: impl FnOnce(R2Config, &str, &str) -> Result<Box<dyn ObjectStore>>,
    ) -> Result<Publication> {
        let secrets = vault.load(&self.generation)?;
        self.check_signer(&secrets)?;
        // Parse both registries before any store access, including retained credentials.
        let [preview, production] = registries(&secrets)?;
        let registry = match channel {
            AdChannel::Preview => preview,
            AdChannel::Production => production,
        };
        let connection = registry.active();
        let is_remote = connection.is_some();
        let mut source: Box<dyn ObjectStore> = match connection {
            Some(connection) => remote(
                connection.config.clone(),
                &connection.access_key,
                &connection.secret_key,
            )?,
            None => Box::new(LocalObjectStore::open(
                root,
                channel,
                &switcher_core::ad_runtime::local::LocalTrust {
                    schema: 1,
                    key_id: self.key_id.clone(),
                    public_key: self.verifying_key()?.to_bytes(),
                },
            )?),
        };
        if is_remote || self.state.schema == 3 {
            self.select_target(source.as_ref())?;
        }
        let lease = if is_remote {
            Some(super::remote_lock::Lease::acquire(self, source.as_mut())?)
        } else {
            None
        };
        let result = self.publish(channel, now, Some(source.as_mut()), vault);
        // Unknown publication outcomes retain ownership until the existing CAS recovery finishes.
        if !self.has_pending_publications()
            && let Some(lease) = lease
        {
            lease.release(self, source.as_mut())?;
        }
        result
    }
}

#[cfg(test)]
mod tests;
