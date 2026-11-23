//! Authenticated encryption for the password vault (AES-256-GCM).
//!
//! File layout: `MAGIC | 12-byte random nonce | ciphertext+tag`. A fresh nonce is generated for
//! every write. Any change to the file (or the wrong key) makes `open` fail instead of returning
//! garbage.

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng, Payload},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{anyhow, bail, Result};

pub const KEY_LEN: usize = 32;
pub type VaultKey = [u8; KEY_LEN];

const MAGIC: &[u8; 4] = b"PWV1";
const NONCE_LEN: usize = 12;
/// Binds the ciphertext to its purpose, so it can't be swapped for another encrypted blob.
const AAD: &[u8] = b"poof-password-vault-v1";

pub fn new_key() -> VaultKey {
    Aes256Gcm::generate_key(OsRng).into()
}

pub fn seal(key: &VaultKey, plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let encrypted = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad: AAD,
            },
        )
        .map_err(|_| anyhow!("encryption failed"))?;
    let mut out = Vec::with_capacity(MAGIC.len() + NONCE_LEN + encrypted.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&encrypted);
    Ok(out)
}

pub fn open(key: &VaultKey, sealed: &[u8]) -> Result<Vec<u8>> {
    let header = MAGIC.len() + NONCE_LEN;
    if sealed.len() <= header || &sealed[..MAGIC.len()] != MAGIC {
        bail!("not a password vault");
    }
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(&sealed[MAGIC.len()..header]);
    cipher
        .decrypt(
            nonce,
            Payload {
                msg: &sealed[header..],
                aad: AAD,
            },
        )
        .map_err(|_| anyhow!("the vault could not be decrypted (wrong key or damaged file)"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let key = new_key();
        let sealed = seal(&key, b"hunter2").unwrap();
        assert_eq!(open(&key, &sealed).unwrap(), b"hunter2");
    }

    #[test]
    fn ciphertext_hides_the_plaintext_and_differs_every_time() {
        let key = new_key();
        let a = seal(&key, b"correct horse battery staple").unwrap();
        let b = seal(&key, b"correct horse battery staple").unwrap();
        assert_ne!(a, b);
        assert!(!a.windows(7).any(|w| w == b"battery"));
    }

    #[test]
    fn wrong_key_and_tampering_are_rejected() {
        let sealed = seal(&new_key(), b"secret").unwrap();
        assert!(open(&new_key(), &sealed).is_err());

        let key = new_key();
        let mut sealed = seal(&key, b"secret").unwrap();
        let last = sealed.len() - 1;
        sealed[last] ^= 1;
        assert!(open(&key, &sealed).is_err());
        assert!(open(&key, b"short").is_err());
    }
}
