use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};

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
    /// Ephemeral (not persisted yet; Arc archives these, we just drop them).
    Normal,
}

/// A profile = isolated cookies/storage/extensions (its own WebView2 user-data folder).
#[derive(Serialize, Deserialize, Clone)]
pub struct Profile {
    pub id: String,
    pub name: String,
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
pub struct Saved {
    pub id: String,
    pub profile_id: String,
    pub space_id: Option<String>,
    pub kind: Kind,
    pub url: String,
    pub title: String,
    pub favicon: Option<String>,
}

fn yes() -> bool {
    true
}

fn system() -> String {
    "system".into()
}

/// Space icons are ids of the built-in icon set (e.g. "house"), never emoji.
pub fn valid_icon(s: &str) -> bool {
    let builtin = !s.is_empty() && s.len() <= 32
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if builtin { return true; }
    let Some(encoded) = s.strip_prefix("svg:") else { return false };
    if encoded.len() > 8192 || encoded.is_empty() { return false; }
    use base64::Engine;
    let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(encoded) else { return false };
    let Ok(svg) = std::str::from_utf8(&bytes) else { return false };
    let lower = svg.to_ascii_lowercase();
    svg.len() <= 6144 && lower.starts_with("<svg")
        && !lower.contains("<script") && !lower.contains("<foreignobject")
        && !lower.contains("<iframe") && !lower.contains("<image")
        && !lower.contains("<!doctype") && !lower.contains("<!entity")
        && !lower.contains("javascript:") && !lower.contains("url(")
        && !lower.contains(" onload") && !lower.contains(" onclick")
        && !lower.contains(" onerror") && !lower.contains("href=")
}

#[derive(Serialize, Deserialize)]
pub struct Persisted {
    pub profiles: Vec<Profile>,
    pub spaces: Vec<Space>,
    pub saved: Vec<Saved>,
    pub current_space: String,
    #[serde(default = "yes")]
    pub auto_pip: bool,
    /// "system" | "light" | "dark"
    #[serde(default = "system")]
    pub theme: String,
    /// Whether the sidebar starts collapsed.
    #[serde(default)]
    pub sidebar_collapsed: bool,
    #[serde(default)]
    pub sites: HashMap<String, Site>,
    /// Store extensions we already tried to preinstall (so removing one sticks).
    #[serde(default)]
    pub preinstalled: Vec<String>,
}

impl Default for Persisted {
    fn default() -> Self {
        let pid = new_id();
        let sid = new_id();
        Self {
            profiles: vec![Profile { id: pid.clone(), name: "Personal".into() }],
            spaces: vec![Space {
                id: sid.clone(),
                profile_id: pid,
                name: "Home".into(),
                icon: "house".into(),
                color: "#6dc5e2".into(),
            }],
            saved: vec![],
            current_space: sid,
            auto_pip: true,
            theme: system(),
            sidebar_collapsed: false,
            sites: HashMap::new(),
            preinstalled: vec![],
        }
    }
}

impl Persisted {
    pub fn prune_sites(&mut self) {
        while self.sites.len() > 400 {
            let Some(k) = self.sites.iter().min_by_key(|(_, s)| s.count).map(|(k, _)| k.clone()) else { break };
            self.sites.remove(&k);
        }
    }

    pub fn load(path: &Path) -> Self {
        let mut p: Self = fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        if p.spaces.is_empty() || p.profiles.is_empty() {
            p = Self::default();
        }
        // Older versions stored an emoji as the space icon; swap those for a real icon.
        for s in p.spaces.iter_mut() {
            if !valid_icon(&s.icon) {
                s.icon = "house".into();
            }
        }
        if !matches!(p.theme.as_str(), "system" | "light" | "dark") {
            p.theme = system();
        }
        if !p.spaces.iter().any(|s| s.id == p.current_space) {
            p.current_space = p.spaces[0].id.clone();
        }
        p
    }

    pub fn save(&self, path: &Path) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let tmp = path.with_extension("json.tmp");
            if fs::write(&tmp, json).is_ok() {
                let _ = fs::rename(&tmp, path);
            }
        }
    }
}
