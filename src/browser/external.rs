//! Links and files handed over by the operating system or by another launch of the browser.

use super::*;

impl Browser {
    /// Opens an address from outside in a new tab. UNTRUSTED: only web links and local files
    /// pass. Before the sidebar has loaded it is queued, because the first state restore
    /// would otherwise switch away from the new tab again.
    pub fn open_external(&mut self, url: &str) {
        if !ok_external_url(url) {
            log::warn!("ignoring an address handed over by another launch");
            return;
        }
        if !self.sidebar_ready {
            self.startup_urls.push(url.trim().to_string());
            return;
        }
        if let Err(error) = self.new_tab(url.trim()) {
            log::error!("{error:#}");
        }
    }

    /// Opens what was queued by `open_external`.
    pub(super) fn open_startup_urls(&mut self) {
        for url in std::mem::take(&mut self.startup_urls) {
            if let Err(error) = self.new_tab(&url) {
                log::error!("{error:#}");
            }
        }
    }

    /// Another launch asked this browser to come to the front (and maybe open a link).
    pub(super) fn on_external_launch(&mut self, url: Option<String>) {
        self.window.set_minimized(false);
        self.window.set_focus();
        if let Some(url) = url {
            self.open_external(&url);
        }
        self.push_state();
    }
}
