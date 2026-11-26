//! Saved logins of a Chromium-based browser (Arc): the `Login Data` SQLite file, and the
//! AES-256-GCM key that protects each password, which `Local State` stores under DPAPI.

use super::{dpapi, Login};
use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::{fs, path::Path};

const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 12;
/// Chromium marks DPAPI-protected keys with this prefix before the protected bytes.
const DPAPI_PREFIX: &[u8] = b"DPAPI";
/// Logins read at most from one profile; the vault holds far fewer anyway.
const MAX_ROWS: usize = 10_000;

pub struct LoginReport {
    pub logins: Vec<Login>,
    /// Passwords in an encryption this importer cannot open (app-bound, `v20`).
    pub unsupported: usize,
}

enum Decrypted {
    Text(String),
    Unsupported,
    Failed,
}

/// The key every `v10`/`v11` password in this browser is encrypted with.
pub fn master_key(user_data: &Path) -> Result<Vec<u8>> {
    let text = fs::read_to_string(user_data.join("Local State"))
        .context("Arc's Local State file could not be read")?;
    let state: Value = serde_json::from_str(&text)?;
    let encoded = state["os_crypt"]["encrypted_key"]
        .as_str()
        .ok_or_else(|| anyhow!("Arc has no saved-password key"))?;
    let blob = base64::engine::general_purpose::STANDARD.decode(encoded)?;
    let protected = blob
        .strip_prefix(DPAPI_PREFIX)
        .ok_or_else(|| anyhow!("Arc's password key is in an unknown format"))?;
    let key = dpapi::unprotect(protected)?;
    if key.len() != KEY_LEN {
        bail!("Arc's password key has an unexpected size");
    }
    Ok(key)
}

