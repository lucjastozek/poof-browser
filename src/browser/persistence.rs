//! Saving browser state to disk, throttled.

use super::*;

impl Browser {
    /// Marks the state dirty and writes it unless a write just happened; bursts of changes
    /// (navigation, favicons, drags) are coalesced and flushed by `flush_save` from the event
    /// loop timer, so the UI thread is not blocked by disk I/O on every event.
    pub fn save(&mut self) {
        self.save_dirty = true;
        if self.last_save.elapsed() >= SAVE_MIN_INTERVAL {
            self.flush_save();
        }
    }

    pub fn save_pending(&self) -> bool {
        self.save_dirty || self.history_dirty
    }

    /// How long the event loop may sleep before something deferred must be flushed
    /// (a coalesced sidebar update sooner than a throttled disk write).
    pub fn flush_delay(&self) -> Option<Duration> {
        if self.state_dirty.get() {
            Some(STATE_MIN_INTERVAL)
        } else if self.save_pending() {
            Some(Duration::from_millis(1500))
        } else {
            None
        }
    }

    /// Final, unconditional write on exit.
    pub fn shutdown(&mut self) {
        self.save_dirty = true;
        self.flush_history(true);
        self.flush_save();
        if let Some(devtools) = self.devtools.take() {
            drop(devtools);
        }
        // Last: the installer replaces our files, so everything must be saved before it starts.
        if let Some(installer) = self.pending_update.take() {
            updater::install_on_exit(&installer);
        }
    }

    pub fn flush_save(&mut self) {
        if self.state_dirty.get() {
            self.push_state();
        }
        self.flush_history(false);
        if !self.save_dirty {
            return;
        }
        let restore = self.p.restore_tabs;
        self.p.saved = self
            .tabs
            .iter()
            .filter(|t| t.kind != Kind::Normal || (restore && t.url != "about:blank"))
            .map(|t| Saved {
                id: t.id.clone(),
                profile_id: t.profile_id.clone(),
                space_id: t.space_id.clone(),
                kind: t.kind,
                folder_id: t.folder_id.clone(),
                // Pinned/favorite tabs reset to `home`; normal tabs resume where they were.
                url: if t.kind == Kind::Normal {
                    t.url.clone()
                } else {
                    t.home.clone()
                },
                title: t.title.clone(),
                // Large inline icons are kept in memory only, not written to disk.
                favicon: t.favicon.clone().filter(|f| f.len() <= FAVICON_PERSIST_MAX),
            })
            .collect();
        self.p.active_tabs = if restore {
            self.active.clone()
        } else {
            HashMap::new()
        };
        let live_collapsed = self.p.sidebar_collapsed;
        if let Some(sp) = &self.split {
            self.p.sidebar_collapsed = sp.prev_collapsed;
        }
        let ok = self.p.save(&self.root.join("state.json"));
        self.p.sidebar_collapsed = live_collapsed;
        self.last_save = Instant::now();
        if ok {
            self.save_dirty = false;
        } else {
            log::warn!("saving state.json failed; will retry");
        }
    }
}

/// The browser's data folder (created on first run). It used to be called ArcRS: carry the
/// user's tabs, logins and extensions over.
pub(super) fn prepare_data_dir() -> Result<PathBuf> {
    let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    let root = base.join("Poof");
    let old = base.join("ArcRS");
    if old.is_dir() && !root.exists() {
        let _ = fs::rename(&old, &root);
    }
    fs::create_dir_all(root.join("extensions"))?;
    Ok(root)
}

/// Loads state.json and repairs values the UI could not have produced.
pub(super) fn load_state(root: &Path) -> Persisted {
    let mut p = Persisted::load(&root.join("state.json"));
    if !matches!(
        p.devtools_position.as_str(),
        "right" | "left" | "bottom" | "top"
    ) {
        p.devtools_position = "right".into();
    }
    if !p.devtools_size.is_finite() || p.devtools_size < PANEL_MIN {
        p.devtools_size = PANEL_DEFAULT;
    }
    p
}
