//! Reads Arc's `StorableSidebar.json`: spaces, their pinned tabs and folders, and favorites.
//!
//! Arc stores the sidebar as `sidebar.containers[]`, whose `spaces` and `items` arrays alternate
//! between an id string and the object it names. Items form a tree through `childrenIds`. The
//! format is undocumented and has changed between Arc versions, so everything is read
//! defensively: unknown shapes are skipped, never an error.

use super::{ArcSpace, ImportedTab, PinnedEntry};
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::HashMap;

/// Arc's first profile lives in `User Data/Default`.
pub const DEFAULT_PROFILE_DIR: &str = "Default";
const DEFAULT_COLOR: &str = "#6dc5e2";
const DEFAULT_ICON: &str = "house";
/// Folders inside folders are flattened; this only stops a corrupt, looping tree.
const MAX_DEPTH: usize = 8;
const URL_MAX: usize = 4096;

/// Everything the sidebar file holds that poof can use.
#[derive(Debug, Default, PartialEq)]
pub struct Sidebar {
    pub spaces: Vec<ArcSpace>,
    /// Favorites ("top apps") with the Arc profile directory they belong to.
    pub favorites: Vec<(String, ImportedTab)>,
}

/// Item ids to their objects, plus the children of each parent in file order.
struct Items<'a> {
    by_id: HashMap<&'a str, &'a Value>,
    children_of: HashMap<&'a str, Vec<&'a str>>,
}

impl<'a> Items<'a> {
    fn new(items: &[&'a Value]) -> Self {
        let mut by_id = HashMap::new();
        let mut children_of: HashMap<&str, Vec<&str>> = HashMap::new();
        for item in items {
            let Some(id) = item["id"].as_str() else {
                continue;
            };
            by_id.insert(id, *item);
            if let Some(parent) = item["parentID"].as_str() {
                children_of.entry(parent).or_default().push(id);
            }
        }
        Self { by_id, children_of }
    }

    /// Children in display order: `childrenIds` when present, else file order by `parentID`.
    fn children(&self, id: &str) -> Vec<&'a Value> {
        let listed: Option<Vec<&str>> = self
            .by_id
            .get(id)
            .and_then(|item| item["childrenIds"].as_array())
            .map(|ids| ids.iter().filter_map(Value::as_str).collect());
        let ids = match listed {
            Some(ids) if !ids.is_empty() => ids,
            _ => self.children_of.get(id).cloned().unwrap_or_default(),
        };
        ids.into_iter()
            .filter_map(|child| self.by_id.get(child).copied())
            .collect()
    }
}

pub fn parse(json: &str) -> Result<Sidebar> {
    let root: Value = serde_json::from_str(json)?;
    let containers = root["sidebar"]["containers"]
        .as_array()
        .ok_or_else(|| anyhow!("this does not look like Arc's sidebar file"))?;

    let objects = |key: &str| -> Vec<&Value> {
        containers
            .iter()
            .filter_map(|c| c[key].as_array())
            .flatten()
            .filter(|v| v.is_object())
            .collect()
    };
    let items = Items::new(&objects("items"));

    let mut sidebar = Sidebar::default();
    for (index, space) in objects("spaces").into_iter().enumerate() {
        sidebar.spaces.push(parse_space(space, index, &items));
    }
    for container in containers {
        sidebar.favorites.extend(parse_favorites(container, &items));
    }
    Ok(sidebar)
}

fn parse_space(space: &Value, index: usize, items: &Items) -> ArcSpace {
    let name = space["title"]
        .as_str()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("Space {}", index + 1));
    let pinned = container_after(space, "pinned")
        .map(|id| pinned_entries(&id, items))
        .unwrap_or_default();
    ArcSpace {
        name,
        icon: icon_for(&space["customInfo"]["iconType"]),
        color: color_for(&space["customInfo"]),
        profile_dir: profile_dir(&space["profile"]),
        pinned,
    }
}

/// The id listed right after the `pinned` marker in a space's container list. Arc writes the
/// marker as the string `"pinned"` or as an object such as `{"pinned": {}}`.
fn container_after(space: &Value, marker: &str) -> Option<String> {
    let ids = space["newContainerIDs"]
        .as_array()
        .or_else(|| space["containerIDs"].as_array())?;
    let at = ids.iter().position(|v| {
        v.as_str() == Some(marker) || v.as_object().is_some_and(|o| o.contains_key(marker))
    })?;
    ids.get(at + 1)?.as_str().map(str::to_string)
}

