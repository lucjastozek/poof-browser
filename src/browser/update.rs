//! The "update available" prompt and what follows from the user's answer.

use super::*;

impl Browser {
    /// Shows (or, with nothing pending, hides) the update card in the sidebar.
    pub(super) fn push_update_offer(&self) {
        let offer = match &self.offered_update {
            Some(update) => json!({ "version": update.version.to_string() }),
            None => Value::Null,
        };
        self.call_sidebar("__updateOffer", &offer);
    }

    /// "Update now" downloads, installs and restarts; "Later" downloads quietly and installs
    /// when the browser closes.
    pub(super) fn answer_update(&mut self, now: bool) {
        let Some(update) = self.offered_update.take() else {
            return;
        };
        self.update_now = now;
        if now {
            self.toast("Downloading the update…");
        }
        updater::spawn_download(update, self.root.clone(), self.proxy.clone());
        self.push_update_offer();
    }

    pub(super) fn on_update_ready(&mut self, installer: PathBuf) {
        self.pending_update = Some(installer);
        if self.update_now {
            // `shutdown` saves everything, then starts the installer and the new version.
            let _ = self.proxy.send_event(UserEvent::CloseWindow);
        }
    }
}