pub fn read_logins(user_data: &Path, profile_dir: &str, key: &[u8]) -> Result<LoginReport> {
    let source = user_data.join(profile_dir).join("Login Data");
    let copy = TempCopy::of(&source)?;
    let db = Connection::open_with_flags(&copy.0, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .context("Arc's password file could not be opened")?;
    let mut statement = db.prepare(
        "SELECT origin_url, username_value, password_value FROM logins \
         WHERE blacklisted_by_user = 0 LIMIT ?1",
    )?;
    let rows = statement.query_map([MAX_ROWS as i64], |row| {
        Ok((
            row.get::<_, Option<String>>(0)?.unwrap_or_default(),
            row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            row.get::<_, Option<Vec<u8>>>(2)?.unwrap_or_default(),
        ))
    })?;

    let mut report = LoginReport {
        logins: Vec::new(),
        unsupported: 0,
    };
    for (url, username, blob) in rows.flatten() {
        if blob.is_empty() {
            continue;
        }
        match decrypt(key, &blob) {
            Decrypted::Text(password) if !password.is_empty() => report.logins.push(Login {
                url,
                username,
                password,
            }),
            Decrypted::Unsupported => report.unsupported += 1,
            _ => {}
        }
    }
    Ok(report)
}

/// `v10`/`v11`: 3-byte version, 12-byte nonce, ciphertext with its 16-byte tag.
/// No prefix: the password is a bare DPAPI blob (very old profiles).
fn decrypt(key: &[u8], blob: &[u8]) -> Decrypted {
    if blob.starts_with(b"v10") || blob.starts_with(b"v11") {
        let body = &blob[3..];
        if body.len() <= NONCE_LEN {
            return Decrypted::Failed;
        }
        let (nonce, sealed) = body.split_at(NONCE_LEN);
        let Ok(cipher) = Aes256Gcm::new_from_slice(key) else {
            return Decrypted::Failed;
        };
        return match cipher
            .decrypt(Nonce::from_slice(nonce), sealed)
            .ok()
            .and_then(|plain| String::from_utf8(plain).ok())
        {
            Some(text) => Decrypted::Text(text),
            None => Decrypted::Failed,
        };
    }
    if blob.starts_with(b"v2") {
        return Decrypted::Unsupported;
    }
    match dpapi::unprotect(blob)
        .ok()
        .and_then(|plain| String::from_utf8(plain).ok())
    {
        Some(text) => Decrypted::Text(text),
        None => Decrypted::Failed,
    }
}

/// A private copy of the database: Arc keeps the original locked while it runs, and reading
/// must never touch the browser's own file.
struct TempCopy(std::path::PathBuf);

impl TempCopy {
    fn of(source: &Path) -> Result<Self> {
        let path = std::env::temp_dir().join(format!("poof-import-{}.db", crate::state::new_id()));
        fs::copy(source, &path).map_err(|e| {
            anyhow!(
                "Arc's saved passwords could not be read ({e}). Quit Arc completely and try again."
            )
        })?;
        Ok(Self(path))
    }
}

impl Drop for TempCopy {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seal(key: &[u8], password: &str) -> Vec<u8> {
        let nonce = [7u8; NONCE_LEN];
        let cipher = Aes256Gcm::new_from_slice(key).unwrap();
        let sealed = cipher
            .encrypt(Nonce::from_slice(&nonce), password.as_bytes())
            .unwrap();
        [b"v10".as_slice(), &nonce, &sealed].concat()
    }

    #[test]
    fn opens_chromium_v10_passwords() {
        let key = [3u8; KEY_LEN];
        let blob = seal(&key, "hunter2");
        assert!(matches!(decrypt(&key, &blob), Decrypted::Text(p) if p == "hunter2"));
    }

    #[test]
    fn a_wrong_key_or_damaged_blob_is_a_failure_not_garbage() {
        let blob = seal(&[3u8; KEY_LEN], "hunter2");
        assert!(matches!(decrypt(&[4u8; KEY_LEN], &blob), Decrypted::Failed));
        let mut damaged = blob.clone();
        *damaged.last_mut().unwrap() ^= 1;
        assert!(matches!(
            decrypt(&[3u8; KEY_LEN], &damaged),
            Decrypted::Failed
        ));
        assert!(matches!(
            decrypt(&[3u8; KEY_LEN], b"v10short"),
            Decrypted::Failed
        ));
    }

    #[test]
    fn app_bound_passwords_are_counted_as_unsupported() {
        assert!(matches!(
            decrypt(&[3u8; KEY_LEN], b"v20 whatever"),
            Decrypted::Unsupported
        ));
    }

    #[test]
    fn reads_rows_from_a_login_data_file() {
        let dir = std::env::temp_dir().join(format!("poof-ld-{}", crate::state::new_id()));
        fs::create_dir_all(dir.join("Default")).unwrap();
        let key = [9u8; KEY_LEN];
        let db = Connection::open(dir.join("Default").join("Login Data")).unwrap();
        db.execute_batch(
            "CREATE TABLE logins (origin_url TEXT, username_value TEXT, password_value BLOB, blacklisted_by_user INTEGER);",
        )
        .unwrap();
        let insert = "INSERT INTO logins VALUES (?1, ?2, ?3, ?4)";
        db.execute(
            insert,
            rusqlite::params!["https://a.com/login", "ann", seal(&key, "pw1"), 0],
        )
        .unwrap();
        db.execute(
            insert,
            rusqlite::params!["https://never.com/", "", Vec::<u8>::new(), 1],
        )
        .unwrap();
        db.execute(
            insert,
            rusqlite::params!["https://b.com/", "bob", b"v20xxxx".to_vec(), 0],
        )
        .unwrap();
        drop(db);

        let report = read_logins(&dir, "Default", &key).unwrap();
        assert_eq!(report.logins.len(), 1);
        assert_eq!(
            (
                report.logins[0].url.as_str(),
                report.logins[0].username.as_str(),
                report.logins[0].password.as_str()
            ),
            ("https://a.com/login", "ann", "pw1")
        );
        assert_eq!(report.unsupported, 1);
        let _ = fs::remove_dir_all(dir);
    }
}
