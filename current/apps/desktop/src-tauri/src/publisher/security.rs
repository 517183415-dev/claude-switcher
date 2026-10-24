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
    fn store(&mut self, generation: &str, secrets: &Secrets) -> Result<()>;
}

pub struct KeychainVault;

impl Vault for KeychainVault {
    fn load(&self, generation: &str) -> Result<Secrets> {
        #[cfg(target_os = "macos")]
        {
            let bytes = zeroize::Zeroizing::new(
                security_framework::passwords::get_generic_password(
                    "com.claude-switcher.personal.publisher",
                    generation,
                )
                .map_err(|_| Error::SecretUnavailable)?,
            );
            let secrets: Secrets =
                switcher_core::signed::strict_json(&bytes, SECRET_LIMIT).map_err(Error::Invalid)?;
            secrets.validate()?;
            Ok(secrets)
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
