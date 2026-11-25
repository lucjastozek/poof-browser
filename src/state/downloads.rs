//! The download list (downloads.json), kept apart from state.json so progress never rewrites it.

use super::storage::write_json_atomically;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    Active,
    /// Paused by the user or interrupted in a way that can be resumed.
    Paused,
    Completed,
    Cancelled,
    Failed,
}

impl DownloadStatus {
    /// Still has a live WebView2 operation behind it.
    pub fn is_live(self) -> bool {
        matches!(self, Self::Active | Self::Paused)
    }
}

/// One download. Stored in downloads.json; the fields marked `serde(skip)` only exist while
/// the browser runs.
#[derive(Serialize, Deserialize, Clone)]
pub struct DownloadEntry {
    pub id: String,
    pub url: String,
    /// Where the file is (or will be) saved.
    pub path: String,
    pub name: String,
    /// Size in bytes, 0 while the server has not said.
    #[serde(default)]
    pub total: u64,
    #[serde(default)]
    pub received: u64,
    pub status: DownloadStatus,
    /// Unix seconds.
    pub started: u64,
    /// The tab the download came from (its WebView must stay alive while this runs).
    #[serde(skip)]
    pub tab: String,
    #[serde(skip)]
    pub profile_id: String,
    /// Smoothed transfer speed in bytes per second.
    #[serde(skip)]
    pub speed: f64,
    #[serde(skip)]
    pub sample: Option<(Instant, u64)>,
    /// The finished file is no longer where it was saved.
    #[serde(skip)]
    pub missing: bool,
}

/// Weight of the newest speed sample; lower is smoother.
const SPEED_SMOOTHING: f64 = 0.3;
/// Samples closer together than this (seconds) are skipped: too noisy to give a speed.
const SPEED_MIN_WINDOW: f64 = 0.2;

impl DownloadEntry {
    pub fn new(id: String, url: String, path: &Path, total: u64, started: u64) -> Self {
        Self {
            id,
            url,
            path: path.to_string_lossy().into_owned(),
            name: file_name(path),
            total,
            received: 0,
            status: DownloadStatus::Active,
            started,
            tab: String::new(),
            profile_id: String::new(),
            speed: 0.0,
            sample: None,
            missing: false,
        }
    }

    /// Records the byte counts and refreshes the smoothed speed.
    pub fn record_progress(&mut self, received: u64, total: u64, now: Instant) {
        self.received = received;
        if total > 0 {
            self.total = total;
        }
        let Some((then, before)) = self.sample else {
            self.sample = Some((now, received));
            return;
        };
        let seconds = now.saturating_duration_since(then).as_secs_f64();
        if seconds < SPEED_MIN_WINDOW {
            return;
        }
        let instant = received.saturating_sub(before) as f64 / seconds;
        self.speed = if self.speed > 0.0 {
            self.speed * (1.0 - SPEED_SMOOTHING) + instant * SPEED_SMOOTHING
        } else {
            instant
        };
        self.sample = Some((now, received));
    }

    /// Forgets the speed history (after a pause, the first sample would be meaningless).
    pub fn reset_speed(&mut self) {
        self.speed = 0.0;
        self.sample = None;
    }
}

/// The last component of `path` as text.
pub fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "download".into())
}

/// `path`, or `name (1).ext`, `name (2).ext`, ... when it exists or another download is
/// about to use it, so a download never overwrites a file.
pub fn unique_path(path: &Path, taken: &HashSet<PathBuf>) -> PathBuf {
    let free = |p: &Path| !p.exists() && !taken.contains(p);
    if free(path) {
        return path.to_path_buf();
    }
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "download".into());
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let dir = path.parent().unwrap_or_else(|| Path::new(""));
    (1..10_000)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|candidate| free(candidate))
        .unwrap_or_else(|| path.to_path_buf())
}

/// Downloads that were still running when the browser last closed cannot continue: they are
/// kept in the list as failed so the user can see what was lost.
pub fn settle_interrupted(entries: &mut [DownloadEntry]) {
    for entry in entries.iter_mut().filter(|e| e.status.is_live()) {
        entry.status = DownloadStatus::Failed;
    }
}

pub fn load_downloads(path: &Path) -> Vec<DownloadEntry> {
    let mut entries: Vec<DownloadEntry> = fs::File::open(path)
        .ok()
        .and_then(|f| serde_json::from_reader(std::io::BufReader::new(f)).ok())
        .unwrap_or_default();
    settle_interrupted(&mut entries);
    entries
}

pub fn save_downloads(entries: &[DownloadEntry], path: &Path) -> bool {
    write_json_atomically(entries, path, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn entry() -> DownloadEntry {
        DownloadEntry::new(
            "a".into(),
            "https://x.test/f.zip".into(),
            Path::new("C:/d/f.zip"),
            0,
            1,
        )
    }

    #[test]
    fn name_comes_from_the_path() {
        assert_eq!(entry().name, "f.zip");
        assert_eq!(file_name(Path::new("/")), "download");
    }

    #[test]
    fn speed_is_smoothed_and_needs_two_samples() {
        let mut e = entry();
        let t0 = Instant::now();
        e.record_progress(0, 1000, t0);
        assert_eq!(e.speed, 0.0);
        e.record_progress(500, 1000, t0 + Duration::from_secs(1));
        assert_eq!(e.speed, 500.0);
        e.record_progress(1000, 1000, t0 + Duration::from_secs(2));
        assert!((e.speed - (500.0 * 0.7 + 500.0 * 0.3)).abs() < 1e-9);
        assert_eq!(e.total, 1000);
    }

    #[test]
    fn samples_that_are_too_close_are_skipped() {
        let mut e = entry();
        let t0 = Instant::now();
        e.record_progress(0, 0, t0);
        e.record_progress(100, 0, t0 + Duration::from_millis(50));
        assert_eq!(e.speed, 0.0);
        assert_eq!(e.received, 100);
        e.reset_speed();
        assert!(e.sample.is_none());
    }

    #[test]
    fn unique_path_skips_existing_and_reserved_names() {
        let dir = std::env::temp_dir().join(format!("poof-dl-{}", crate::state::new_id()));
        fs::create_dir_all(&dir).unwrap();
        let wanted = dir.join("a.txt");
        assert_eq!(unique_path(&wanted, &HashSet::new()), wanted);

        fs::write(&wanted, b"x").unwrap();
        let mut taken = HashSet::new();
        taken.insert(dir.join("a (1).txt"));
        assert_eq!(unique_path(&wanted, &taken), dir.join("a (2).txt"));

        let bare = dir.join("noext");
        fs::write(&bare, b"x").unwrap();
        assert_eq!(unique_path(&bare, &HashSet::new()), dir.join("noext (1)"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn running_downloads_do_not_survive_a_restart() {
        let mut list = vec![entry(), entry()];
        list[1].status = DownloadStatus::Completed;
        settle_interrupted(&mut list);
        assert_eq!(list[0].status, DownloadStatus::Failed);
        assert_eq!(list[1].status, DownloadStatus::Completed);
    }

    #[test]
    fn entries_round_trip_without_runtime_fields() {
        let mut e = entry();
        e.tab = "t".into();
        e.speed = 5.0;
        let json = serde_json::to_string(&e).unwrap();
        assert!(!json.contains("speed") && !json.contains("\"tab\""));
        let back: DownloadEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.speed, 0.0);
        assert_eq!(back.status, DownloadStatus::Active);
    }
}
