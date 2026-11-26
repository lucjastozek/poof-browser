//! The decrypted contents of the vault and the rules for changing them. No I/O here.

use crate::state::new_id;
use serde::{Deserialize, Serialize};

/// Credentials kept at most. Far beyond normal use; stops a runaway page from filling the disk.
const MAX_CREDENTIALS: usize = 5000;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Credential {
    pub id: String,
    pub profile_id: String,
    pub origin: String,
    pub username: String,
    pub password: String,
    /// Unix seconds of the last save, so the newest credential is filled first.
    pub updated: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct NeverSave {
    profile_id: String,
    origin: String,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Vault {
    #[serde(default)]
    credentials: Vec<Credential>,
    /// Sites the user said "never save passwords for".
    #[serde(default)]
    never: Vec<NeverSave>,
}

/// What to do with a login the user just submitted.
#[derive(Debug, PartialEq)]
pub enum Offer {
    /// Already saved unchanged, or the user declined this site.
    Skip,
    New,
    /// Same username, different password: the id of the credential to update.
    Update(String),
}

/// Result of `Vault::add_if_new`.
#[derive(Debug, PartialEq)]
pub enum Added {
    Yes,
    AlreadySaved,
    VaultFull,
}

impl Vault {
    /// Credentials for one site, newest first.
    pub fn for_origin(&self, profile_id: &str, origin: &str) -> Vec<&Credential> {
        let mut found: Vec<&Credential> = self
            .credentials
            .iter()
            .filter(|c| c.profile_id == profile_id && c.origin == origin)
            .collect();
        found.sort_by_key(|c| std::cmp::Reverse(c.updated));
        found
    }

    pub fn for_profile(&self, profile_id: &str) -> Vec<&Credential> {
        self.credentials
            .iter()
            .filter(|c| c.profile_id == profile_id)
            .collect()
    }

    pub fn get(&self, id: &str) -> Option<&Credential> {
        self.credentials.iter().find(|c| c.id == id)
    }

    pub fn offer(&self, profile_id: &str, origin: &str, username: &str, password: &str) -> Offer {
        if self.is_never(profile_id, origin) {
            return Offer::Skip;
        }
        match self
            .for_origin(profile_id, origin)
            .into_iter()
            .find(|c| c.username == username)
        {
            None => Offer::New,
            Some(c) if c.password == password => Offer::Skip,
            Some(c) => Offer::Update(c.id.clone()),
        }
    }

    /// Adds the credential, or updates the password of the same username on the same site.
    /// Returns false if the vault is full.
    pub fn upsert(
        &mut self,
        profile_id: &str,
        origin: &str,
        username: &str,
        password: &str,
        now: u64,
    ) -> bool {
        let existing = self
            .credentials
            .iter_mut()
            .find(|c| c.profile_id == profile_id && c.origin == origin && c.username == username);
        if let Some(c) = existing {
            c.password = password.to_string();
            c.updated = now;
            return true;
        }
        if self.credentials.len() >= MAX_CREDENTIALS {
            return false;
        }
        self.credentials.push(Credential {
            id: new_id(),
            profile_id: profile_id.into(),
            origin: origin.into(),
            username: username.into(),
            password: password.into(),
            updated: now,
        });
        true
    }

    /// Adds a login unless the same username is already saved for that site: an import never
    /// replaces a password the user has here.
    pub fn add_if_new(
        &mut self,
        profile_id: &str,
        origin: &str,
        username: &str,
        password: &str,
        now: u64,
    ) -> Added {
        let known = self
            .credentials
            .iter()
            .any(|c| c.profile_id == profile_id && c.origin == origin && c.username == username);
        if known {
            return Added::AlreadySaved;
        }
        if self.upsert(profile_id, origin, username, password, now) {
            Added::Yes
        } else {
            Added::VaultFull
        }
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.credentials.len();
        self.credentials.retain(|c| c.id != id);
        self.credentials.len() != before
    }

    pub fn is_never(&self, profile_id: &str, origin: &str) -> bool {
        self.never
            .iter()
            .any(|n| n.profile_id == profile_id && n.origin == origin)
    }

    pub fn add_never(&mut self, profile_id: &str, origin: &str) {
        if !self.is_never(profile_id, origin) {
            self.never.push(NeverSave {
                profile_id: profile_id.into(),
                origin: origin.into(),
            });
        }
    }

    /// `(profile, origin)` of every saved credential; lets the browser skip pages without logins
    /// without decrypting anything.
    pub fn origins(&self) -> impl Iterator<Item = (String, String)> + '_ {
        self.credentials
            .iter()
            .map(|c| (c.profile_id.clone(), c.origin.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SITE: &str = "https://example.com";

    #[test]
    fn upsert_adds_then_updates_the_same_login() {
        let mut v = Vault::default();
        assert!(v.upsert("p", SITE, "ann", "one", 1));
        assert!(v.upsert("p", SITE, "ann", "two", 2));
        assert_eq!(v.for_origin("p", SITE).len(), 1);
        assert_eq!(v.for_origin("p", SITE)[0].password, "two");
    }

    #[test]
    fn credentials_are_isolated_by_profile_and_origin() {
        let mut v = Vault::default();
        v.upsert("p1", SITE, "ann", "x", 1);
        assert!(v.for_origin("p2", SITE).is_empty());
        assert!(v.for_origin("p1", "https://example.com:8443").is_empty());
        assert!(v.for_origin("p1", "https://sub.example.com").is_empty());
    }

    #[test]
    fn newest_credential_comes_first() {
        let mut v = Vault::default();
        v.upsert("p", SITE, "old", "x", 1);
        v.upsert("p", SITE, "new", "y", 9);
        assert_eq!(v.for_origin("p", SITE)[0].username, "new");
    }

    #[test]
    fn offers_depend_on_what_is_saved() {
        let mut v = Vault::default();
        assert_eq!(v.offer("p", SITE, "ann", "pw"), Offer::New);
        v.upsert("p", SITE, "ann", "pw", 1);
        assert_eq!(v.offer("p", SITE, "ann", "pw"), Offer::Skip);
        let id = v.for_origin("p", SITE)[0].id.clone();
        assert_eq!(v.offer("p", SITE, "ann", "changed"), Offer::Update(id));
        assert_eq!(v.offer("p", SITE, "bob", "pw"), Offer::New);
    }

    #[test]
    fn never_save_silences_offers_for_that_site_only() {
        let mut v = Vault::default();
        v.add_never("p", SITE);
        assert_eq!(v.offer("p", SITE, "ann", "pw"), Offer::Skip);
        assert_eq!(v.offer("p", "https://other.com", "ann", "pw"), Offer::New);
        assert_eq!(v.offer("q", SITE, "ann", "pw"), Offer::New);
    }

    #[test]
    fn add_if_new_never_replaces_an_existing_password() {
        let mut v = Vault::default();
        assert_eq!(v.add_if_new("p", SITE, "ann", "first", 1), Added::Yes);
        assert_eq!(
            v.add_if_new("p", SITE, "ann", "other", 2),
            Added::AlreadySaved
        );
        assert_eq!(v.add_if_new("p", SITE, "bob", "pw", 2), Added::Yes);
        assert_eq!(v.add_if_new("q", SITE, "ann", "pw", 2), Added::Yes);
        assert_eq!(v.for_origin("p", SITE).len(), 2);
        let ann = v
            .for_origin("p", SITE)
            .into_iter()
            .find(|c| c.username == "ann");
        assert_eq!(ann.unwrap().password, "first");
    }

    #[test]
    fn remove_deletes_by_id() {
        let mut v = Vault::default();
        v.upsert("p", SITE, "ann", "pw", 1);
        let id = v.for_origin("p", SITE)[0].id.clone();
        assert!(v.remove(&id));
        assert!(!v.remove(&id));
        assert!(v.get(&id).is_none());
    }
}
