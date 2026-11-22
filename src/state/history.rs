//! The browsing history file (history.json), kept apart from state.json so frequent writes stay small.

use super::storage::write_json_atomically;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

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
    fs::File::open(path)
        .ok()
        .and_then(|f| serde_json::from_reader(std::io::BufReader::new(f)).ok())
        .unwrap_or_default()
}

pub fn save_history(entries: &[HistoryEntry], path: &Path) -> bool {
    write_json_atomically(entries, path, false)
}