fn pinned_entries(container: &str, items: &Items) -> Vec<PinnedEntry> {
    let mut out = Vec::new();
    for item in items.children(container) {
        if item["data"]["list"].is_object() {
            let mut tabs = Vec::new();
            collect_tabs(item, items, 0, &mut tabs);
            if !tabs.is_empty() {
                out.push(PinnedEntry::Folder {
                    name: item_title(item).unwrap_or_else(|| "Folder".into()),
                    tabs,
                });
            }
        } else {
            let mut tabs = Vec::new();
            collect_tabs(item, items, 0, &mut tabs);
            out.extend(tabs.into_iter().map(PinnedEntry::Tab));
        }
    }
    out
}

/// The tab in `item`, or every tab below it (folders, split views) in order.
fn collect_tabs(item: &Value, items: &Items, depth: usize, out: &mut Vec<ImportedTab>) {
    if let Some(tab) = tab_of(item) {
        out.push(tab);
        return;
    }
    if depth >= MAX_DEPTH {
        return;
    }
    let Some(id) = item["id"].as_str() else {
        return;
    };
    for child in items.children(id) {
        collect_tabs(child, items, depth + 1, out);
    }
}

fn tab_of(item: &Value) -> Option<ImportedTab> {
    let data = &item["data"]["tab"];
    let url = data["savedURL"].as_str()?.trim();
    let web = url.starts_with("https://") || url.starts_with("http://");
    if !web || url.len() > URL_MAX {
        return None;
    }
    let title = item_title(item)
        .or_else(|| {
            data["savedTitle"]
                .as_str()
                .map(str::trim)
                .map(str::to_string)
        })
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| url.to_string());
    Some(ImportedTab {
        url: url.to_string(),
        title,
    })
}

/// A name the user gave the item in Arc.
fn item_title(item: &Value) -> Option<String> {
    item["title"]
        .as_str()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
}

/// `topAppsContainerIDs` alternates a profile descriptor with the id of its favorites container.
fn parse_favorites(container: &Value, items: &Items) -> Vec<(String, ImportedTab)> {
    let Some(ids) = container["topAppsContainerIDs"].as_array() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut profile = DEFAULT_PROFILE_DIR.to_string();
    for entry in ids {
        match entry.as_str() {
            Some(id) => {
                let mut tabs = Vec::new();
                for item in items.children(id) {
                    collect_tabs(item, items, 0, &mut tabs);
                }
                out.extend(tabs.into_iter().map(|t| (profile.clone(), t)));
            }
            None => profile = profile_dir(entry),
        }
    }
    out
}

/// The Arc profile directory a space (or favorites list) belongs to: `{"custom": {"_0":
/// {"directoryBasename": "Profile 1"}}}`, or the default profile.
pub fn profile_dir(profile: &Value) -> String {
    find_string(profile, "directoryBasename", 0)
        .filter(|dir| safe_dir_name(dir))
        .unwrap_or_else(|| DEFAULT_PROFILE_DIR.to_string())
}

/// The name ends up in a file path, so only plain folder names are accepted.
pub fn safe_dir_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name != "."
        && name != ".."
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '_' || c == '-')
}

fn find_string(value: &Value, key: &str, depth: usize) -> Option<String> {
    if depth > MAX_DEPTH {
        return None;
    }
    match value {
        Value::Object(map) => map
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| map.values().find_map(|v| find_string(v, key, depth + 1))),
        _ => None,
    }
}

/// Maps Arc's emoji or symbol name to one of poof's built-in space icons.
fn icon_for(icon_type: &Value) -> String {
    let Some(raw) = find_any_string(icon_type, 0) else {
        return DEFAULT_ICON.into();
    };
    let key = raw.trim().trim_end_matches('\u{fe0f}').to_lowercase();
    ICONS
        .iter()
        .find(|(from, _)| *from == key)
        .map(|(_, to)| to.to_string())
        .unwrap_or_else(|| DEFAULT_ICON.into())
}

fn find_any_string(value: &Value, depth: usize) -> Option<String> {
    if depth > 4 {
        return None;
    }
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Object(map) => map.values().find_map(|v| find_any_string(v, depth + 1)),
        _ => None,
    }
}

