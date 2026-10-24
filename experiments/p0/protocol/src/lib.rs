//! P0-only protocol experiment. No HTTP, filesystem, installer, or account access.
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

pub mod ads;
pub mod publication;
pub mod update;

pub type Result<T> = std::result::Result<T, &'static str>;

#[derive(Clone)]
pub struct PinnedKey {
    pub id: &'static str,
    pub public: VerifyingKey,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    // A JSON string transports the exact UTF-8 signed payload bytes.
    pub payload: String,
    pub signature: Vec<u8>,
}

pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn verify<T: DeserializeOwned>(wire: &[u8], key: &PinnedKey) -> Result<(T, String)> {
    let envelope: Envelope = serde_json::from_slice(wire).map_err(|_| "envelope_json")?;
    let signature = Signature::from_slice(&envelope.signature).map_err(|_| "signature_shape")?;
    key.public
        .verify_strict(envelope.payload.as_bytes(), &signature)
        .map_err(|_| "signature")?;
    let body = serde_json::from_str(&envelope.payload).map_err(|_| "payload_json")?;
    Ok((body, digest(envelope.payload.as_bytes())))
}

pub fn valid_time(issued: u64, expires: u64, now: u64, trusted: bool) -> Result<()> {
    if !trusted { return Err("clock_untrusted"); }
    if issued > now || expires <= now || expires <= issued { return Err("time"); }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Edition { Personal, Public }

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Sequence {
    pub highest: u64,
    pub payload_hash: Option<String>,
}

impl Sequence {
    pub fn check(&self, sequence: u64, hash: &str) -> Result<()> {
        if sequence == 0 || sequence < self.highest { return Err("old_sequence"); }
        if sequence == self.highest && self.payload_hash.as_deref() != Some(hash) {
            return Err("sequence_conflict");
        }
        Ok(())
    }
    pub fn remember(&mut self, sequence: u64, hash: String) {
        self.highest = sequence;
        self.payload_hash = Some(hash);
    }
}

#[cfg(test)]
pub mod fixture {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    // PUBLIC TEST FIXTURES. These fixed bytes are deliberately not production secrets.
    pub const UPDATE_SEED: [u8; 32] = [0x11; 32];
    pub const AD_SEED: [u8; 32] = [0x22; 32];
    pub fn key(seed: &[u8; 32], id: &'static str) -> PinnedKey {
        PinnedKey { id, public: SigningKey::from_bytes(seed).verifying_key() }
    }
    pub fn sign_raw(payload: &str, seed: &[u8; 32]) -> Vec<u8> {
        let signature = SigningKey::from_bytes(seed).sign(payload.as_bytes());
        serde_json::to_vec(&Envelope { payload: payload.to_owned(), signature: signature.to_bytes().to_vec() }).unwrap()
    }
    pub fn sign<T: Serialize>(value: &T, seed: &[u8; 32]) -> Vec<u8> {
        sign_raw(&serde_json::to_string(value).unwrap(), seed)
    }
}
