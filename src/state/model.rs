//! Plain data the browser persists: profiles, spaces, folders and saved tabs.

use serde::{Deserialize, Serialize};

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

impl Profile {
    /// A profile with the default cleanup rules.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: new_id(),
            name: name.into(),
            tab_retention_hours: default_tab_retention(),
            cookie_retention_hours: 0,
            cache_retention_hours: 0,
            history_retention_hours: 0,
            last_cleanup_at: 0,
        }
    }
}

pub(super) fn default_tab_retention() -> u64 {
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
