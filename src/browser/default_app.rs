//! "Make poof the default browser": the setting in the sidebar and its status.

use super::*;

impl Browser {
    /// Reports whether poof is the default browser to the settings view.
    pub(super) fn push_default_browser_status(&self) {
        let status = default_browser::status();
        let payload = json!({ "supported": status.supported, "is_default": status.is_default });
        let _ = self.sidebar.evaluate_script(&format!(
            "window.__setDefaultBrowser&&window.__setDefaultBrowser({payload})"
        ));
    }

    /// Registers poof with the system and starts (or finishes) making it the default.
    pub(super) fn set_default_browser(&self) {
        match default_browser::make_default() {
            Ok(default_browser::Outcome::Done) => self.toast("poof is now your default browser."),
            Ok(default_browser::Outcome::ConfirmInSystemSettings) => {
                self.toast("Choose poof in the settings window that just opened.")
            }
            Err(error) => {
                log::warn!("setting the default browser failed: {error:#}");
                self.toast("Couldn’t set poof as the default browser.");
            }
        }
        self.push_default_browser_status();
    }
}
