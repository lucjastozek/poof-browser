//! Password manager glue: login forms reported by pages, the "save password?" offer in the
//! sidebar, autofill, and the saved-passwords list. Storage and encryption live in `crate::passwords`.
//!
//! Pages are untrusted. A page only says "a login form is here" or "the user submitted these
//! values"; which site that is always comes from the tab's real address, never from the page.

use super::*;
use crate::passwords::{origin_of, Offer};

/// Longest username or password accepted from a page.
const FIELD_MAX: usize = 1024;
const FILL_JS: &str = include_str!("js/password-fill.js");

/// A login waiting for the user to say yes or no. Held only until they decide.
pub(super) struct PasswordOffer {
    profile_id: String,
    origin: String,
    username: String,
    password: String,
}

impl Browser {
    /// The profile and exact site of a tab, if passwords may be used there.
    fn password_site(&self, tab: &str) -> Option<(String, String)> {
        let tab = self.tabs.iter().find(|t| t.id == tab)?;
        Some((tab.profile_id.clone(), origin_of(&tab.url)?))
    }

    /// The user submitted a login form: offer to save it, unless it is already saved or declined.
    pub(super) fn on_page_password_submit(
        &mut self,
        tab: &str,
        username: String,
        password: String,
    ) {
        if !self.p.save_passwords || tab.is_empty() {
            return;
        }
        if password.is_empty() || password.len() > FIELD_MAX || username.len() > FIELD_MAX {
            return;
        }
        let Some((profile_id, origin)) = self.password_site(tab) else {
            return;
        };
        let offer = self
            .passwords
            .offer(&profile_id, &origin, &username, &password);
        let updates = match offer {
            Ok(Offer::New) => false,
            Ok(Offer::Update(_)) => true,
            Ok(Offer::Skip) => return,
            Err(error) => {
                log::warn!("password vault: {error:#}");
                return;
            }
        };
        self.call_sidebar(
            "__passwordOffer",
            &json!({ "origin": origin, "username": username, "update": updates }),
        );
        self.password_offer = Some(PasswordOffer {
            profile_id,
            origin,
            username,
            password,
        });
    }

    pub(super) fn accept_password_offer(&mut self) {
        let Some(offer) = self.password_offer.take() else {
            return;
        };
        let saved = self.passwords.save(
            &offer.profile_id,
            &offer.origin,
            &offer.username,
            &offer.password,
        );
        match saved {
            Ok(()) => self.call_sidebar("__toast", &json!("Password saved")),
            Err(error) => {
                log::warn!("saving password failed: {error:#}");
                self.call_sidebar("__toast", &json!("Couldn't save the password"));
            }
        }
        self.call_sidebar("__passwordOffer", &Value::Null);
    }

    pub(super) fn dismiss_password_offer(&mut self, never: bool) {
        if let Some(offer) = self.password_offer.take() {
            if never {
                if let Err(error) = self.passwords.never_save(&offer.profile_id, &offer.origin) {
                    log::warn!("password vault: {error:#}");
                }
            }
        }
        self.call_sidebar("__passwordOffer", &Value::Null);
    }

    /// A page has a login form: fill the newest saved login for this exact site.
    pub(super) fn on_page_password_form(&mut self, tab: &str) {
        if !self.p.save_passwords || tab.is_empty() {
            return;
        }
        let Some((profile_id, origin)) = self.password_site(tab) else {
            return;
        };
        if !self.passwords.has_credentials(&profile_id, &origin) {
            return;
        }
        let login = match self.passwords.credentials_for(&profile_id, &origin) {
            Ok(mut found) if !found.is_empty() => found.swap_remove(0),
            Ok(_) => return,
            Err(error) => {
                log::warn!("password vault: {error:#}");
                return;
            }
        };
        let script = format!(
            "({FILL_JS})({},{},{})",
            json!(origin),
            json!(login.username),
            json!(login.password)
        );
        if let Some(wv) = self
            .tabs
            .iter()
            .find(|t| t.id == tab)
            .and_then(|t| t.webview.as_ref())
        {
            let _ = wv.evaluate_script(&script);
        }
    }

    /// Sends the saved logins of the current profile (without passwords) to the sidebar.
    pub(super) fn list_passwords(&mut self) {
        let profile_id = self.cur_space().profile_id.clone();
        let rows = match self.passwords.list(&profile_id) {
            Ok(rows) => rows,
            Err(error) => {
                log::warn!("password vault: {error:#}");
                self.call_sidebar(
                    "__setPasswords",
                    &json!({ "error": "The password vault could not be opened." }),
                );
                return;
            }
        };
        let rows: Vec<Value> = rows
            .iter()
            .map(|c| json!({ "id": c.id, "origin": c.origin, "username": c.username, "updated": c.updated }))
            .collect();
        self.call_sidebar("__setPasswords", &json!({ "rows": rows }));
    }

    /// Sends one password to the sidebar, on request (show or copy).
    pub(super) fn reveal_password(&mut self, id: &str) {
        match self.passwords.get(id) {
            Ok(Some(login)) => self.call_sidebar(
                "__passwordSecret",
                &json!({ "id": id, "password": login.password }),
            ),
            Ok(None) => {}
            Err(error) => log::warn!("password vault: {error:#}"),
        }
    }

    pub(super) fn delete_password(&mut self, id: &str) {
        if let Err(error) = self.passwords.delete(id) {
            log::warn!("password vault: {error:#}");
        }
        self.list_passwords();
    }

    /// Calls `window.<function>(argument)` in the sidebar if the UI defined it.
    pub(super) fn call_sidebar(&self, function: &str, argument: &Value) {
        let _ = self
            .sidebar
            .evaluate_script(&format!("window.{function}&&window.{function}({argument})"));
    }
}