/// Common Arc space icons (emoji, and SF Symbol names for its symbol picker).
const ICONS: &[(&str, &str)] = &[
    ("🏠", "house"),
    ("🏡", "house"),
    ("💼", "briefcase"),
    ("📚", "book-2"),
    ("📖", "book-2"),
    ("🎵", "music"),
    ("🎶", "music"),
    ("🎧", "headphone"),
    ("🎮", "game-1"),
    ("🚀", "rocket"),
    ("⭐", "star"),
    ("🌟", "star"),
    ("❤", "heart"),
    ("💖", "heart"),
    ("🔥", "fire"),
    ("🌍", "earth"),
    ("🌎", "earth"),
    ("🌏", "earth"),
    ("💻", "laptop"),
    ("🖥", "computer"),
    ("🛒", "shopping-cart-1"),
    ("💰", "currency-dollar"),
    ("💵", "currency-dollar"),
    ("📷", "camera"),
    ("🎬", "film"),
    ("✉", "mail"),
    ("📧", "mail"),
    ("🔒", "lock"),
    ("🎨", "palette"),
    ("🧪", "flask"),
    ("🌱", "leaf"),
    ("🌿", "leaf"),
    ("☀", "sun"),
    ("🌙", "moon"),
    ("🚗", "car"),
    ("🐱", "cat"),
    ("🐶", "dog"),
    ("🎓", "school"),
    ("🏦", "bank"),
    ("🛠", "tool"),
    ("🔧", "tool"),
    ("star", "star"),
    ("star.fill", "star"),
    ("heart", "heart"),
    ("heart.fill", "heart"),
    ("house", "house"),
    ("house.fill", "house"),
    ("bolt", "flash"),
    ("bolt.fill", "flash"),
    ("book", "book-2"),
    ("book.fill", "book-2"),
    ("gamecontroller", "game-1"),
    ("music.note", "music"),
    ("camera", "camera"),
    ("briefcase", "briefcase"),
    ("cart", "shopping-cart-1"),
    ("flame", "fire"),
    ("globe", "globe"),
    ("leaf", "leaf"),
    ("lock", "lock"),
    ("moon", "moon"),
    ("sun.max", "sun"),
    ("paintpalette", "palette"),
    ("terminal", "terminal"),
    ("hammer", "tool"),
];

/// The space's accent color as `#rrggbb`. Arc keeps it as 0..1 red/green/blue floats inside
/// `windowTheme`, preferring the palette's mid tone.
fn color_for(custom_info: &Value) -> String {
    let theme = &custom_info["windowTheme"];
    let mid = &theme["primaryColorPalette"]["midTone"];
    rgb_of(mid)
        .or_else(|| find_rgb(theme, 0))
        .map(|(r, g, b)| format!("#{:02x}{:02x}{:02x}", channel(r), channel(g), channel(b)))
        .unwrap_or_else(|| DEFAULT_COLOR.into())
}

