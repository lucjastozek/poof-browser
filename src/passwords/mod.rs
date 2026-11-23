//! Saved logins, encrypted on disk.
//!
//! * The vault is one file (`passwords.vault`) holding every profile's credentials, encrypted
//!   with AES-256-GCM (`crypto.rs`).
//! * The encryption key is random and lives in the operating system's credential store
//!   (Windows Credential Manager, macOS Keychain, Secret Service on Linux), never next to the
//!   file. Copying the vault to another machine or user account does not expose it.
//! * Nothing is touched until the first save or lookup, so users who never save a password pay
//!   nothing at startup. Decrypted data is not cached: each operation reads, uses and drops it.
//!   Only the set of sites that have logins is kept in memory.
//! * The store never overwrites a vault it cannot decrypt, and never invents a new key for an
//!   existing vault, so a missing key can't silently destroy saved passwords.

mod crypto;
mod origin;
mod vault;

pub use origin::origin_of;
pub use vault::{Credential, Offer};

use anyhow::{anyhow, bail, Result};
use crypto::{VaultKey, KEY_LEN};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use vault::Vault;

/// Where the vault key is kept. The operating system's credential store in production.
pub trait KeyProvider: Send {
    fn get(&self) -> Result<Option<Vec<u8>>>;
    fn set(&self, key: &[u8]) -> Result<()>;
}

/// The operating system's credential store (via the `keyring` crate).
struct OsKeyring;

impl OsKeyring {
    fn entry() -> Result<keyring::Entry> {
        Ok(keyring::Entry::new("poof", "password-vault-key")?)
    }
}

impl KeyProvider for OsKeyring {
    fn get(&self) -> Result<Option<Vec<u8>>> {
        match Self::entry()?.get_secret() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn set(&self, key: &[u8]) -> Result<()> {
        Ok(Self::entry()?.set_secret(key)?)
    }
}

pub struct PasswordStore {
    path: PathBuf,
    provider: Box<dyn KeyProvider>,
    key: Option<VaultKey>,
    /// `(profile, origin)` pairs that have credentials; `None` until first needed.
    origins: Option<HashSet<(String, String)>>,
}

impl PasswordStore {
    /// The vault inside the browser's data folder, keyed by the OS credential store.
    pub fn open(data_dir: &Path) -> Self {
        Self::with_provider(data_dir.join("passwords.vault"), Box::new(OsKeyring))
    }

    pub fn with_provider(path: PathBuf, provider: Box<dyn KeyProvider>) -> Self {
        Self {
            path,
            provider,
            key: None,
            origins: None,
        }
    }

    /// Cheap check used on every page with a login form: no decryption after the first call.
    pub fn has_credentials(&mut self, profile_id: &str, origin: &str) -> bool {
        match self.origin_index() {
            Ok(index) => index.contains(&(profile_id.to_string(), origin.to_string())),
            Err(error) => {
                log::warn!("password vault unavailable: {error:#}");
                false
            }
        }
    }

    /// Newest first.
    pub fn credentials_for(&mut self, profile_id: &str, origin: &str) -> Result<Vec<Credential>> {
        let vault = self.read()?;
        Ok(vault
            .for_origin(profile_id, origin)
            .into_iter()
            .cloned()
            .collect())
    }

    pub fn list(&mut self, profile_id: &str) -> Result<Vec<Credential>> {
        let vault = self.read()?;
        Ok(vault.for_profile(profile_id).into_iter().cloned().collect())
    }

    pub fn get(&mut self, id: &str) -> Result<Option<Credential>> {
        Ok(self.read()?.get(id).cloned())
    }

    pub fn offer(
        &mut self,
        profile_id: &str,
        origin: &str,
        username: &str,
        password: &str,
    ) -> Result<Offer> {
        Ok(self.read()?.offer(profile_id, origin, username, password))
    }

    pub fn save(
        &mut self,
        profile_id: &str,
        origin: &str,
        username: &str,
        password: &str,
    ) -> Result<()> {
        let mut vault = self.read()?;
        if !vault.upsert(profile_id, origin, username, password, now_secs()) {
            bail!("the password vault is full");
        }
        self.write(&vault)
    }

    pub fn delete(&mut self, id: &str) -> Result<bool> {
        let mut vault = self.read()?;
        let removed = vault.remove(id);
        if removed {
            self.write(&vault)?;
        }
        Ok(removed)
    }

    pub fn never_save(&mut self, profile_id: &str, origin: &str) -> Result<()> {
        let mut vault = self.read()?;
        vault.add_never(profile_id, origin);
        self.write(&vault)
    }

    fn origin_index(&mut self) -> Result<&HashSet<(String, String)>> {
        if self.origins.is_none() {
            let vault = self.read()?;
            self.origins = Some(vault.origins().collect());
        }
        Ok(self.origins.as_ref().expect("index was just built"))
    }

