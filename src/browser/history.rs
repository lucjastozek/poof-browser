//! Browsing history: recording visits, retention and the sidebar history list.

use super::*;

impl Browser {
    pub(super) fn record_visit(&mut self, tab: &str, profile_id: String, url: String) {
        self.logged_urls.insert(tab.to_string(), url.clone());
        // DuckDuckGo's click-tracking redirect is not a page the user visited.
        if url.contains("duckduckgo.com/l/") {
            return;
        }
        let ts = now_secs();
        if let Some(last) = self
            .history
            .iter()
            .rev()
            .find(|h| h.profile_id == profile_id)
        {
            if last.url == url && ts.saturating_sub(last.ts) < 30 {
                return;
            }
        }
        self.history.push(HistoryEntry {
            id: new_id(),
            profile_id,
            url,
            title: String::new(),
            ts,
        });
        if self.history.len() > HISTORY_MAX {
            let excess = self.history.len() - HISTORY_MAX;
            self.history.drain(..excess);
        }
        self.history_dirty = true;
    }

    /// Applies each profile's "history retention" setting to the history log.
    pub(super) fn prune_history(&mut self) {
        let now = now_secs();
        let retention: HashMap<String, u64> = self
            .p
            .profiles
            .iter()
            .map(|p| (p.id.clone(), p.history_retention_hours))
            .collect();
        let before = self.history.len();
        self.history.retain(
            |h| match retention.get(&h.profile_id).copied().unwrap_or(0) {
                0 => true,
                hours => now.saturating_sub(h.ts) < hours.saturating_mul(3600),
            },
        );
        if self.history.len() > HISTORY_MAX {
            let excess = self.history.len() - HISTORY_MAX;
            self.history.drain(..excess);
        }
        if self.history.len() != before {
            self.history_dirty = true;
        }
    }

    pub(super) fn push_history(&self) {
        let pid = self.cur_space().profile_id.clone();
        let q = self.history_query.to_lowercase();
        let rows: Vec<Value> = self
            .history
            .iter()
            .rev()
            .filter(|h| {
                h.profile_id == pid
                    && (q.is_empty()
                        || h.url.to_lowercase().contains(&q)
                        || h.title.to_lowercase().contains(&q))
            })
            .take(HISTORY_SHOWN)
            .map(|h| json!({ "id": h.id, "url": h.url, "title": h.title, "ts": h.ts }))
            .collect();
        let payload = serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into());
        let _ = self.sidebar.evaluate_script(&format!(
            "window.__setHistory&&window.__setHistory({payload})"
        ));
    }

    pub(super) fn flush_history(&mut self, force: bool) {
        if !self.history_dirty || (!force && self.last_history_save.elapsed() < SAVE_MIN_INTERVAL) {
            return;
        }
        let ok = save_history(&self.history, &self.root.join("history.json"));
        self.last_history_save = Instant::now();
        if ok {
            self.history_dirty = false;
        } else {
            log::warn!("saving history.json failed; will retry");
        }
    }

    /// Filters the history list shown in the sidebar.
    pub(super) fn list_history(&mut self, query: &str) {
        self.history_query = clip(query.trim(), 200);
        self.push_history();
    }

    /// Removes one history entry.
    pub(super) fn delete_history_entry(&mut self, id: &str) {
        let before = self.history.len();
        self.history.retain(|h| h.id != id);
        if self.history.len() != before {
            self.history_dirty = true;
        }
        self.push_history();
    }

    /// Clears the history of the current profile.
    pub(super) fn clear_history(&mut self) {
        let pid = self.cur_space().profile_id.clone();
        self.history.retain(|h| h.profile_id != pid);
        self.history_dirty = true;
        self.push_history();
    }
}
