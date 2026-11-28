//! The state snapshot pushed to the sidebar UI, and the suggestions it shows.

use super::*;

impl Browser {
    /// Visited sites by use first, then popular defaults the user hasn't visited yet.
    pub(super) fn suggestions(&self) -> Vec<Value> {
        let mut user: Vec<_> = self.p.sites.iter().collect();
        user.sort_by(|a, b| b.1.count.cmp(&a.1.count).then(a.0.cmp(b.0)));
        let mut seen: HashSet<String> = HashSet::new();
        let mut out: Vec<Value> = Vec::new();
        for (host, s) in user.into_iter().take(40) {
            seen.insert(host.clone());
            out.push(json!({"label": host, "url": s.url, "favicon": s.favicon, "visits": s.count, "popular": false}));
        }
        for (name, u) in DEFAULT_SITES {
            let host = url::Url::parse(u)
                .ok()
                .and_then(|x| {
                    x.host_str()
                        .map(|h| h.trim_start_matches("www.").to_string())
                })
                .unwrap_or_default();
            if seen.insert(host) {
                out.push(
                    json!({"label": name, "url": u, "favicon": null, "visits": 0, "popular": true}),
                );
            }
        }
        out
    }

    /// Suggestions are rebuilt only after visit stats changed, not on every state push.
    pub(super) fn suggestions_cached(&self) -> std::cell::Ref<'_, [Value]> {
        if self.sugg_cache.borrow().is_none() {
            let fresh = self.suggestions();
            *self.sugg_cache.borrow_mut() = Some(fresh);
        }
        std::cell::Ref::map(self.sugg_cache.borrow(), |cache| {
            cache.as_deref().unwrap_or(&[])
        })
    }

    /// What the sidebar lists for one space: favorites (shared by its profile), pinned tabs,
    /// loose tabs and folders. Used for the current space's state and for swipe previews.
    pub(super) fn space_view(&self, sp: &Space) -> serde_json::Map<String, Value> {
        let row = |t: &Tab| {
            json!({
                "id": t.id, "title": t.title, "url": t.url,
                "favicon": t.favicon, "loaded": t.webview.is_some(), "folder_id": t.folder_id,
                "dev": is_dev_url(&t.url)
            })
        };
        let in_space = |k: Kind| -> Vec<_> {
            self.tabs
                .iter()
                .filter(|t| t.kind == k && t.space_id.as_deref() == Some(sp.id.as_str()))
                .map(row)
                .collect()
        };
        let favorites: Vec<_> = self
            .tabs
            .iter()
            .filter(|t| t.kind == Kind::Favorite && t.profile_id == sp.profile_id)
            .map(row)
            .collect();
        let folders: Vec<_> = self
            .p
            .folders
            .iter()
            .filter(|f| f.space_id == sp.id)
            .collect();
        let mut view = serde_json::Map::new();
        view.insert("favorites".into(), json!(favorites));
        view.insert("pinned".into(), json!(in_space(Kind::Pinned)));
        view.insert("tabs".into(), json!(in_space(Kind::Normal)));
        view.insert("folders".into(), json!(folders));
        view
    }

    /// Every space's lists, keyed by space id, for the sidebar's swipe carousel. Sent on request
    /// only (never with the regular state), so ordinary pushes stay small.
    pub(super) fn push_space_snapshots(&self) {
        let all: serde_json::Map<String, Value> = self
            .p
            .spaces
            .iter()
            .map(|sp| (sp.id.clone(), Value::Object(self.space_view(sp))))
            .collect();
        let payload = Value::Object(all).to_string();
        let _ = self
            .sidebar
            .evaluate_script(&format!("window.__setSnaps&&window.__setSnaps({payload})"));
    }

    /// Media in tabs that are not showing, for the sidebar's mini player.
    fn media_rows(&self) -> Vec<Value> {
        let mut rows: Vec<_> = self
            .tabs
            .iter()
            .filter(|t| t.webview.is_some() && self.visible.as_deref() != Some(t.id.as_str()))
            .filter_map(|t| {
                let (playing, title, artist) = self.media.get(&t.id)?;
                Some(json!({
                    "id": t.id, "url": t.url, "favicon": t.favicon,
                    "playing": playing, "title": title, "artist": artist
                }))
            })
            .collect();
        rows.truncate(3);
        rows
    }

    pub(super) fn push_state(&self) {
        let sp = self.cur_space();
        let view = self.space_view(sp);
        let url = self
            .visible
            .as_ref()
            .and_then(|id| self.tabs.iter().find(|t| &t.id == id))
            .map(|t| t.url.clone())
            .unwrap_or_default();

        let sugg_ref = self.suggestions_cached();
        let sugg: &[Value] = &sugg_ref;
        let mut state = json!({
            "profiles": self.p.profiles,
            "spaces": self.p.spaces,
            "current": sp.id,
            "active": self.visible,
            "switching": self.switching_space.get(),
            "url": url,
            "suggestions": sugg,
            "store_ext": self.store_ext(),
            "theme": self.p.theme,
            "auto_pip": self.p.auto_pip,
            "search_engine": self.p.search_engine,
            "search_engines": search::ENGINES.iter().map(|(id, name, _)| json!({"id": id, "name": name})).collect::<Vec<_>>(),
            "shortcuts": shortcuts::describe(&self.p.shortcuts),
            "restore_tabs": self.p.restore_tabs,
            "link_rules": self.p.link_rules,
            "boosts": self.p.boosts,
            "mini_window": self.p.mini_window,
            "peek_links": self.p.peek_links,
            "feedback_sound": self.p.feedback_sound,
            "media": self.media_rows(),
            "link_fallback": self.p.link_fallback,
            "save_passwords": self.p.save_passwords,
            "sidebar_collapsed": self.p.sidebar_collapsed,
            "sidebar_width": self.p.sidebar_width,
            "sidebar_compact": self.p.sidebar_compact,
            // Empty-list tips: only from the 3rd to the 49th tab ever opened, and never again
            // once favorites or pins have been used in any space.
            "hints": {
                "favorites": (3..50).contains(&self.p.tabs_opened)
                    && !(self.p.used_favorites || self.tabs.iter().any(|t| t.kind == Kind::Favorite)),
                "pinned": (3..50).contains(&self.p.tabs_opened)
                    && !(self.p.used_pinned || self.tabs.iter().any(|t| t.kind == Kind::Pinned)),
            },
            "schedules": self.p.schedules,
            "resume": self.resume_cards_json(),
            "devtools_open": self.devtools_open,
            "devtools_position": self.p.devtools_position,
            "devtools_size": self.p.devtools_size,
            "sidebar_peek": self.peek,
        "split": self.split.as_ref().map(|s| json!({"a": s.a, "b": s.b, "stacked": s.stacked})),
                    "splits": self.saved_splits.iter()
                .filter(|s| self.tabs.iter().any(|t| t.id == s.a) && self.tabs.iter().any(|t| t.id == s.b))
                .map(|s| json!({"a": s.a, "b": s.b}))
                .collect::<Vec<_>>(),
            "profile_cleanup": self.p.profiles.iter()
                .find(|p| p.id == sp.profile_id)
                .map(|p| json!({
                    "tab_retention_hours": p.tab_retention_hours,
                    "cookie_retention_hours": p.cookie_retention_hours,
                    "cache_retention_hours": p.cache_retention_hours,
                    "history_retention_hours": p.history_retention_hours
                }))
                .unwrap_or_else(|| json!({})),
        });
        drop(sugg_ref);
        if let Some(fields) = state.as_object_mut() {
            fields.extend(view);
        }

        // Skip the IPC round-trip and the sidebar re-render when nothing visible changed.
        let payload = state.to_string();
        let mut hasher = DefaultHasher::new();
        payload.hash(&mut hasher);
        let digest = hasher.finish().max(1);
        self.last_push.set(Instant::now());
        self.state_dirty.set(false);
        if self.last_state_hash.get() == digest {
            return;
        }
        if self
            .sidebar
            .evaluate_script(&format!("window.__setState({payload})"))
            .is_ok()
        {
            self.last_state_hash.set(digest);
        }
    }

    /// A toast with a "change" button that opens the settings tab where the cause can be edited.
    pub(super) fn toast_change(&self, msg: &str, tab: &str) {
        let m = serde_json::to_string(msg).unwrap_or_default();
        let t = serde_json::to_string(tab).unwrap_or_default();
        let _ = self.sidebar.evaluate_script(&format!(
            "window.__changeToast&&window.__changeToast({m},{t})"
        ));
    }

    pub(super) fn toast(&self, msg: &str) {
        let m = serde_json::to_string(msg).unwrap_or_default();
        let _ = self
            .sidebar
            .evaluate_script(&format!("window.__toast&&window.__toast({m})"));
    }
}
