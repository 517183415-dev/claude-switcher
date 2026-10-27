//! Bounded per-channel destination history, encrypted inside the existing Vault item.
use super::*;
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Registry {
    schema: u8,
    #[serde(deserialize_with = "Option::deserialize")]
    selected: Option<String>,
    targets: BTreeMap<String, Connection>,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            schema: 2,
            selected: None,
            targets: BTreeMap::new(),
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Stored {
    Registry(Registry),
    Legacy(Connection),
}

impl Registry {
    pub(super) fn read(bytes: &[u8], channel: AdChannel) -> Result<Self> {
        if bytes.is_empty() {
            return Ok(Self::default());
        }
        let stored: Stored = switcher_core::signed::strict_json(bytes, CONNECTION_LIMIT)
            .map_err(|_| Error::Invalid("r2_configuration"))?;
        let registry = match stored {
            Stored::Registry(registry) => registry,
            Stored::Legacy(connection) => {
                connection.validate(channel)?;
                let target = connection.config.identity()?;
                Self {
                    schema: 2,
                    selected: Some(target.clone()),
                    targets: BTreeMap::from([(target, connection)]),
                }
            }
        };
        registry.validate(channel)?;
        Ok(registry)
    }

    fn validate(&self, channel: AdChannel) -> Result<()> {
        if self.schema != 2
            || self.targets.len() > super::super::TARGET_LIMIT
            || self
                .selected
                .as_ref()
                .is_some_and(|id| !self.targets.contains_key(id))
        {
            return Err(Error::Invalid("r2_configuration_registry"));
        }
        for (id, connection) in &self.targets {
            connection.validate(channel)?;
            if connection.config.identity()? != *id {
                return Err(Error::Invalid("r2_configuration_identity"));
            }
        }
        Ok(())
    }

    pub(super) fn active(&self) -> Option<&Connection> {
        self.selected.as_ref().and_then(|id| self.targets.get(id))
    }

    pub(super) fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    pub(super) fn select(&mut self, target: Option<&str>) -> Result<()> {
        if target.is_some_and(|id| !self.targets.contains_key(id)) {
            return Err(Error::Invalid("r2_retained_target_missing"));
        }
        self.selected = target.map(str::to_owned);
        Ok(())
    }

    pub(super) fn insert(&mut self, connection: Connection) -> Result<()> {
        let id = connection.config.identity()?;
        if !self.targets.contains_key(&id) && self.targets.len() >= super::super::TARGET_LIMIT {
            return Err(Error::Invalid("r2_configuration_capacity"));
        }
        self.targets.insert(id.clone(), connection);
        self.selected = Some(id);
        Ok(())
    }

    pub(super) fn encode(&self, channel: AdChannel) -> Result<Zeroizing<Vec<u8>>> {
        self.validate(channel)?;
        // Keep the existing credential byte budget; capacity failure preserves the Vault item.
        let bytes = Zeroizing::new(
            serde_json::to_vec(self).map_err(|_| Error::Invalid("r2_configuration"))?,
        );
        if bytes.len() > CONNECTION_LIMIT {
            return Err(Error::Invalid("r2_configuration_size"));
        }
        Ok(bytes)
    }

    pub(super) fn check_separation(&self, other: &Self) -> Result<()> {
        for a in self.targets.values() {
            for b in other.targets.values() {
                if (a
                    .config
                    .account_id
                    .eq_ignore_ascii_case(&b.config.account_id)
                    && a.config.bucket == b.config.bucket)
                    || a.access_key == b.access_key
                {
                    return Err(Error::Invalid(
                        "r2_channels_require_separate_buckets_and_keys",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(super) fn public_targets(&self) -> Vec<RemoteTarget> {
        self.targets
            .iter()
            .map(|(id, connection)| RemoteTarget {
                target: id.clone(),
                config: connection.config.clone(),
                selected: self.selected.as_ref() == Some(id),
            })
            .collect()
    }

    pub(super) fn contains(&self, target: &str) -> bool {
        self.targets.contains_key(target)
    }
}
