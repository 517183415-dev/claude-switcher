//! Bounded password backup using Argon2id and XChaCha20-Poly1305.
use super::{Error, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, OsRng, Payload, rand_core::RngCore},
};
use zeroize::Zeroizing;

pub const LIMIT: usize = 64 * 1024 * 1024;
const HEADER: usize = 8 + 4 + 4 + 4 + 16 + 24;
const MAGIC: &[u8; 8] = b"CSADBK01";
const MEMORY_KIB: u32 = 64 * 1024;
const ITERATIONS: u32 = 3;
const LANES: u32 = 1;

fn key(passphrase: &[u8], salt: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
    if passphrase.len() < 12 || passphrase.len() > 1024 {
        return Err(Error::Invalid("backup_passphrase_length"));
    }
    let params = Params::new(MEMORY_KIB, ITERATIONS, LANES, Some(32))
        .map_err(|_| Error::Invalid("backup_kdf"))?;
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(passphrase, salt, &mut *key)
        .map_err(|_| Error::Invalid("backup_kdf"))?;
    Ok(key)
}

pub fn seal(plaintext: &[u8], passphrase: &[u8]) -> Result<Vec<u8>> {
    if plaintext.len() > LIMIT - HEADER - 16 {
        return Err(Error::Invalid("backup_size"));
    }
    let mut salt = [0; 16];
    let mut nonce = [0; 24];
    OsRng.try_fill_bytes(&mut salt).map_err(|_| Error::Random)?;
    OsRng
        .try_fill_bytes(&mut nonce)
        .map_err(|_| Error::Random)?;
    let mut header = Vec::with_capacity(HEADER);
    header.extend_from_slice(MAGIC);
    for parameter in [MEMORY_KIB, ITERATIONS, LANES] {
        header.extend_from_slice(&parameter.to_be_bytes());
    }
    header.extend_from_slice(&salt);
    header.extend_from_slice(&nonce);
    let key = key(passphrase, &salt)?;
    let cipher = XChaCha20Poly1305::new((&*key).into());
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: &header,
            },
        )
        .map_err(|_| Error::Invalid("backup_encrypt"))?;
    header.extend_from_slice(&ciphertext);
    Ok(header)
}

pub fn open(bytes: &[u8], passphrase: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    if bytes.len() < HEADER + 16 || bytes.len() > LIMIT || &bytes[..8] != MAGIC {
        return Err(Error::Invalid("backup_header"));
    }
    // Exact supported parameters prevent unauthenticated memory/CPU amplification.
    for (offset, expected) in [(8, MEMORY_KIB), (12, ITERATIONS), (16, LANES)] {
        if bytes[offset..offset + 4] != expected.to_be_bytes() {
            return Err(Error::Invalid("backup_kdf_parameters"));
        }
    }
    let key = key(passphrase, &bytes[20..36])?;
    let cipher = XChaCha20Poly1305::new((&*key).into());
    cipher
        .decrypt(
            XNonce::from_slice(&bytes[36..60]),
            Payload {
                msg: &bytes[HEADER..],
                aad: &bytes[..HEADER],
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| Error::Invalid("backup_password_or_integrity"))
}
