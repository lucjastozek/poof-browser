//! Tab lifecycle: open, activate, hide, unload and close.

use super::*;

impl Browser {
    pub(super) fn visible_webview(&self) -> Option<&WebView> {
        let id = self.visible.as_ref()?;
        self.tabs.iter().find(|t| &t.id == id)?.webview.as_ref()
    }

    pub(super) fn eval_visible(&self, js: &str) {
        if let Some(wv) = self.visible_webview() {
            let _ = wv.evaluate_script(js);
        }
    }

    pub(super) fn set_kind(&mut self, id: &str, kind: Kind) {
        let cur = self.p.current_space.clone();
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            t.kind = kind;
            t.space_id = if kind == Kind::Favorite {
                None
            } else {
                Some(t.space_id.clone().unwrap_or(cur))
            };
            t.home = t.url.clone();
        }
        self.save();
    }

    /// Drops a tab into a list. `to` is "pinned", "tab" or "fav"; `index` is its position in that list.
    pub(super) fn move_tab(&mut self, id: &str, to: &str, index: usize) {
        let sp = self.cur_space().clone();
        let Some(from) = self.tabs.iter().position(|t| t.id == id) else {
            return;
        };
        let mut t = self.tabs.remove(from);
        let kind = match to {
            "pinned" => Kind::Pinned,
            "fav" => Kind::Favorite,
            _ => Kind::Normal,
        };
        t.folder_id = None;
        if t.kind != kind {
            t.home = t.url.clone();
        }
        t.kind = kind;
        t.space_id = if kind == Kind::Favorite {
            None
        } else {
            Some(sp.id.clone())
        };
        let profile = t.profile_id.clone();
        let peers: Vec<usize> = self
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, x)| {
                x.kind == kind
                    && if kind == Kind::Favorite {
                        x.profile_id == profile
                    } else {
                        x.space_id.as_deref() == Some(sp.id.as_str())
                            && (kind != Kind::Pinned || x.folder_id.is_none())
                    }
            })
            .map(|(i, _)| i)
            .collect();
        let pos = match peers.get(index) {
            Some(&i) => i,
            None => peers.last().map(|i| i + 1).unwrap_or(self.tabs.len()),
        };
        self.tabs.insert(pos, t);
        self.save();
    }

    pub(super) fn new_tab(&mut self, url: &str) -> Result<()> {
        let sp = self.cur_space().clone();
        let id = new_id();
        self.tabs.push(Tab {
            id: id.clone(),
            profile_id: sp.profile_id,
            space_id: Some(sp.id),
            kind: Kind::Normal,
            folder_id: None,
            home: url.to_string(),
            url: url.to_string(),
            host: String::new(),
            title: String::new(),
            favicon: None,
            webview: None,
            last_active: SystemTime::now(),
        });
        self.activate(&id)
    }

    /// Recreates the WebView of a tab whose renderer died, or of every tab of a profile whose
    /// browser process died. Guards against crash loops.
    pub(super) fn recover_tab(&mut self, tab: &str, browser_died: bool) {
        let Some(profile) = self
            .tabs
            .iter()
            .find(|t| t.id == tab)
            .map(|t| t.profile_id.clone())
        else {
            return;
        };
        if self.split.is_some() && (browser_died || self.split_has(tab)) {
            self.close_split();
        }

        let now = Instant::now();
        let entry = self.crashes.entry(tab.to_string()).or_insert((now, 0));
        if now.duration_since(entry.0) > Duration::from_secs(30) {
            *entry = (now, 0);
        }
        entry.1 += 1;
        let count = entry.1;
        if count > 3 {
            self.toast("This page keeps crashing, so it was not reloaded again.");
            return;
        }

        let victims: Vec<usize> = self
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                t.webview.is_some()
                    && if browser_died {
                        t.profile_id == profile
                    } else {
                        t.id == tab
                    }
            })
            .map(|(i, _)| i)
            .collect();
        if victims.is_empty() {
            return;
        }

        let visible_hit = self
            .visible
            .as_deref()
            .map(|v| victims.iter().any(|&i| self.tabs[i].id == v))
            .unwrap_or(false);
        if visible_hit {
            if self.devtools_open {
                self.close_devtools();
            }
            self.leave_fullscreen();
        }
        let revive = if visible_hit {
            self.visible.take()
        } else {
            None
        };
        for i in victims {
            self.tabs[i].webview = None;
        }
        if browser_died {
            // The profile's environment is gone, so extensions must be loaded again.
            self.ext_done.remove(&profile);
        }
        if let Some(id) = revive {
            if let Err(e) = self.activate(&id) {
                log::error!("could not recover crashed tab: {e:#}");
            }
        }
        self.toast("A page crashed and was reloaded.");
    }

    pub(super) fn hide_visible(&mut self) {
        self.end_split();
        self.leave_fullscreen();
        let Some(id) = self.visible.take() else {
            return;
        };
        self.responsive_mode = false;
        self.responsive_bounds = None;
        // Only wake the page for auto-PIP when the user actually started a video on it.
        let try_pip = self.p.auto_pip && self.vid_tabs.contains(&id);
        let mut trimmed = false;
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            // The idle clock (for sleeping the tab) starts when the user leaves it.
            t.last_active = SystemTime::now();
            if let Some(wv) = t.webview.as_ref() {
                if try_pip {
                    pip::on_leave(wv);
                }
                let _ = wv.set_visible(false);
                // A page that may be about to pop out a video keeps its memory budget until
                // the next maintenance pass (`trim_hidden_tabs`) knows whether PIP started.
                if !try_pip && !is_playing_audio(wv) {
                    set_memory_level(wv, true);
                    trimmed = true;
                }
            }
        }
        if trimmed {
            self.low_mem.insert(id);
        }
        if let Some(devtools) = self.devtools.as_ref() {
            let _ = devtools.set_visible(false);
        }
    }

    pub(super) fn activate(&mut self, id: &str) -> Result<()> {
        let Some(idx) = self.tabs.iter().position(|t| t.id == id) else {
            return Ok(());
        };
        self.tabs[idx].last_active = SystemTime::now();
        self.mru.retain(|x| x != id);
        self.mru.insert(0, id.to_string());
        self.bg_tabs.remove(id);
        if self.split.is_some() {
            if self.split_has(id) {
                return self.focus_split_pane(id);
            }
            self.end_split();
        }

        if self.split.is_none() {
            let pair = self
                .saved_splits
                .iter()
                .find(|s| {
                    (s.a == id || s.b == id)
                        && self.tabs.iter().any(|t| t.id == s.a)
                        && self.tabs.iter().any(|t| t.id == s.b)
                })
                .cloned();
            if let Some(pair) = pair {
                return self.restore_split(pair, id);
            }
        }

        if self.visible.as_deref() == Some(id) && !self.overlay {
            return Ok(());
        }
        self.hide_visible();
        self.ensure_webview(idx)?;
        self.overlay = false;
        self.visible = Some(id.to_string());
        self.active
            .insert(self.p.current_space.clone(), id.to_string());
        if self.low_mem.remove(id) {
            if let Some(wv) = &self.tabs[idx].webview {
                set_memory_level(wv, false);
            }
        }
        self.relayout();
        self.sync_scrollbar();
        if let Some(wv) = &self.tabs[idx].webview {
            wv.focus()?;
            if self.p.auto_pip && self.vid_tabs.contains(id) {
                pip::on_enter(wv);
            }
        }
        if self.p.restore_tabs {
            self.save();
        }
        if self.devtools_open {
            self.attach_devtools()?;
        }
        Ok(())
    }

    pub(super) fn close_tab(&mut self, id: &str) -> Result<()> {
        let Some(idx) = self.tabs.iter().position(|t| t.id == id) else {
            return Ok(());
        };
        if self.split_has(id) {
            self.end_split();
            self.relayout();
        }
        self.saved_splits.retain(|s| s.a != id && s.b != id);
        let was_visible = self.visible.as_deref() == Some(id);
        if was_visible {
            self.leave_fullscreen();
            self.visible = None;
            self.responsive_mode = false;
            self.responsive_bounds = None;
            if let Some(wv) = &self.tabs[idx].webview {
                let _ = wv.set_visible(false);
            }
            if let Some(devtools) = self.devtools.as_ref() {
                let _ = devtools.set_visible(false);
            }
        }
        self.bg_tabs.remove(id);
        self.mru.retain(|x| x != id);
        if self.tabs[idx].kind == Kind::Normal {
            self.logged_urls.remove(id);
            self.closed_tabs.insert(0, self.tabs[idx].url.clone());
            self.closed_tabs.truncate(25);
            self.tabs.remove(idx);
        } else {
            // Pinned / favorite: unload and reset to its home URL, keep the entry.
            let t = &mut self.tabs[idx];
            t.webview = None;
            t.url = t.home.clone();
        }
        self.active.retain(|_, v| v != id);
        if was_visible {
            self.restore_space_tab()?;
        }
        self.prune_ext_done();
        self.save();
        Ok(())
    }

    /// Frees the tab's WebView but keeps the entry (and its current URL) in the sidebar.
    pub(super) fn unload_tab(&mut self, id: &str) -> Result<()> {
        let Some(idx) = self.tabs.iter().position(|t| t.id == id) else {
            return Ok(());
        };
        if self.tabs[idx].webview.is_none() {
            return Ok(());
        }
        if self.split_has(id) {
            self.close_split();
        }
        if self.visible.as_deref() == Some(id) {
            // Move to a neighbouring tab first, so the unloaded one is not reloaded right away.
            let ids = self.space_tab_ids();
            let pos = ids.iter().position(|x| x == id).unwrap_or(0);
            let next = ids
                .get(pos + 1)
                .or_else(|| pos.checked_sub(1).and_then(|p| ids.get(p)))
                .cloned();
            match next {
                Some(n) => self.activate(&n)?,
                None => {
                    if self.devtools_open {
                        self.close_devtools();
                    }
                    self.hide_visible();
                }
            }
        }
        self.active.retain(|_, v| v != id);
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            t.webview = None;
            t.last_active = SystemTime::now();
        }
        self.prune_ext_done();
        self.save();
        Ok(())
    }

    /// Closes every unpinned tab of the current space that is not inside a folder.
    pub(super) fn clear_tabs(&mut self) -> Result<()> {
        let space = self.p.current_space.clone();
        let ids: Vec<String> = self
            .tabs
            .iter()
            .filter(|t| {
                t.kind == Kind::Normal
                    && t.folder_id.is_none()
                    && t.space_id.as_deref() == Some(space.as_str())
            })
            .map(|t| t.id.clone())
            .collect();
        // Visible tab last, so focus is only restored once.
        let visible = self.visible.clone();
        for id in ids
            .iter()
            .filter(|id| visible.as_deref() != Some(id.as_str()))
        {
            self.close_tab(id)?;
        }
        if let Some(v) = visible.filter(|v| ids.contains(v)) {
            self.close_tab(&v)?;
        }
        Ok(())
    }

    pub(super) fn reopen_closed_tab(&mut self) -> Result<()> {
        if self.closed_tabs.is_empty() {
            return Ok(());
        }
        let url = self.closed_tabs.remove(0);
        self.new_tab(&url)
    }

    /// Opens a link in a new tab right behind the current one without switching to it.
    pub(super) fn open_background_tab(&mut self, url: &str) -> Result<()> {
        let sp = self.cur_space().clone();
        let opener = self
            .visible
            .as_ref()
            .and_then(|v| self.tabs.iter().position(|t| &t.id == v))
            .filter(|&i| {
                self.tabs[i].kind == Kind::Normal
                    && self.tabs[i].space_id.as_deref() == Some(sp.id.as_str())
            });
        let folder_id = opener.and_then(|i| self.tabs[i].folder_id.clone());
        let id = new_id();
        let tab = Tab {
            id: id.clone(),
            profile_id: sp.profile_id,
            space_id: Some(sp.id),
            kind: Kind::Normal,
            folder_id,
            home: url.to_string(),
            url: url.to_string(),
            host: String::new(),
            title: String::new(),
            favicon: None,
            webview: None,
            last_active: SystemTime::now(),
        };
        let pos = match opener {
            Some(i) => {
                // Several ctrl+clicks in a row keep their click order.
                let mut at = i + 1;
                while at < self.tabs.len() && self.bg_tabs.contains(&self.tabs[at].id) {
                    at += 1;
                }
                at
            }
            None => self.tabs.len(),
        };
        self.tabs.insert(pos, tab);
        self.bg_tabs.insert(id.clone());
        // Start loading right away (hidden); the tab is not activated.
        if let Some(idx) = self.tabs.iter().position(|t| t.id == id) {
            if let Err(e) = self.ensure_webview(idx) {
                log::warn!("could not start background tab: {e:#}");
            }
        }
        if self.p.restore_tabs {
            self.save();
        }
        Ok(())
    }

    pub(super) fn activate_tab_index(&mut self, index: usize) -> Result<()> {
        let ids = self.space_tab_ids();
        if ids.is_empty() {
            return Ok(());
        }
        self.activate(&ids[index.min(ids.len() - 1)])
    }
}
