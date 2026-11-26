//! Importing from Arc. The files are read on a background thread (`crate::import`); this module
//! turns what was read into poof profiles, spaces, pinned tabs, folders, favorites and logins.
//! Nothing existing is replaced: a profile, space or login that is already here is kept as it is.

use super::*;
use crate::{
    import::{self, ArcImport, ArcOptions, ImportedTab, PinnedEntry},
    passwords::{origin_of, NewLogin},
};

/// Tabs (pinned and favorites together) one import may add; a corrupt file cannot flood the UI.
const MAX_IMPORTED_TABS: usize = 2000;
const DEFAULT_COLOR: &str = "#6dc5e2";

#[derive(Default)]
struct Stats {
    profiles: usize,
    spaces: usize,
    spaces_kept: usize,
    tabs: usize,
    favorites: usize,
    passwords: usize,
    passwords_kept: usize,
    passwords_unusable: usize,
}

impl Browser {
    pub(super) fn start_arc_import(&mut self, spaces: bool, passwords: bool) {
        if self.importing {
            return;
        }
        if !spaces && !passwords {
            self.report_import(false, "Choose what to import.");
            return;
        }
        self.importing = true;
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let read = std::panic::catch_unwind(|| {
                import::import_arc(ArcOptions { spaces, passwords }).map_err(|e| format!("{e:#}"))
            });
            let result =
                read.unwrap_or_else(|_| Err("Reading Arc's data failed unexpectedly.".into()));
            let _ = proxy.send_event(UserEvent::ImportDone(Box::new(result)));
        });
    }

    pub(super) fn finish_arc_import(&mut self, result: Result<ArcImport, String>) {
        self.importing = false;
        match result {
            Ok(data) => {
                let message = self.apply_arc_import(data);
                self.report_import(true, &message);
            }
            Err(message) => self.report_import(false, &message),
        }
    }

    fn report_import(&self, ok: bool, message: &str) {
        self.call_sidebar("__importFinished", &json!({ "ok": ok, "message": message }));
    }

    fn apply_arc_import(&mut self, data: ArcImport) -> String {
        let mut stats = Stats::default();
        let mut notes = data.notes.clone();
        let mut profile_ids: HashMap<String, String> = HashMap::new();

        for space in &data.spaces {
            let profile_id =
                self.imported_profile(&data, &space.profile_dir, &mut profile_ids, &mut stats);
            self.import_space(space, &profile_id, &mut stats);
        }
        for profile in &data.profiles {
            if profile.favorites.is_empty() && profile.logins.is_empty() {
                continue;
            }
            let profile_id =
                self.imported_profile(&data, &profile.dir, &mut profile_ids, &mut stats);
            for favorite in &profile.favorites {
                if self.import_tab(&profile_id, None, None, Kind::Favorite, favorite, &stats) {
                    stats.favorites += 1;
                }
            }
            let mut logins = Vec::new();
            for login in &profile.logins {
                match origin_of(&login.url) {
                    Some(origin) => logins.push(NewLogin {
                        profile_id: profile_id.clone(),
                        origin,
                        username: login.username.clone(),
                        password: login.password.clone(),
                    }),
                    None => stats.passwords_unusable += 1,
                }
            }
            match self.passwords.import_logins(logins) {
                Ok(counts) => {
                    stats.passwords += counts.added;
                    stats.passwords_kept += counts.already_saved;
                    stats.passwords_unusable += counts.rejected;
                }
                Err(error) => notes.push(format!("Passwords could not be saved: {error:#}")),
            }
        }

        self.save();
        self.push_state();
        summary(&stats, &notes)
    }

    /// The poof profile for an Arc profile directory: one with the same name if it exists,
    /// otherwise a new one.
    fn imported_profile(
        &mut self,
        data: &ArcImport,
        dir: &str,
        ids: &mut HashMap<String, String>,
        stats: &mut Stats,
    ) -> String {
        if let Some(id) = ids.get(dir) {
            return id.clone();
        }
        let name = data
            .profiles
            .iter()
            .find(|p| p.dir == dir)
            .map(|p| clip(p.name.trim(), 40))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "Arc".into());
        let existing = self
            .p
            .profiles
            .iter()
            .find(|p| p.name.to_lowercase() == name.to_lowercase())
            .map(|p| p.id.clone());
        let id = existing.unwrap_or_else(|| {
            let profile = Profile::new(name);
            let id = profile.id.clone();
            self.p.profiles.push(profile);
            stats.profiles += 1;
            id
        });
        ids.insert(dir.to_string(), id.clone());
        id
    }

    fn import_space(&mut self, space: &import::ArcSpace, profile_id: &str, stats: &mut Stats) {
        let name = clip(space.name.trim(), 40);
        let name = if name.is_empty() {
            "Space".into()
        } else {
            name
        };
        if self
            .p
            .spaces
            .iter()
            .any(|s| s.profile_id == profile_id && s.name == name)
        {
            stats.spaces_kept += 1;
            return;
        }
        let space_id = new_id();
        self.p.spaces.push(Space {
            id: space_id.clone(),
            profile_id: profile_id.to_string(),
            name,
            icon: clean_icon(&space.icon),
            color: if ok_color(&space.color) {
                space.color.clone()
            } else {
                DEFAULT_COLOR.into()
            },
        });
        stats.spaces += 1;

        // `order` is the position in the pinned section, counting loose tabs and folders alike.
        let mut order = 0;
        for entry in &space.pinned {
            match entry {
                PinnedEntry::Tab(tab) => {
                    if self.import_tab(profile_id, Some(&space_id), None, Kind::Pinned, tab, stats)
                    {
                        stats.tabs += 1;
                        order += 1;
                    }
                }
                PinnedEntry::Folder { name, tabs } => {
                    let folder_id = new_id();
                    let folder_name = clip(name.trim(), 40);
                    self.p.folders.push(TabFolder {
                        id: folder_id.clone(),
                        space_id: space_id.clone(),
                        name: if folder_name.is_empty() {
                            "Folder".into()
                        } else {
                            folder_name
                        },
                        collapsed: false,
                        pinned: true,
                        order,
                    });
                    let mut added = 0;
                    for tab in tabs {
                        let sid = Some(space_id.as_str());
                        if self.import_tab(
                            profile_id,
                            sid,
                            Some(&folder_id),
                            Kind::Pinned,
                            tab,
                            stats,
                        ) {
                            stats.tabs += 1;
                            added += 1;
                        }
                    }
                    if added == 0 {
                        self.p.folders.retain(|f| f.id != folder_id);
                    } else {
                        order += 1;
                    }
                }
            }
        }
    }

    /// Adds one saved tab (it loads when first shown). False when it is skipped.
    fn import_tab(
        &mut self,
        profile_id: &str,
        space_id: Option<&str>,
        folder_id: Option<&str>,
        kind: Kind,
        tab: &ImportedTab,
        stats: &Stats,
    ) -> bool {
        if stats.tabs + stats.favorites >= MAX_IMPORTED_TABS || !ok_url(&tab.url, 4096, false) {
            return false;
        }
        if kind == Kind::Favorite
            && self
                .tabs
                .iter()
                .any(|t| t.kind == Kind::Favorite && t.profile_id == profile_id && t.url == tab.url)
        {
            return false;
        }
        let saved = Saved {
            id: new_id(),
            profile_id: profile_id.to_string(),
            space_id: space_id.map(str::to_string),
            kind,
            url: tab.url.clone(),
            title: clip(tab.title.trim(), 300),
            favicon: None,
            folder_id: folder_id.map(str::to_string),
        };
        self.tabs.push(Tab::from_saved(&saved));
        true
    }
}

