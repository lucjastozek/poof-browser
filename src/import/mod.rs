//! Importing from other browsers. Everything here is plain data in, plain data out: it runs on a
//! background thread, touches no browser state, and hands the result to the main loop
//! (`browser/import.rs`), which decides what to create.

mod arc_location;
mod arc_sidebar;
mod chromium_logins;
mod dpapi;

use anyhow::{bail, Result};
use std::collections::{BTreeMap, BTreeSet};

/// What the user asked to bring over.
#[derive(Clone, Copy, Debug)]
pub struct ArcOptions {
    /// Spaces with their pinned tabs, folders and favorites (and the profiles they use).
    pub spaces: bool,
    pub passwords: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImportedTab {
    pub url: String,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PinnedEntry {
    Tab(ImportedTab),
    Folder {
        name: String,
        tabs: Vec<ImportedTab>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArcSpace {
    pub name: String,
    /// One of poof's built-in space icon ids.
    pub icon: String,
    /// `#rrggbb`
    pub color: String,
    /// Directory of the Arc profile the space uses, inside Arc's `User Data`.
    pub profile_dir: String,
    pub pinned: Vec<PinnedEntry>,
}

/// A saved login as Arc had it, already decrypted. Handle like any other secret.
pub struct Login {
    pub url: String,
    pub username: String,
    pub password: String,
}

pub struct ArcProfile {
    pub dir: String,
    pub name: String,
    pub favorites: Vec<ImportedTab>,
    pub logins: Vec<Login>,
}

/// Everything read from Arc, ready to be turned into poof profiles, spaces and logins.
#[derive(Default)]
pub struct ArcImport {
    pub profiles: Vec<ArcProfile>,
    pub spaces: Vec<ArcSpace>,
    /// Things the user should know about (skipped passwords, unreadable files).
    pub notes: Vec<String>,
}

/// Reads Arc's data folder. Fails with a message fit to show the user.
pub fn import_arc(options: ArcOptions) -> Result<ArcImport> {
    let arc = arc_location::locate()?;
    let names = arc_location::profile_names(&arc.user_data);
    let mut out = ArcImport::default();

    let mut favorites: BTreeMap<String, Vec<ImportedTab>> = BTreeMap::new();
    let mut dirs: BTreeSet<String> = BTreeSet::new();
    if options.spaces {
        let sidebar = arc_location::read_sidebar(&arc)?;
        dirs.extend(sidebar.spaces.iter().map(|s| s.profile_dir.clone()));
        for (dir, tab) in sidebar.favorites {
            dirs.insert(dir.clone());
            favorites.entry(dir).or_default().push(tab);
        }
        out.spaces = sidebar.spaces;
    }
    if options.passwords {
        dirs.extend(arc_location::profiles_with_logins(&arc.user_data));
    }
    if dirs.is_empty() {
        bail!("Arc has nothing to import yet");
    }

    let mut logins = if options.passwords {
        read_all_logins(&arc.user_data, &dirs, &mut out.notes)
    } else {
        BTreeMap::new()
    };
    for dir in dirs {
        out.profiles.push(ArcProfile {
            name: arc_location::display_name(&names, &dir),
            favorites: favorites.remove(&dir).unwrap_or_default(),
            logins: logins.remove(&dir).unwrap_or_default(),
            dir,
        });
    }
    Ok(out)
}

/// Saved logins per profile directory. A failure is reported as a note: the spaces still import.
fn read_all_logins(
    user_data: &std::path::Path,
    dirs: &BTreeSet<String>,
    notes: &mut Vec<String>,
) -> BTreeMap<String, Vec<Login>> {
    let mut out = BTreeMap::new();
    let key = match chromium_logins::master_key(user_data) {
        Ok(key) => key,
        Err(error) => {
            notes.push(format!("Passwords were not imported: {error:#}"));
            return out;
        }
    };
    for dir in dirs {
        match chromium_logins::read_logins(user_data, dir, &key) {
            Ok(report) => {
                if report.unsupported > 0 {
                    notes.push(format!(
                        "{} passwords use a newer Arc encryption and were skipped.",
                        report.unsupported
                    ));
                }
                if !report.logins.is_empty() {
                    out.insert(dir.clone(), report.logins);
                }
            }
            Err(error) => notes.push(format!("Passwords were not imported: {error:#}")),
        }
    }
    out
}
