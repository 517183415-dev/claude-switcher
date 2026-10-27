//! Assemble all authenticated sources before local index maintenance or file GC.
use super::super::{
    channel_index,
    cleanup::FixedCurrents,
    protocol::{Object, StoreError},
    remote_lock::Lease,
};
use super::*;
use std::collections::BTreeMap;

struct Remote {
    store: Box<dyn ObjectStore>,
    lease: Option<Lease>,
}
enum Location {
    Local,
    Remote(String),
}
struct Sources {
    active: String,
    local: Option<LocalObjectStore>,
    targets: BTreeMap<String, Location>,
    remotes: BTreeMap<String, Remote>,
}

impl Sources {
    fn verify(&mut self) -> Result<()> {
        for remote in self.remotes.values_mut() {
            remote
                .lease
                .as_ref()
                .ok_or(Error::Invalid("remote_lock_missing"))?
                .verify(remote.store.as_mut())?;
        }
        Ok(())
    }
    fn read(&mut self, target: &str) -> Result<[Option<Object>; 2]> {
        self.verify()?;
        match self
            .targets
            .get(target)
            .ok_or(Error::Invalid("cleanup_target_missing"))?
        {
            Location::Local => self
                .local
                .as_mut()
                .ok_or(Error::MissingSource)?
                .read_currents()
                .map_err(Error::Store),
            Location::Remote(id) => {
                let remote = self.remotes.get_mut(id).ok_or(Error::MissingSource)?;
                let mut objects = [None, None];
                objects[channel_index(remote.store.channel())] = remote.store.current()?;
                Ok(objects)
            }
        }
    }
    fn release(&mut self, publisher: &Publisher) -> Result<()> {
        // Keep every acquired lock if any ledger still has an uncertain publication.
        if publisher.has_pending_publications() {
            return Ok(());
        }
        let mut error = None;
        for remote in self.remotes.values_mut().rev() {
            if let Some(lease) = remote.lease.take()
                && let Err(e) = lease.release(publisher, remote.store.as_mut())
            {
                error = Some(e);
            }
        }
        error.map_or(Ok(()), Err)
    }
}

impl FixedCurrents for Sources {
    fn target_identity(&self) -> Option<String> {
        Some(self.active.clone())
    }
    fn supports_multiple_targets(&self) -> bool {
        true
    }
    fn read_currents(&mut self) -> std::result::Result<[Option<Object>; 2], StoreError> {
        self.read(&self.active.clone()).map_err(|e| match e {
            Error::Store(e) => e,
            _ => StoreError::Permission,
        })
    }
    fn read_other_currents(
        &mut self,
        targets: &[String],
    ) -> Result<BTreeMap<String, [Option<Object>; 2]>> {
        targets
            .iter()
            .map(|target| Ok((target.clone(), self.read(target)?)))
            .collect()
    }
}

impl Publisher {
    pub(crate) fn with_configured_currents<T>(
        &mut self,
        root: &Path,
        vault: &dyn Vault,
        run: impl FnOnce(&mut Self, &mut dyn FixedCurrents) -> Result<T>,
    ) -> Result<T> {
        self.with_configured_currents_using(
            root,
            vault,
            |config, access, secret| Ok(Box::new(R2Store::open(config, access, secret)?)),
            run,
        )
    }

    pub(super) fn with_configured_currents_using<T>(
        &mut self,
        root: &Path,
        vault: &dyn Vault,
        mut open: impl FnMut(R2Config, &str, &str) -> Result<Box<dyn ObjectStore>>,
        run: impl FnOnce(&mut Self, &mut dyn FixedCurrents) -> Result<T>,
    ) -> Result<T> {
        let secrets = vault.load(&self.generation)?;
        self.check_signer(&secrets)?;
        let registries = registries(&secrets)?;
        let mut required: Vec<String> = self.state.other_targets.keys().cloned().collect();
        if let Some(target) = &self.state.target {
            required.push(target.clone());
        }
        // Resolve every remote target before opening any provider or issuing requests.
        let mut configs = BTreeMap::new();
        for target in &required {
            if target.starts_with("r2:") {
                let connection = registries
                    .iter()
                    .find_map(|r| r.get(target))
                    .ok_or(Error::Invalid("cleanup_target_configuration_missing"))?;
                configs.insert(target.clone(), connection);
            } else if !target.starts_with("local:") {
                return Err(Error::Invalid("cleanup_target_configuration_missing"));
            }
        }
        let local =
            if self.state.target.is_none() || required.iter().any(|t| t.starts_with("local:")) {
                Some(LocalObjectStore::open(
                    root,
                    AdChannel::Preview,
                    &switcher_core::ad_runtime::local::LocalTrust {
                        schema: 1,
                        key_id: self.key_id.clone(),
                        public_key: self.verifying_key()?.to_bytes(),
                    },
                )?)
            } else {
                None
            };
        let local_id = local.as_ref().and_then(ObjectStore::target_identity);
        if required
            .iter()
            .filter(|t| t.starts_with("local:"))
            .any(|t| Some(t) != local_id.as_ref())
        {
            return Err(Error::Invalid("cleanup_local_target_changed"));
        }
        let active = self
            .state
            .target
            .clone()
            .or_else(|| local_id.clone())
            .ok_or(Error::MissingSource)?;
        if required.is_empty() {
            required.push(active.clone());
        }
        let mut sources = Sources {
            active,
            local,
            targets: BTreeMap::new(),
            remotes: BTreeMap::new(),
        };
        for target in required {
            if let Some(connection) = configs.get(&target) {
                let id = connection.config.lock_identity()?;
                if !sources.remotes.contains_key(&id) {
                    let store = open(
                        connection.config.clone(),
                        &connection.access_key,
                        &connection.secret_key,
                    )?;
                    if store.target_identity().as_ref() != Some(&target)
                        || store.lock_identity().as_ref() != Some(&id)
                    {
                        return Err(Error::Invalid("cleanup_provider_identity"));
                    }
                    sources
                        .remotes
                        .insert(id.clone(), Remote { store, lease: None });
                }
                sources.targets.insert(target, Location::Remote(id));
            } else {
                sources.targets.insert(target, Location::Local);
            }
        }
        // Deterministic acquisition order; no waiting, expiry or foreign-owner takeover.
        for remote in sources.remotes.values_mut() {
            match Lease::acquire(self, remote.store.as_mut()) {
                Ok(lease) => remote.lease = Some(lease),
                Err(error) => {
                    sources.release(self)?;
                    return Err(error);
                }
            }
        }
        let result = run(self, &mut sources);
        sources.release(self)?;
        result
    }
}
