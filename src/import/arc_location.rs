//! Finding Arc's data on disk and reading the small files that describe it.

use super::arc_sidebar::{self, safe_dir_name, Sidebar, DEFAULT_PROFILE_DIR};
use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

const SIDEBAR_FILE: &str = "StorableSidebar.json";
/// The sidebar file is a few megabytes at most; anything bigger is not Arc's.
const SIDEBAR_MAX_BYTES: u64 = 256 * 1024 * 1024;

pub struct ArcLocation {
    /// Chromium's `User Data` folder: `Local State` and one folder per profile.
    pub user_data: PathBuf,
    pub sidebar: PathBuf,
}

/// Folders Arc may keep its data in. The Windows build is a packaged app, so its data lives
/// under `Packages\TheBrowserCompany.Arc_<id>\LocalCache\Local\Arc`.
fn candidate_roots() -> Vec<PathBuf> {
    #[allow(unused_mut)] // only the Windows and macOS builds add folders
    let mut roots = Vec::new();
    #[cfg(windows)]
    if let Some(local) = dirs::data_local_dir() {
        if let Ok(packages) = fs::read_dir(local.join("Packages")) {
            for package in packages.flatten() {
                let name = package.file_name();
                if name.to_string_lossy().starts_with("TheBrowserCompany.Arc") {
                    roots.push(package.path().join("LocalCache").join("Local").join("Arc"));
                }
            }
        }
        roots.push(local.join("Arc"));
    }
    #[cfg(target_os = "macos")]
    if let Some(support) = dirs::data_dir() {
        roots.push(support.join("Arc"));
    }
    roots
}

pub fn locate() -> Result<ArcLocation> {
    for root in candidate_roots() {
        let user_data = root.join("User Data");
        let sidebar = [root.join(SIDEBAR_FILE), user_data.join(SIDEBAR_FILE)]
            .into_iter()
            .find(|path| path.is_file());
        if let (true, Some(sidebar)) = (user_data.is_dir(), sidebar) {
            return Ok(ArcLocation { user_data, sidebar });
        }
    }
    bail!("Arc's data folder was not found. Is Arc installed for this Windows user?")
}

pub fn read_sidebar(arc: &ArcLocation) -> Result<Sidebar> {
    let size = fs::metadata(&arc.sidebar)?.len();
    if size > SIDEBAR_MAX_BYTES {
        bail!("Arc's sidebar file is unexpectedly large");
    }
    let json = fs::read_to_string(&arc.sidebar)
        .with_context(|| format!("could not read {}", arc.sidebar.display()))?;
    arc_sidebar::parse(&json).map_err(|e| anyhow!("could not read Arc's spaces: {e}"))
}

/// Profile directory to the name Arc shows for it (from `Local State`).
pub fn profile_names(user_data: &Path) -> HashMap<String, String> {
    let Ok(text) = fs::read_to_string(user_data.join("Local State")) else {
        return HashMap::new();
    };
    let Ok(state) = serde_json::from_str::<Value>(&text) else {
        return HashMap::new();
    };
    state["profile"]["info_cache"]
        .as_object()
        .map(|cache| {
            cache
                .iter()
                .filter_map(|(dir, info)| {
                    Some((dir.clone(), info["name"].as_str()?.trim().to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// What poof calls an imported profile. Chromium's generic "Person 1" is not a useful name.
pub fn display_name(names: &HashMap<String, String>, dir: &str) -> String {
    let named = names
        .get(dir)
        .map(String::as_str)
        .filter(|n| !n.is_empty() && !n.starts_with("Person "));
    match named {
        Some(name) => name.to_string(),
        None if dir == DEFAULT_PROFILE_DIR => "Arc".into(),
        None => format!("Arc {dir}"),
    }
}

/// Profile directories that have a saved-passwords file.
pub fn profiles_with_logins(user_data: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(user_data) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.path().join("Login Data").is_file())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| safe_dir_name(name))
        .collect()
}
