//! Personal-only secret storage. No command line process receives secret bytes.
use super::{Error, Result};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

pub const SECRET_LIMIT: usize = 160 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Secrets {
    pub signing_seed: Vec<u8>,
    pub preview_credential: Vec<u8>,
    pub production_credential: Vec<u8>,
}

impl Secrets {
    pub fn validate(&self) -> Result<()> {
        if self.signing_seed.len() != 32
            || self.preview_credential.len() > 16 * 1024
            || self.production_credential.len() > 16 * 1024
        {
            return Err(Error::Invalid("secret_shape"));
        }
        Ok(())
    }
}

impl Drop for Secrets {
    fn drop(&mut self) {
        self.signing_seed.zeroize();
        self.preview_credential.zeroize();
        self.production_credential.zeroize();
    }
}

/// A generation uses one atomic Keychain item for its entire secret bundle.
/// Implementations must not log, format or persist unencrypted secret bytes.
pub trait Vault {
    fn load(&self, generation: &str) -> Result<Secrets>;
    /// Return None only when the vault can prove that the item is absent.
    fn load_optional(&self, generation: &str) -> Result<Option<Secrets>> {
        self.load(generation).map(Some)
    }
    fn store(&mut self, generation: &str, secrets: &Secrets) -> Result<()>;
    /// Create a new item without replacing an existing one. Unsupported vaults fail closed.
    fn create(&mut self, _generation: &str, _secrets: &Secrets) -> Result<()> {
        Err(Error::SecretUnavailable)
    }
}

pub struct KeychainVault;

/// Personal application boundary. A poisoned secret store fails closed.
pub(crate) struct ManagedVault(std::sync::Mutex<Box<dyn Vault + Send>>);

impl Default for ManagedVault {
    fn default() -> Self {
        Self(std::sync::Mutex::new(Box::new(KeychainVault)))
    }
}

impl ManagedVault {
    pub(crate) fn with<T>(&self, run: impl FnOnce(&mut dyn Vault) -> Result<T>) -> Result<T> {
        let mut vault = self.0.lock().map_err(|_| Error::SecretUnavailable)?;
        run(vault.as_mut())
    }

    #[cfg(test)]
    pub(crate) fn synthetic(vault: impl Vault + Send + 'static) -> Self {
        Self(std::sync::Mutex::new(Box::new(vault)))
    }
}

impl Vault for KeychainVault {
    fn create(&mut self, generation: &str, secrets: &Secrets) -> Result<()> {
        secrets.validate()?;
        #[cfg(target_os = "macos")]
        {
            use core_foundation::data::CFData;
            use security_framework::item::{ItemAddOptions, ItemAddValue, ItemClass};
            let bytes = zeroize::Zeroizing::new(
                serde_json::to_vec(secrets).map_err(|_| Error::Invalid("secret_encoding"))?,
            );
            if bytes.len() > SECRET_LIMIT {
                return Err(Error::Invalid("secret_size"));
            }
            let mut item = ItemAddOptions::new(ItemAddValue::Data {
                class: ItemClass::generic_password(),
                data: CFData::from_buffer(&bytes),
            });
            item.set_service("com.claude-switcher.personal.publisher")
                .set_account_name(generation);
            item.add().map_err(|_| Error::SecretUnavailable)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = generation;
            Err(Error::SecretUnavailable)
        }
    }

    fn load(&self, generation: &str) -> Result<Secrets> {
        self.load_optional(generation)?
            .ok_or(Error::SecretUnavailable)
    }

    fn load_optional(&self, generation: &str) -> Result<Option<Secrets>> {
        #[cfg(target_os = "macos")]
        {
            // Security.framework's errSecItemNotFound. Other Keychain errors
            // (including access denial) must never be treated as absence.
            const KEYCHAIN_ITEM_NOT_FOUND: i32 = -25300;
            let bytes = match security_framework::passwords::get_generic_password(
                "com.claude-switcher.personal.publisher",
                generation,
            ) {
                Ok(bytes) => zeroize::Zeroizing::new(bytes),
                Err(error) if error.code() == KEYCHAIN_ITEM_NOT_FOUND => return Ok(None),
                Err(_) => return Err(Error::SecretUnavailable),
            };
            let secrets: Secrets =
                switcher_core::signed::strict_json(&bytes, SECRET_LIMIT).map_err(Error::Invalid)?;
            secrets.validate()?;
            Ok(Some(secrets))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = generation;
            Err(Error::SecretUnavailable)
        }
    }

    fn store(&mut self, generation: &str, secrets: &Secrets) -> Result<()> {
        secrets.validate()?;
        #[cfg(target_os = "macos")]
        {
            let bytes = zeroize::Zeroizing::new(
                serde_json::to_vec(secrets).map_err(|_| Error::Invalid("secret_encoding"))?,
            );
            if bytes.len() > SECRET_LIMIT {
                return Err(Error::Invalid("secret_size"));
            }
            security_framework::passwords::set_generic_password(
                "com.claude-switcher.personal.publisher",
                generation,
                &bytes,
            )
            .map_err(|_| Error::SecretUnavailable)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = generation;
            Err(Error::SecretUnavailable)
        }
    }
}

#[cfg(test)]
mod managed_tests {
    use super::*;

    #[test]
    fn poisoned_managed_vault_never_runs_another_secret_operation() {
        let vault = ManagedVault::synthetic(super::super::test_support::MemoryVault::default());
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: Result<()> = vault.with(|_| panic!("synthetic secret-store interruption"));
        }));
        assert!(panic.is_err());
        for _ in 0..2 {
            let result: Result<()> = vault.with(|_| panic!("poisoned vault must fail closed"));
            assert!(matches!(result, Err(Error::SecretUnavailable)));
        }
    }
}
