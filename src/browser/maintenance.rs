//! Background upkeep that frees memory: sleeping idle tabs and trimming hidden ones.

use super::*;

impl Browser {
    /// A profile whose last WebView is gone loses its WebView2 environment, so its extensions
    /// have to be loaded again the next time one is created.
    pub(super) fn prune_ext_done(&mut self) {
        let live: HashSet<String> = self
            .tabs
            .iter()
            .filter(|t| t.webview.is_some())
            .map(|t| t.profile_id.clone())
            .chain(self.lingering.iter().map(|l| l.profile_id.clone()))
            .collect();
        self.ext_done.retain(|p| live.contains(p));
        self.fail_orphaned_downloads(&live);
        // Per-tab bookkeeping dies with the WebView it describes.
        let loaded: HashSet<&str> = self
            .tabs
            .iter()
            .filter(|t| t.webview.is_some())
            .map(|t| t.id.as_str())
            .collect();
        self.vid_tabs.retain(|id| loaded.contains(id.as_str()));
        self.pip_tabs.retain(|id| loaded.contains(id.as_str()));
        self.live_tabs.retain(|id| loaded.contains(id.as_str()));
        self.low_mem.retain(|id| loaded.contains(id.as_str()));
    }

    /// Keeps hidden pages as small as possible: pages that are neither playing audio nor
    /// showing a PIP window get the LOW memory target (this also covers a PIP window that was
    /// closed, or audio that stopped, after the tab was left); pages that started doing either
    /// get their normal budget back.
    pub(super) fn trim_hidden_tabs(&mut self) {
        let visible = self.visible.clone();
        let split_ids: Vec<String> = self
            .split
            .iter()
            .flat_map(|s| [s.a.clone(), s.b.clone()])
            .collect();
        for t in self.tabs.iter() {
            let Some(wv) = t.webview.as_ref() else {
                continue;
            };
            if visible.as_deref() == Some(t.id.as_str()) || split_ids.contains(&t.id) {
                continue;
            }
            let busy = self.pip_tabs.contains(&t.id)
                || self.live_tabs.contains(&t.id)
                || is_playing_audio(wv);
            let low = self.low_mem.contains(&t.id);
            if busy && low {
                set_memory_level(wv, false);
                self.low_mem.remove(&t.id);
            } else if !busy && !low {
                set_memory_level(wv, true);
                self.low_mem.insert(t.id.clone());
            }
        }
    }

    /// Releases the WebView (and with it the renderer process) of background tabs that have been
    /// idle for `tab_sleep_minutes`. The tab stays in the sidebar at its current URL and reloads
    /// when selected. The visible tab, the DevTools target, tabs playing audio, tabs in a call or
    /// capturing media, and tabs with a running download are kept.
    pub(super) fn sleep_idle_tabs(&mut self) {
        let minutes = self.p.tab_sleep_minutes;
        if minutes == 0 {
            return;
        }
        let limit = Duration::from_secs(minutes.saturating_mul(60));
        let now = SystemTime::now();
        let visible = self.visible.clone();
        let devtools_tab = self.devtools_tab.clone();
        let mut slept = false;
        let split_ids: Vec<String> = self
            .split
            .iter()
            .flat_map(|s| [s.a.clone(), s.b.clone()])
            .collect();
        let pip_tabs = &self.pip_tabs;
        let live_tabs = &self.live_tabs;
        let downloading = self.tabs_with_live_downloads();
        for t in self.tabs.iter_mut() {
            let Some(wv) = t.webview.as_ref() else {
                continue;
            };
            if visible.as_deref() == Some(t.id.as_str())
                || devtools_tab.as_deref() == Some(t.id.as_str())
                || split_ids.contains(&t.id)
                || pip_tabs.contains(&t.id)
                || live_tabs.contains(&t.id)
                || downloading.contains(&t.id)
            {
                continue;
            }
            if now.duration_since(t.last_active).unwrap_or(Duration::ZERO) < limit {
                continue;
            }
            if is_playing_audio(wv) {
                t.last_active = now;
                continue;
            }
            t.webview = None;
            slept = true;
        }
        if slept {
            self.prune_ext_done();
        }
    }

    /// Expires inactive normal tabs, puts idle ones to sleep and applies profile-level
    /// browsing-data policies.
    pub fn cleanup_expired_tabs(&mut self) {
        let now = SystemTime::now();
        let expired: Vec<String> = self
            .tabs
            .iter()
            .filter(|t| t.kind == Kind::Normal && self.visible.as_deref() != Some(t.id.as_str()))
            .filter(|t| {
                let hours = self
                    .p
                    .profiles
                    .iter()
                    .find(|p| p.id == t.profile_id)
                    .map(|p| p.tab_retention_hours)
                    .unwrap_or(48);
                now.duration_since(t.last_active)
                    .unwrap_or(Duration::ZERO)
                    .as_secs()
                    >= hours.saturating_mul(3600)
            })
            .filter(|t| !self.split_has(&t.id))
            .map(|t| t.id.clone())
            .collect();
        for id in expired {
            if let Err(e) = self.close_tab(&id) {
                log::warn!("auto-tidy could not close tab: {e:#}");
            }
        }
        self.sleep_idle_tabs();
        self.prune_ext_done();
        self.trim_hidden_tabs();
        self.cleanup_profile_data();
        self.prune_history();
        self.push_state();
    }
}
