use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, io::Write, path::Path};

pub fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Shared by every space of the same profile.
    Favorite,
    /// Belongs to one space, persisted.
    Pinned,
    /// Ephemeral unless `restore_tabs` is on, in which case it is persisted too.
    Normal,
}

/// A profile = isolated cookies/storage/extensions (its own WebView2 user-data folder).
#[derive(Serialize, Deserialize, Clone)]
pub struct Profile {
    pub id: String,
    pub name: String,
    #[serde(default = "default_tab_retention")]
    pub tab_retention_hours: u64,
    #[serde(default)]
    pub cookie_retention_hours: u64,
    #[serde(default)]
    pub cache_retention_hours: u64,
    #[serde(default)]
    pub history_retention_hours: u64,
    #[serde(default)]
    pub last_cleanup_at: u64,
}

fn default_tab_retention() -> u64 {
    48
}

/// Many spaces can belong to one profile.
#[derive(Serialize, Deserialize, Clone)]
pub struct Space {
    pub id: String,
    pub profile_id: String,
    pub name: String,
    pub icon: String,
    /// Single flat accent color (older state files called this `c1`).
    #[serde(alias = "c1")]
    pub color: String,
}

/// Local-only visit stats used to rank the new-tab suggestions.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Site {
    pub url: String,
    pub favicon: Option<String>,
    pub count: u32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TabFolder {
    pub id: String,
    pub space_id: String,
    pub name: String,
    #[serde(default)]
    pub collapsed: bool,
    #[serde(default)]
    pub pinned: bool,
    /// Position among loose tabs and folders in the sidebar section.
    #[serde(default = "folder_order_default")]
    pub order: usize,
}

fn folder_order_default() -> usize {
    usize::MAX
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Saved {
    pub id: String,
    pub profile_id: String,
    pub space_id: Option<String>,
    pub kind: Kind,
    pub url: String,
    pub title: String,
    pub favicon: Option<String>,
    #[serde(default)]
    pub folder_id: Option<String>,
}

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

/// Space icons are ids of the built-in icon set (e.g. "house"), never emoji.
pub fn valid_icon(s: &str) -> bool {
    let builtin = !s.is_empty()
        && s.len() <= 32
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if builtin {
        return true;
    }
    let Some(encoded) = s.strip_prefix("svg:") else {
        return false;
    };
    if encoded.len() > 8192 || encoded.is_empty() {
        return false;
    }
    use base64::Engine;
    let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(encoded) else {
        return false;
    };
    let Ok(svg) = std::str::from_utf8(&bytes) else {
        return false;
    };
    let lower = svg.to_ascii_lowercase();
    svg.len() <= 6144
        && lower.starts_with("<svg")
        && !lower.contains("<script")
        && !lower.contains("<foreignobject")
        && !lower.contains("<iframe")
        && !lower.contains("<image")
        && !lower.contains("<!doctype")
        && !lower.contains("<!entity")
        && !lower.contains("javascript:")
        && !lower.contains("url(")
        && !lower.contains(" onload")
        && !lower.contains(" onclick")
        && !lower.contains(" onerror")
        && !lower.contains("href=")
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
    fn sanitize_devtools_preferences(&mut self) {
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
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
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

    /// Writes compact JSON to a temp file, fsyncs, keeps the previous file as `.bak`, then
    /// swaps the new one in. Returns whether the save succeeded.
    pub fn save(&self, path: &Path) -> bool {
        let Ok(json) = serde_json::to_vec(self) else {
            return false;
        };
        let tmp = path.with_extension("json.tmp");
        let written = (|| -> std::io::Result<()> {
            let mut file = fs::File::create(&tmp)?;
            file.write_all(&json)?;
            file.sync_all()?;
            Ok(())
        })();
        if written.is_err() {
            let _ = fs::remove_file(&tmp);
            return false;
        }
        if path.exists() {
            let _ = fs::copy(path, path.with_extension("json.bak"));
        }
        fs::rename(&tmp, path).is_ok()
    }
}

/// One visited page. Stored in history.json, separate from state.json, so frequent writes stay small.
#[derive(Serialize, Deserialize, Clone)]
pub struct HistoryEntry {
    pub id: String,
    pub profile_id: String,
    pub url: String,
    #[serde(default)]
    pub title: String,
    /// Unix seconds.
    pub ts: u64,
}

pub fn load_history(path: &Path) -> Vec<HistoryEntry> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_history(entries: &[HistoryEntry], path: &Path) -> bool {
    let Ok(json) = serde_json::to_vec(entries) else {
        return false;
    };
    let tmp = path.with_extension("json.tmp");
    let written = (|| -> std::io::Result<()> {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(&json)?;
        file.sync_all()?;
        Ok(())
    })();
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
        return false;
    }
    fs::rename(&tmp, path).is_ok()
}

#[cfg(test)]
mod devtools_state_tests {
    use super::*;

    #[test]
    fn devtools_preferences_default_to_a_right_docked_panel() {
        let state = Persisted::default();
        assert_eq!(state.devtools_position, "right");
        assert_eq!(state.devtools_size, 420.0);
    }

    #[test]
    fn invalid_devtools_preferences_are_sanitized() {
        let mut state = Persisted {
            devtools_position: "floating".into(),
            devtools_size: f64::NAN,
            ..Persisted::default()
        };
        state.sanitize_devtools_preferences();
        assert_eq!(state.devtools_position, "right");
        assert_eq!(state.devtools_size, 420.0);

        state.devtools_size = 2_000.0;
        state.sanitize_devtools_preferences();
        assert_eq!(state.devtools_size, 800.0);
    }

    #[test]
    fn restore_tabs_is_off_by_default() {
        let state = Persisted::default();
        assert!(!state.restore_tabs);
        assert!(state.active_tabs.is_empty());
    }

    #[test]
    fn background_tabs_sleep_by_default() {
        assert_eq!(Persisted::default().tab_sleep_minutes, 10);
    }

    #[test]
    fn prune_sites_keeps_the_most_visited() {
        let mut state = Persisted::default();
        for i in 0..450u32 {
            state.sites.insert(
                format!("h{i}.test"),
                Site {
                    url: String::new(),
                    favicon: None,
                    count: i,
                },
            );
        }
        state.prune_sites();
        assert_eq!(state.sites.len(), 400);
        assert!(state.sites.contains_key("h449.test"));
        assert!(!state.sites.contains_key("h0.test"));
    }
}
