//! Messages from web pages. UNTRUSTED: strict parsing, size limits, own-tab only.

use super::*;

impl Browser {
    /// Messages from web pages are untrusted: strict parsing, size limits, own-tab only.
    pub(super) fn handle_page(&mut self, tab: &str, body: &str) {
        if body.len() > 700_000 {
            return;
        }
        let Ok(msg) = serde_json::from_str::<PageMsg>(body) else {
            return;
        };
        match msg {
            // Internal messages only come from native code (empty tab id). A page always has
            // its own non-empty id baked in, so it cannot forge them.
            PageMsg::DevToolsTick => {
                if tab.is_empty() {
                    self.poll_devtools();
                }
                return;
            }
            PageMsg::Relayout => {
                if tab.is_empty() {
                    self.relayout();
                }
                return;
            }
            PageMsg::DevToolsState { s } => {
                if tab.is_empty() {
                    if let Some(report) = s {
                        self.apply_devtools_report(report);
                    }
                }
                return;
            }
            PageMsg::OpenBackground { u } => {
                // Internal only: a page always carries its own non-empty tab id, so it cannot forge this.
                if tab.is_empty() && ok_url(&u, 4096, false) {
                    if let Err(e) = self.open_background_tab(&u) {
                        log::error!("{e:#}");
                    }
                }
            }
            PageMsg::Meta { u } => self.on_page_meta(tab, u),
            PageMsg::Favicon { f } => self.on_page_favicon(tab, f),
            PageMsg::Fullscreen { active } => self.handle_fullscreen(tab, active),
            PageMsg::PwForm => {
                self.on_page_password_form(tab);
                return;
            }
            PageMsg::PwSubmit { u, p } => {
                self.on_page_password_submit(tab, u, p);
                return;
            }
            PageMsg::Key { k } if k == "history" => {
                self.toggle_history_view(tab);
                return;
            }
            PageMsg::Key { k } => self.run_shortcut(tab, &k),
            PageMsg::Vid => {
                self.on_page_video(tab);
                return;
            }
            PageMsg::Pip { active } => {
                self.on_page_pip(tab, active);
                return;
            }
            PageMsg::Focus => self.on_page_focus(tab),
        }
        // The sidebar is a separate WebView and only sees tab changes through pushed state.
        // (Identical states are dropped inside push_state.)
        self.push_state();
    }

