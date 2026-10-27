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

#[derive(Clone, Default)]
pub struct KeychainVault {
    #[cfg(target_os = "macos")]
    keychain: Option<security_framework::os::macos::keychain::SecKeychain>,
}

#[cfg(target_os = "macos")]
const KEYCHAIN_SERVICE: &str = "com.claude-switcher.personal.publisher";

impl KeychainVault {
    /// Explicit location is available only to native tests, never to product IPC.
    #[cfg(all(test, target_os = "macos"))]
    fn isolated(keychain: security_framework::os::macos::keychain::SecKeychain) -> Self {
        Self {
            keychain: Some(keychain),
        }
    }

    #[cfg(target_os = "macos")]
    fn encode(secrets: &Secrets) -> Result<zeroize::Zeroizing<Vec<u8>>> {
        let bytes = zeroize::Zeroizing::new(
            serde_json::to_vec(secrets).map_err(|_| Error::Invalid("secret_encoding"))?,
        );
        if bytes.len() > SECRET_LIMIT {
            return Err(Error::Invalid("secret_size"));
        }
        Ok(bytes)
    }
}

/// Personal application boundary. A poisoned secret store fails closed.
pub(crate) struct ManagedVault(std::sync::Mutex<Box<dyn Vault + Send>>);

impl Default for ManagedVault {
    fn default() -> Self {
        Self(std::sync::Mutex::new(Box::new(KeychainVault::default())))
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
            let bytes = Self::encode(secrets)?;
            let mut item = ItemAddOptions::new(ItemAddValue::Data {
                class: ItemClass::generic_password(),
                data: CFData::from_buffer(&bytes),
            });
            item.set_service(KEYCHAIN_SERVICE)
                .set_account_name(generation);
            if let Some(keychain) = &self.keychain {
                item.set_location(security_framework::item::Location::FileKeychain(
                    keychain.clone(),
                ));
            }
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
            let found = if let Some(keychain) = &self.keychain {
                security_framework::os::macos::passwords::find_generic_password(
                    Some(std::slice::from_ref(keychain)),
                    KEYCHAIN_SERVICE,
                    generation,
                )
                .map(|(bytes, _)| bytes.to_vec())
            } else {
                security_framework::passwords::get_generic_password(KEYCHAIN_SERVICE, generation)
            };
            let bytes = match found {
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
            let bytes = Self::encode(secrets)?;
            if let Some(keychain) = &self.keychain {
                keychain.set_generic_password(KEYCHAIN_SERVICE, generation, &bytes)
            } else {
                security_framework::passwords::set_generic_password(
                    KEYCHAIN_SERVICE,
                    generation,
                    &bytes,
                )
            }
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

#[cfg(all(test, target_os = "macos"))]
#[path = "native_keychain_tests.rs"]
mod native_keychain_tests;