    /// The key from the credential store. A key is created only when there is no vault yet.
    fn key(&mut self) -> Result<VaultKey> {
        if let Some(key) = self.key {
            return Ok(key);
        }
        let key = match self.provider.get()? {
            Some(bytes) => <VaultKey>::try_from(bytes.as_slice()).map_err(|_| {
                anyhow!("the stored vault key has the wrong length (expected {KEY_LEN} bytes)")
            })?,
            None if self.path.exists() => {
                bail!("the password vault exists but its key is missing from the credential store")
            }
            None => {
                let key = crypto::new_key();
                self.provider.set(&key)?;
                key
            }
        };
        self.key = Some(key);
        Ok(key)
    }

    fn read(&mut self) -> Result<Vault> {
        let sealed = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Vault::default())
            }
            Err(error) => return Err(error.into()),
        };
        let key = self.key()?;
        let plain = crypto::open(&key, &sealed)?;
        Ok(serde_json::from_slice(&plain)?)
    }

    fn write(&mut self, vault: &Vault) -> Result<()> {
        let key = self.key()?;
        let sealed = crypto::seal(&key, &serde_json::to_vec(vault)?)?;
        write_atomically(&self.path, &sealed)?;
        self.origins = Some(vault.origins().collect());
        Ok(())
    }
}

/// Temp file + rename, so a crash never leaves a half-written vault.
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("vault.tmp");
    fs::write(&tmp, bytes)?;
    if path.exists() {
        let _ = fs::copy(path, path.with_extension("vault.bak"));
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// A credential store held in memory, shared so a test can "restart" with the same key.
    #[derive(Clone, Default)]
    struct MemoryKeys(Arc<Mutex<Option<Vec<u8>>>>);

    impl KeyProvider for MemoryKeys {
        fn get(&self) -> Result<Option<Vec<u8>>> {
            Ok(self.0.lock().unwrap().clone())
        }
        fn set(&self, key: &[u8]) -> Result<()> {
            *self.0.lock().unwrap() = Some(key.to_vec());
            Ok(())
        }
    }

    fn temp_vault() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("poof-pw-test-{}", crate::state::new_id()));
        fs::create_dir_all(&dir).unwrap();
        dir.join("passwords.vault")
    }

    fn store(path: &Path, keys: &MemoryKeys) -> PasswordStore {
        PasswordStore::with_provider(path.to_path_buf(), Box::new(keys.clone()))
    }

    const SITE: &str = "https://example.com";

    #[test]
    fn saved_logins_survive_a_restart() {
        let (path, keys) = (temp_vault(), MemoryKeys::default());
        store(&path, &keys)
            .save("p", SITE, "ann", "s3cret")
            .unwrap();

        let mut reopened = store(&path, &keys);
        assert!(reopened.has_credentials("p", SITE));
        assert!(!reopened.has_credentials("p", "https://other.com"));
        let found = reopened.credentials_for("p", SITE).unwrap();
        assert_eq!(
            (found[0].username.as_str(), found[0].password.as_str()),
            ("ann", "s3cret")
        );
    }

    #[test]
    fn the_file_on_disk_contains_no_plaintext() {
        let (path, keys) = (temp_vault(), MemoryKeys::default());
        store(&path, &keys)
            .save("p", SITE, "ann", "s3cret-passw0rd")
            .unwrap();
        let bytes = fs::read(&path).unwrap();
        for needle in ["s3cret-passw0rd", "ann", "example.com"] {
            assert!(
                !bytes.windows(needle.len()).any(|w| w == needle.as_bytes()),
                "{needle}"
            );
        }
    }

    #[test]
    fn a_missing_key_never_overwrites_an_existing_vault() {
        let (path, keys) = (temp_vault(), MemoryKeys::default());
        store(&path, &keys).save("p", SITE, "ann", "pw").unwrap();
        let before = fs::read(&path).unwrap();

        let mut lost_key = store(&path, &MemoryKeys::default());
        assert!(lost_key.save("p", SITE, "bob", "pw2").is_err());
        assert!(lost_key.list("p").is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
    }

    #[test]
    fn a_damaged_vault_is_refused_not_replaced() {
        let (path, keys) = (temp_vault(), MemoryKeys::default());
        store(&path, &keys).save("p", SITE, "ann", "pw").unwrap();
        let mut bytes = fs::read(&path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        fs::write(&path, &bytes).unwrap();

        let mut s = store(&path, &keys);
        assert!(s.save("p", SITE, "bob", "pw2").is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }

    #[test]
    fn untouched_store_creates_no_file_and_no_key() {
        let (path, keys) = (temp_vault(), MemoryKeys::default());
        let mut s = store(&path, &keys);
        assert!(!s.has_credentials("p", SITE));
        assert!(s.list("p").unwrap().is_empty());
        assert!(!path.exists());
        assert!(keys.get().unwrap().is_none());
    }

    #[test]
    fn delete_and_never_save_persist() {
        let (path, keys) = (temp_vault(), MemoryKeys::default());
        let mut s = store(&path, &keys);
        s.save("p", SITE, "ann", "pw").unwrap();
        s.never_save("p", "https://bank.com").unwrap();
        let id = s.list("p").unwrap()[0].id.clone();
        assert!(s.delete(&id).unwrap());

        let mut reopened = store(&path, &keys);
        assert!(!reopened.has_credentials("p", SITE));
        assert_eq!(
            reopened.offer("p", "https://bank.com", "a", "b").unwrap(),
            Offer::Skip
        );
    }
}