fn channel(fraction: f64) -> u8 {
    (fraction.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn rgb_of(value: &Value) -> Option<(f64, f64, f64)> {
    Some((
        value["red"].as_f64()?,
        value["green"].as_f64()?,
        value["blue"].as_f64()?,
    ))
}

fn find_rgb(value: &Value, depth: usize) -> Option<(f64, f64, f64)> {
    if depth > MAX_DEPTH {
        return None;
    }
    match value {
        Value::Object(map) => {
            rgb_of(value).or_else(|| map.values().find_map(|v| find_rgb(v, depth + 1)))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r##"{
      "version": 1,
      "sidebar": { "containers": [
        { "global": {}, "topAppsContainerIDs": [
            {"default": true}, "top-default",
            {"custom": {"_0": {"directoryBasename": "Profile 1"}}}, "top-work"
        ]},
        { "spaces": [
            "space-1",
            { "id": "space-1", "title": "Personal",
              "newContainerIDs": [{"pinned": {}}, "pin-1", {"unpinned": {"_0": {"shared": {}}}}, "unpin-1"],
              "profile": {"default": true},
              "customInfo": {
                "iconType": {"emoji_v2": "🚀"},
                "windowTheme": {"primaryColorPalette": {"midTone": {"red": 1.0, "green": 0.5, "blue": 0.0, "alpha": 1}}}
              } },
            "space-2",
            { "id": "space-2", "title": "",
              "containerIDs": ["pinned", "pin-2", "unpinned", "unpin-2"],
              "profile": {"custom": {"_0": {"directoryBasename": "Profile 1"}}},
              "customInfo": {"iconType": {"icon": "star.fill"}} }
          ],
          "items": [
            "pin-1", {"id": "pin-1", "parentID": "space-1", "childrenIds": ["t1", "f1", "junk"], "title": null, "data": {"itemContainer": {}}},
            "t1", {"id": "t1", "parentID": "pin-1", "title": null, "data": {"tab": {"savedURL": "https://example.com/a", "savedTitle": "Example A"}}},
            "f1", {"id": "f1", "parentID": "pin-1", "title": "Docs", "childrenIds": ["t2", "t3"], "data": {"list": {}}},
            "t2", {"id": "t2", "parentID": "f1", "title": "Renamed", "data": {"tab": {"savedURL": "https://docs.rs/", "savedTitle": "docs.rs"}}},
            "t3", {"id": "t3", "parentID": "f1", "data": {"tab": {"savedURL": "javascript:alert(1)", "savedTitle": "bad"}}},
            "junk", {"id": "junk", "parentID": "pin-1", "data": {"easel": {}}},
            "pin-2", {"id": "pin-2", "parentID": "space-2", "data": {"itemContainer": {}}},
            "t4", {"id": "t4", "parentID": "pin-2", "data": {"tab": {"savedURL": "http://localhost:3000/", "savedTitle": ""}}},
            "top-default", {"id": "top-default", "childrenIds": ["t5"], "data": {"itemContainer": {}}},
            "t5", {"id": "t5", "parentID": "top-default", "data": {"tab": {"savedURL": "https://mail.google.com/", "savedTitle": "Gmail"}}},
            "top-work", {"id": "top-work", "data": {"itemContainer": {}}},
            "t6", {"id": "t6", "parentID": "top-work", "data": {"tab": {"savedURL": "https://github.com/", "savedTitle": "GitHub"}}}
          ] }
      ] } }"##;

    #[test]
    fn reads_spaces_with_profile_icon_and_color() {
        let sidebar = parse(SAMPLE).unwrap();
        assert_eq!(sidebar.spaces.len(), 2);
        let (a, b) = (&sidebar.spaces[0], &sidebar.spaces[1]);
        assert_eq!(
            (a.name.as_str(), a.icon.as_str(), a.color.as_str()),
            ("Personal", "rocket", "#ff8000")
        );
        assert_eq!(a.profile_dir, "Default");
        assert_eq!(b.name, "Space 2");
        assert_eq!((b.icon.as_str(), b.color.as_str()), ("star", "#6dc5e2"));
        assert_eq!(b.profile_dir, "Profile 1");
    }

    #[test]
    fn pinned_tabs_and_folders_keep_their_order_and_names() {
        let sidebar = parse(SAMPLE).unwrap();
        let tab = |url: &str, title: &str| ImportedTab {
            url: url.into(),
            title: title.into(),
        };
        assert_eq!(
            sidebar.spaces[0].pinned,
            vec![
                PinnedEntry::Tab(tab("https://example.com/a", "Example A")),
                PinnedEntry::Folder {
                    name: "Docs".into(),
                    tabs: vec![tab("https://docs.rs/", "Renamed")],
                },
            ]
        );
        // Falls back to the address when Arc has no title.
        assert_eq!(
            sidebar.spaces[1].pinned,
            vec![PinnedEntry::Tab(tab(
                "http://localhost:3000/",
                "http://localhost:3000/"
            ))]
        );
    }

    #[test]
    fn favorites_follow_their_profile() {
        let sidebar = parse(SAMPLE).unwrap();
        let favs: Vec<(&str, &str)> = sidebar
            .favorites
            .iter()
            .map(|(dir, t)| (dir.as_str(), t.url.as_str()))
            .collect();
        assert_eq!(
            favs,
            vec![
                ("Default", "https://mail.google.com/"),
                ("Profile 1", "https://github.com/")
            ]
        );
    }

    #[test]
    fn profile_directories_cannot_escape_the_data_folder() {
        let evil = serde_json::json!({"custom": {"_0": {"directoryBasename": "../../Windows"}}});
        assert_eq!(profile_dir(&evil), "Default");
        assert!(safe_dir_name("Profile 12"));
        assert!(!safe_dir_name("a/b") && !safe_dir_name("..") && !safe_dir_name(""));
    }

    #[test]
    fn rejects_files_that_are_not_the_sidebar() {
        assert!(parse("{}").is_err());
        assert!(parse("not json").is_err());
    }
}
