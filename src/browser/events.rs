//! Entry points for everything that wakes the main loop.

use super::*;

impl Browser {
    pub fn on_user_event(&mut self, ev: UserEvent) {
        match ev {
            UserEvent::CloseWindow => {}
            // `Popup` needs the event-loop target and is handled in main.rs.
            UserEvent::Popup(_) => {}
            UserEvent::PopupCmd { window, cmd } => self.on_popup_cmd(window, &cmd),
            UserEvent::PopupClose { window } => {
                self.popups.remove(&window);
            }
            UserEvent::PopupTitle { window, title } => self.set_popup_title(window, &title),
            UserEvent::DevToolsReady { tab, url } => self.show_devtools(&tab, &url),
            UserEvent::DevToolsFailed { tab, message } => {
                if self.devtools_open && self.visible.as_deref() == Some(tab.as_str()) {
                    self.close_devtools();
                    self.toast(&message);
                }
            }
            UserEvent::TabCrashed { tab, browser } => {
                self.recover_tab(&tab, browser);
                self.push_state();
            }
            UserEvent::Ui(raw) => self.handle_ui(&raw),
            UserEvent::Page { tab, body } => self.handle_page(&tab, &body),
            UserEvent::Fullscreen { tab, active } => self.handle_fullscreen(&tab, active),
            UserEvent::UpdateFound(update) => {
                self.offered_update = Some(update);
                self.push_update_offer();
            }
            UserEvent::UpdateReady(installer) => self.on_update_ready(installer),
            UserEvent::PageSnapshot(png) => self.on_page_snapshot(png),
            UserEvent::UpdateFailed => {
                self.update_now = false;
                self.toast("Couldn’t download the update. It will be offered again next time.");
            }
            UserEvent::Download(event) => self.on_download_event(event),
            UserEvent::External { url } => self.on_external_launch(url),
            UserEvent::Title { tab, title } => {
                if let Some(t) = self.tabs.iter_mut().find(|t| t.id == tab) {
                    t.title = clip(&title, 300);
                    if !t.title.is_empty() {
                        let (pid, url, ttl) =
                            (t.profile_id.clone(), t.url.clone(), t.title.clone());
                        if let Some(h) = self
                            .history
                            .iter_mut()
                            .rev()
                            .find(|h| h.profile_id == pid && h.url == url)
                        {
                            if h.title != ttl {
                                h.title = ttl;
                                self.history_dirty = true;
                            }
                        }
                    }
                }
                // Pages that animate their title must not re-render the sidebar every tick.
                if self.last_push.get().elapsed() >= STATE_MIN_INTERVAL {
                    self.push_state();
                } else {
                    self.state_dirty.set(true);
                }
            }
            UserEvent::OpenPdf { tab, url } => self.open_pdf_in_tab(&tab, &url),
            UserEvent::OpenTab { url } => {
                if ok_url(&url, 4096, false) {
                    if let Err(e) = self.new_tab(&url) {
                        log::error!("{e:#}");
                    }
                    self.push_state();
                }
            }
            UserEvent::LiveFolder { id, source, result } => {
                self.apply_live_result(&id, &source, result)
            }
            UserEvent::ImportDone(result) => self.finish_arc_import(*result),
            UserEvent::ExtResult { ok, msg, dir, id } => {
                self.toast(&msg);
                if ok {
                    if let Some(id) = id {
                        if !self.p.preinstalled.contains(&id) {
                            self.p.preinstalled.push(id);
                        }
                    }
                    if let Some(dir) = dir {
                        // Hot-load into every profile that already has a live webview;
                        // the rest pick it up when their first webview is created.
                        let loaded: HashSet<String> = self
                            .tabs
                            .iter()
                            .filter(|t| t.webview.is_some())
                            .map(|t| t.profile_id.clone())
                            .collect();
                        for pid in &loaded {
                            if let Some(wv) = self
                                .tabs
                                .iter()
                                .find(|t| &t.profile_id == pid && t.webview.is_some())
                                .and_then(|t| t.webview.as_ref())
                            {
                                extensions::add(wv, &dir);
                            }
                        }
                        self.ext_done.retain(|p| loaded.contains(p));
                    }
                    self.save();
                    self.push_exts();
                }
            }
        }
    }

    pub(super) fn handle_ui(&mut self, raw: &str) {
        match serde_json::from_str::<Cmd>(raw) {
            Ok(cmd) => {
                if let Err(e) = self.run(cmd) {
                    log::error!("{e:#}");
                }
            }
            Err(e) => log::warn!("bad ui message: {e}"),
        }
        // Polls arrive many times a second; they never change the pushed state while open.
        if raw.contains("switcher_poll") && self.switcher.is_some() {
            return;
        }
        self.push_state();
    }
}