    /// The page (or WebView2) reported its address: tracks the host, site stats and history.
    pub(super) fn on_page_meta(&mut self, tab: &str, u: Option<String>) {
        let mut persist = false;
        let mut sites_dirty = false;
        let mut visit: Option<(String, String)> = None;
        let restore = self.p.restore_tabs;
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == tab) {
            let mut url_changed = false;
            if let Some(u) = u.filter(|u| ok_url(u, 4096, false)) {
                if self.logged_urls.get(tab).map(String::as_str) != Some(u.as_str()) {
                    visit = Some((t.profile_id.clone(), u.clone()));
                }
                if t.url != u {
                    url_changed = true;
                }
                if let Ok(parsed) = url::Url::parse(&u) {
                    if let Some(h) = parsed.host_str() {
                        let host = h.trim_start_matches("www.").to_string();
                        if t.host != host {
                            t.favicon = None;
                            t.host = host.clone();
                            let s = self.p.sites.entry(host).or_default();
                            s.count += 1;
                            s.url = format!("{}://{}/", parsed.scheme(), h);
                            sites_dirty = true;
                        }
                    }
                }
                t.url = u;
            }
            if let Some(fv) = &t.favicon {
                if let Some(s) = self.p.sites.get_mut(&t.host) {
                    if s.favicon.as_ref() != Some(fv) {
                        s.favicon = Some(fv.clone());
                        sites_dirty = true;
                    }
                }
            }
            persist = url_changed && (t.kind != Kind::Normal || restore);
        }
        self.p.prune_sites();
        if let Some((profile_id, url)) = visit {
            self.record_visit(tab, profile_id, url);
        }
        if sites_dirty {
            self.sugg_cache.replace(None);
        }
        if persist {
            self.save();
        }
        if self.visible.as_deref() == Some(tab) {
            self.sync_scrollbar();
        }
    }

    /// The page's favicon changed.
    pub(super) fn on_page_favicon(&mut self, tab: &str, f: Option<String>) {
        let mut persist = false;
        let mut sites_dirty = false;
        let restore = self.p.restore_tabs;
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == tab) {
            if update_favicon(&mut t.favicon, f) {
                if let Some(s) = self.p.sites.get_mut(&t.host) {
                    s.favicon = t.favicon.clone();
                    sites_dirty = true;
                }
                persist = t.kind != Kind::Normal || restore;
            }
        }
        if sites_dirty {
            self.sugg_cache.replace(None);
        }
        if persist {
            self.save();
        }
    }

    /// Ctrl+H: opens or closes the history view in the sidebar.
    pub(super) fn toggle_history_view(&self, tab: &str) {
        if tab.is_empty() || self.visible.as_deref() == Some(tab) {
            let _ = self.sidebar.focus();
            let _ = self
                .sidebar
                .evaluate_script("window.__toggleHistory && window.__toggleHistory()");
        }
    }

    /// Runs a keyboard shortcut (`k` is the shortcut's name) for the tab it came from.
    pub(super) fn run_shortcut(&mut self, tab: &str, k: &str) {
        // An empty tab ID means the shortcut originated in the sidebar.
        let active_tab = self.visible.clone().unwrap_or_default();
        let tab = if tab.is_empty() {
            active_tab.as_str()
        } else {
            tab
        };
        if self.split_has(tab) && self.visible.as_deref() != Some(tab) {
            let id = tab.to_string();
            let _ = self.focus_split_pane(&id);
        }
        if self.visible.as_deref() != Some(tab) {
            return;
        }
        let r = match k {
            "new_tab" => self.new_tab("about:blank"),
            "focus_url" => {
                let _ = self.sidebar.focus();
                let _ = self
                    .sidebar
                    .evaluate_script("window.__focusUrl && window.__focusUrl()");
                Ok(())
            }
            "close" => self.close_tab(tab),
            "reopen" => self.reopen_closed_tab(),
            "back" => {
                self.eval_visible("history.back()");
                Ok(())
            }
            "forward" => {
                self.eval_visible("history.forward()");
                Ok(())
            }
            "reload" => {
                self.eval_visible("location.reload()");
                Ok(())
            }
            "space_next" => self.cycle_space(1),
            "space_prev" => self.cycle_space(-1),
            "tab_next" => self.switcher_step(1),
            "tab_prev" => self.switcher_step(-1),
            "tab_1" | "tab_2" | "tab_3" | "tab_4" | "tab_5" | "tab_6" | "tab_7" | "tab_8"
            | "tab_last" => {
                let index = if k == "tab_last" {
                    usize::MAX
                } else {
                    k[4..].parse::<usize>().unwrap_or(1).saturating_sub(1)
                };
                self.activate_tab_index(index)
            }
            "print" => {
                self.eval_visible("window.print()");
                Ok(())
            }
            "favorite" => {
                self.set_kind(tab, Kind::Favorite);
                Ok(())
            }
            "devtools_toggle" => self.toggle_devtools(),
            "split_toggle" => self.toggle_split(),
            "split_orient" => {
                self.toggle_split_orientation();
                Ok(())
            }
            "toggle_scrollbar" => {
                self.toggle_scrollbar();
                Ok(())
            }
            _ => Ok(()),
        };
        if let Err(e) = r {
            log::error!("{e:#}");
        }
    }

    /// The user started a video on this page, so it may be popped out when they leave it.
    pub(super) fn on_page_video(&mut self, tab: &str) {
        if !tab.is_empty() && self.tabs.iter().any(|t| t.id == tab) {
            self.vid_tabs.insert(tab.to_string());
        }
    }

    /// The page opened or closed a Picture-in-Picture window.
    pub(super) fn on_page_pip(&mut self, tab: &str, active: bool) {
        if tab.is_empty() || !self.tabs.iter().any(|t| t.id == tab) {
            return;
        }
        if !active {
            self.pip_tabs.remove(tab);
            return;
        }
        self.pip_tabs.insert(tab.to_string());
        // A PIP window keeps playing; give the page its normal memory budget.
        if self.low_mem.remove(tab) {
            if let Some(wv) = self
                .tabs
                .iter()
                .find(|t| t.id == tab)
                .and_then(|t| t.webview.as_ref())
            {
                set_memory_level(wv, false);
            }
        }
    }

    /// The page took keyboard focus: if it is a split pane, make it the active one.
    pub(super) fn on_page_focus(&mut self, tab: &str) {
        if self.split_has(tab) && self.visible.as_deref() != Some(tab) {
            let id = tab.to_string();
            if let Err(e) = self.focus_split_pane(&id) {
                log::error!("{e:#}");
            }
        }
    }
}
