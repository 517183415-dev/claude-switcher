//! Non-expiring cooperative locks. Unique CAS revisions fence delayed requests.
//! Ownership is local operational state, deliberately absent from portable backups.
use super::protocol::{Object, ObjectStore, StoreError};
use super::{Error, Publisher, Result, TARGET_LIMIT, encode, random_id, valid_generation};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use switcher_core::{signed, storage};

pub(super) const WIRE_LIMIT: usize = 1024;
const JOURNAL_LIMIT: usize = 16 * 1024;
const JOURNAL: &str = "remote-locks.json";

fn valid_identity(id: &str) -> bool {
    id.strip_prefix("r2-lock:")
        .is_some_and(|hash| hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: u8,
    #[serde(deserialize_with = "Option::deserialize")]
    owner: Option<String>,
    revision: String,
}

impl Record {
    fn read(object: &Object) -> Result<Self> {
        let record: Self =
            signed::strict_json(&object.bytes, WIRE_LIMIT).map_err(Error::Invalid)?;
        if record.schema != 1
            || !valid_generation(&record.revision)
            || record
                .owner
                .as_deref()
                .is_some_and(|s| !valid_generation(s))
            || object.etag.is_empty()
            || object.etag.len() > 1024
            || object.etag.chars().any(char::is_control)
        {
            return Err(Error::Invalid("remote_lock_record"));
        }
        Ok(record)
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    owners: BTreeMap<String, String>,
}

impl Publisher {
    pub(super) fn has_pending_publications(&self) -> bool {
        self.state.pending.is_some()
            || self
                .state
                .other_targets
                .values()
                .any(|ledger| ledger.pending.is_some())
    }
    fn lock_journal(&self) -> Result<Journal> {
        if !self._lock.protects(&self.root) {
            return Err(Error::Invalid("remote_lock_root_changed"));
        }
        let journal: Journal =
            match storage::read_private_bytes(&self.root.join(JOURNAL), JOURNAL_LIMIT) {
                Ok(bytes) => signed::strict_json(&bytes, JOURNAL_LIMIT).map_err(Error::Invalid)?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Journal::default(),
                Err(e) => return Err(e.into()),
            };
        if journal.owners.len() > TARGET_LIMIT * 2
            || journal
                .owners
                .iter()
                .any(|(id, owner)| !valid_identity(id) || !valid_generation(owner))
        {
            return Err(Error::Invalid("remote_lock_journal"));
        }
        Ok(journal)
    }

    fn save_lock_journal(&self, journal: &Journal) -> Result<()> {
        if !self._lock.protects(&self.root) {
            return Err(Error::Invalid("remote_lock_root_changed"));
        }
        storage::atomic_write(&self.root.join(JOURNAL), &encode(journal, JOURNAL_LIMIT)?)?;
        Ok(())
    }

    pub(super) fn require_no_remote_intents(&self) -> Result<()> {
        if !self.lock_journal()?.owners.is_empty() {
            return Err(Error::Invalid("remote_lock_recovery_required"));
        }
        Ok(())
    }
}

pub(super) struct Lease {
    identity: String,
    owner: String,
    revision: String,
    etag: String,
}

impl Lease {
    pub(super) fn acquire(publisher: &Publisher, source: &mut dyn ObjectStore) -> Result<Self> {
        let identity = source
            .lock_identity()
            .ok_or(Error::Invalid("remote_lock_unsupported"))?;
        if !valid_identity(&identity) {
            return Err(Error::Invalid("remote_lock_identity"));
        }
        // Validate provider identities before they can enter the durable journal.
        let mut journal = publisher.lock_journal()?;
        let owner = journal
            .owners
            .get(&identity)
            .cloned()
            .map(Ok)
            .unwrap_or_else(random_id)?;
        let current = source.read_lock()?;
        if let Some(object) = &current {
            let record = Record::read(object)?;
            if record.owner.as_ref().is_some_and(|held| held != &owner) {
                return Err(StoreError::Conflict.into());
            }
        }
        if !journal.owners.contains_key(&identity) && journal.owners.len() >= TARGET_LIMIT * 2 {
            return Err(Error::Invalid("remote_lock_capacity"));
        }
        // Persist BEFORE issuing a request that can time out and commit later.
        journal.owners.insert(identity.clone(), owner.clone());
        publisher.save_lock_journal(&journal)?;
        let revision = random_id()?;
        let wire = encode(
            &Record {
                schema: 1,
                owner: Some(owner.clone()),
                revision: revision.clone(),
            },
            WIRE_LIMIT,
        )?;
        source.commit_lock(current.as_ref().map(|o| o.etag.as_str()), &wire)?;
        let object = source
            .read_lock()?
            .ok_or(Error::Invalid("remote_lock_readback"))?;
        let record = Record::read(&object)?;
        if record.owner.as_ref() != Some(&owner) || record.revision != revision {
            return Err(StoreError::Conflict.into());
        }
        Ok(Self {
            identity,
            owner,
            revision,
            etag: object.etag,
        })
    }

    pub(super) fn verify(&self, source: &mut dyn ObjectStore) -> Result<()> {
        if source.lock_identity().as_ref() != Some(&self.identity) {
            return Err(Error::Invalid("remote_lock_identity"));
        }
        let object = source
            .read_lock()?
            .ok_or(Error::Invalid("remote_lock_lost"))?;
        let record = Record::read(&object)?;
        if object.etag != self.etag
            || record.owner.as_ref() != Some(&self.owner)
            || record.revision != self.revision
        {
            return Err(StoreError::Conflict.into());
        }
        Ok(())
    }

    pub(super) fn release(self, publisher: &Publisher, source: &mut dyn ObjectStore) -> Result<()> {
        self.verify(source)?;
        let wire = encode(
            &Record {
                schema: 1,
                owner: None,
                revision: random_id()?,
            },
            WIRE_LIMIT,
        )?;
        // An unknown response retains the journal. Never DELETE (avoids ABA).
        source.commit_lock(Some(&self.etag), &wire)?;
        let mut journal = publisher.lock_journal()?;
        if journal.owners.get(&self.identity) != Some(&self.owner) {
            return Err(Error::Invalid("remote_lock_owner_changed"));
        }
        journal.owners.remove(&self.identity);
        publisher.save_lock_journal(&journal)
    }
}

#[cfg(test)]
mod tests;
