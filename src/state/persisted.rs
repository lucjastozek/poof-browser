//! Everything that lives in state.json: preferences plus the user's spaces, folders and tabs.

use super::{model::*, storage::write_json_atomically};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};

fn yes() -> bool {
    true
}

fn system() -> String {
    "system".into()
}

fn default_sidebar_width() -> f64 {
    280.0
}
fn default_devtools_position() -> String {
    "right".into()
}
fn default_devtools_size() -> f64 {
    420.0
}
/// Background tabs release their WebView (and its renderer process) after this many idle minutes.
fn default_tab_sleep_minutes() -> u64 {
    10
}

#[derive(Serialize, Deserialize)]
pub struct Persisted {
    pub profiles: Vec<Profile>,
    pub spaces: Vec<Space>,
    pub saved: Vec<Saved>,
    #[serde(default)]
    pub folders: Vec<TabFolder>,
    /// One-time migration away from the previous always-pinned folder behavior.
    #[serde(default)]
    pub folder_layout_version: u8,
    pub current_space: String,
    #[serde(default = "yes")]
    pub auto_pip: bool,
    /// Reopen the tabs that were open when the browser was closed.
    #[serde(default)]
    pub restore_tabs: bool,
    /// Last active tab per space (space id -> tab id); only kept while `restore_tabs` is on.
    #[serde(default)]
    pub active_tabs: HashMap<String, String>,
    /// "system" | "light" | "dark"
    #[serde(default = "system")]
    pub theme: String,
    /// Width of the expanded sidebar in logical pixels.
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: f64,
    /// Whether the sidebar is fully hidden.
    #[serde(default)]
    pub sidebar_collapsed: bool,
    /// Show the sidebar as a compact icon rail when it is not collapsed.
    #[serde(default)]
    pub sidebar_compact: bool,
    /// Dock side for the embedded WebView2 DevTools frontend.
    #[serde(default = "default_devtools_position")]
    pub devtools_position: String,
    /// Logical pixels allocated to the DevTools pane.
    #[serde(default = "default_devtools_size")]
    pub devtools_size: f64,
    #[serde(default)]
    pub sites: HashMap<String, Site>,
    /// Store extensions we already tried to preinstall (so removing one sticks).
    #[serde(default)]
    pub preinstalled: Vec<String>,
    /// Minutes of inactivity before a background tab is unloaded from memory (0 = never).
    /// Tabs playing audio are never unloaded. The tab stays in the sidebar and reloads on click.
    #[serde(default = "default_tab_sleep_minutes")]
    pub tab_sleep_minutes: u64,
}

impl Default for Persisted {
    fn default() -> Self {
        let pid = new_id();
        let sid = new_id();
        Self {
            profiles: vec![Profile {
                id: pid.clone(),
                name: "Personal".into(),
                tab_retention_hours: default_tab_retention(),
                cookie_retention_hours: 0,
                cache_retention_hours: 0,
                history_retention_hours: 0,
                last_cleanup_at: 0,
            }],
            spaces: vec![Space {
                id: sid.clone(),
                profile_id: pid,
                name: "Home".into(),
                icon: "house".into(),
                color: "#6dc5e2".into(),
            }],
            saved: vec![],
            folders: vec![],
            folder_layout_version: 1,
            current_space: sid,
            auto_pip: true,
            restore_tabs: false,
            active_tabs: HashMap::new(),
            theme: system(),
            sidebar_width: default_sidebar_width(),
            sidebar_collapsed: false,
            sidebar_compact: false,
            devtools_position: default_devtools_position(),
            devtools_size: default_devtools_size(),
            sites: HashMap::new(),
            preinstalled: vec![],
            tab_sleep_minutes: default_tab_sleep_minutes(),
        }
    }
}

impl Persisted {
    pub(super) fn sanitize_devtools_preferences(&mut self) {
        if !matches!(
            self.devtools_position.as_str(),
            "right" | "left" | "bottom" | "top"
        ) {
            self.devtools_position = default_devtools_position();
        }
        if !self.devtools_size.is_finite() {
            self.devtools_size = default_devtools_size();
        }
        self.devtools_size = self.devtools_size.clamp(240.0, 800.0);
    }

    /// Keeps the 400 most-visited sites (single sort instead of repeated scans).
    pub fn prune_sites(&mut self) {
        const MAX: usize = 400;
        if self.sites.len() <= MAX {
            return;
        }
        let mut ranked: Vec<(String, u32)> = self
            .sites
            .iter()
            .map(|(k, s)| (k.clone(), s.count))
            .collect();
        ranked.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        let excess = self.sites.len() - MAX;
        for (key, _) in ranked.into_iter().take(excess) {
            self.sites.remove(&key);
        }
    }

    fn read(path: &Path) -> Option<Self> {
        // Parse straight from the file; no second in-memory copy of the whole document.
        let file = fs::File::open(path).ok()?;
        serde_json::from_reader(std::io::BufReader::new(file)).ok()
    }

    pub fn load(path: &Path) -> Self {
        // Fall back to the last good backup if the main file is missing or corrupt.
        let mut p: Self = Self::read(path)
            .or_else(|| Self::read(&path.with_extension("json.bak")))
            .unwrap_or_default();
        if p.spaces.is_empty() || p.profiles.is_empty() {
            p = Self::default();
        }
        // The previous folder implementation forced every folder and its tabs into Pinned.
        // Convert those legacy folders to ordinary tabs once, preserving membership.
        if p.folder_layout_version < 1 {
            for folder in p.folders.iter_mut() {
                folder.pinned = false;
            }
            for saved in p.saved.iter_mut().filter(|s| s.folder_id.is_some()) {
                saved.kind = Kind::Normal;
            }
            p.folder_layout_version = 1;
        }
        // Older versions stored an emoji as the space icon; swap those for a real icon.
        for s in p.spaces.iter_mut() {
            if !valid_icon(&s.icon) {
                s.icon = "house".into();
            }
        }
        if !p.sidebar_width.is_finite() {
            p.sidebar_width = default_sidebar_width();
        }
        p.sidebar_width = p.sidebar_width.clamp(180.0, 480.0);
        p.sanitize_devtools_preferences();
        p.tab_sleep_minutes = p.tab_sleep_minutes.min(10_080);
        if !matches!(p.theme.as_str(), "system" | "light" | "dark") {
            p.theme = system();
        }
        if !p.spaces.iter().any(|s| s.id == p.current_space) {
            p.current_space = p.spaces[0].id.clone();
        }
        p
    }

    /// Writes compact JSON atomically and keeps the previous file as `.bak`.
    /// Returns whether the save succeeded.
    pub fn save(&self, path: &Path) -> bool {
        write_json_atomically(self, path, true)
    }
}
