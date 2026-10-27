//! Synthetic secret storage shared by unit tests and the isolated IPC fixture.
use super::{
    Error, Result,
    security::{self, Secrets, Vault},
};
use std::collections::HashMap;
use switcher_core::signed;

#[derive(Default)]
pub(crate) struct MemoryVault {
    pub(crate) items: HashMap<String, Vec<u8>>,
    pub(crate) fail: bool,
}
impl Drop for MemoryVault {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        for value in self.items.values_mut() {
            value.zeroize();
        }
    }
}
impl Vault for MemoryVault {
    fn create(&mut self, generation: &str, secrets: &Secrets) -> Result<()> {
        secrets.validate()?;
        if self.fail || self.items.contains_key(generation) {
            return Err(Error::SecretUnavailable);
        }
        self.items
            .insert(generation.into(), serde_json::to_vec(secrets).unwrap());
        Ok(())
    }

    fn load(&self, generation: &str) -> Result<Secrets> {
        let bytes = self.items.get(generation).ok_or(Error::SecretUnavailable)?;
        signed::strict_json(bytes, security::SECRET_LIMIT).map_err(Error::Invalid)
    }
    fn load_optional(&self, generation: &str) -> Result<Option<Secrets>> {
        if self.items.contains_key(generation) {
            self.load(generation).map(Some)
        } else {
            Ok(None)
        }
    }
    fn store(&mut self, generation: &str, secrets: &Secrets) -> Result<()> {
        if self.fail {
            return Err(Error::SecretUnavailable);
        }
        self.items
            .insert(generation.into(), serde_json::to_vec(secrets).unwrap());
        Ok(())
    }
}