fn summary(stats: &Stats, notes: &[String]) -> String {
    let plural =
        |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let mut parts = Vec::new();
    for (n, one, many) in [
        (stats.profiles, "profile", "profiles"),
        (stats.spaces, "space", "spaces"),
        (stats.tabs, "pinned tab", "pinned tabs"),
        (stats.favorites, "favorite", "favorites"),
        (stats.passwords, "password", "passwords"),
    ] {
        if n > 0 {
            parts.push(plural(n, one, many));
        }
    }
    let mut text = if parts.is_empty() {
        "Nothing new to import.".to_string()
    } else {
        format!("Imported {}.", parts.join(", "))
    };
    let mut kept = Vec::new();
    if stats.spaces_kept > 0 {
        kept.push(plural(stats.spaces_kept, "space", "spaces"));
    }
    if stats.passwords_kept > 0 {
        kept.push(plural(stats.passwords_kept, "password", "passwords"));
    }
    if !kept.is_empty() {
        text.push_str(&format!(" Already here: {}.", kept.join(", ")));
    }
    if stats.passwords_unusable > 0 {
        text.push_str(&format!(
            " {} skipped (not https or no room).",
            plural(stats.passwords_unusable, "password", "passwords")
        ));
    }
    for note in notes {
        text.push(' ');
        text.push_str(note);
    }
    text
}
