//! Downloads: the list behind the Downloads view, progress bookkeeping and file actions.
//!
//! The native hooks live in `download_hooks.rs`; this file turns their events into list
//! changes and pushes the list to the sidebar.

use super::*;
use crate::events::DownloadEvent;

/// A WebView of a closed tab that is kept alive until its downloads end, because a profile
/// whose last WebView is gone loses its WebView2 environment (and the download with it).
pub(super) struct Lingering {
    pub(super) tab: String,
    pub(super) profile_id: String,
    _webview: WebView,
}

/// What follows the pointer while a download is dragged out (a generic document card).
#[cfg(any(windows, target_os = "macos"))]
const DRAG_PREVIEW: &[u8] = include_bytes!("drag-file.png");

impl Browser {
    pub(super) fn on_download_event(&mut self, event: DownloadEvent) {
        match event {
            DownloadEvent::Started {
                id,
                tab,
                url,
                path,
                total,
            } => self.download_started(id, &tab, url, &path, total),
            DownloadEvent::Progress {
                id,
                received,
                total,
            } => self.download_progress(&id, received, total),
            DownloadEvent::State {
                id,
                status,
                path,
                received,
                total,
            } => self.download_state(&id, status, path, received, total),
        }
    }

    fn download_started(&mut self, id: String, tab: &str, url: String, path: &Path, total: u64) {
        let mut entry = DownloadEntry::new(id, clip(&url, 2048), path, total, now_secs());
        entry.tab = tab.to_string();
        entry.profile_id = self
            .tabs
            .iter()
            .find(|t| t.id == tab)
            .map(|t| t.profile_id.clone())
            .unwrap_or_default();
        self.toast(&format!("Downloading {}", clip(&entry.name, 60)));
        self.downloads.insert(0, entry);
        self.trim_downloads();
        self.persist_downloads();
        self.push_downloads();
    }

    fn download_progress(&mut self, id: &str, received: u64, total: u64) {
        let Some(entry) = self.downloads.iter_mut().find(|d| d.id == id) else {
            return;
        };
        if entry.status != DownloadStatus::Active {
            return;
        }
        entry.record_progress(received, total, Instant::now());
        self.push_downloads();
    }

    fn download_state(
        &mut self,
        id: &str,
        status: DownloadStatus,
        path: Option<PathBuf>,
        received: u64,
        total: u64,
    ) {
        let Some(entry) = self.downloads.iter_mut().find(|d| d.id == id) else {
            return;
        };
        let was = entry.status;
        entry.status = status;
        entry.reset_speed();
        entry.received = received.max(entry.received);
        if total > 0 {
            entry.total = total;
        }
        if let Some(path) = path {
            entry.path = path.to_string_lossy().into_owned();
            entry.name = file_name(&path);
        }
        if status == DownloadStatus::Completed {
            entry.received = entry.total.max(entry.received);
            entry.total = entry.received;
        }
        let name = clip(&entry.name, 60);
        if was != status {
            match status {
                DownloadStatus::Completed => self.toast(&format!("Downloaded {name}")),
                DownloadStatus::Failed => self.toast(&format!("Download failed: {name}")),
                _ => {}
            }
        }
        if !status.is_live() {
            self.release_lingering();
        }
        self.persist_downloads();
        self.push_downloads();
    }

    /// Keeps the list bounded: the oldest finished downloads go first.
    pub(super) fn trim_downloads(&mut self) {
        while self.downloads.len() > DOWNLOADS_MAX {
            let Some(oldest) = self.downloads.iter().rposition(|d| !d.status.is_live()) else {
                return;
            };
            self.downloads.remove(oldest);
        }
    }

    pub(super) fn persist_downloads(&self) {
        if !save_downloads(&self.downloads, &self.root.join("downloads.json")) {
            log::warn!("saving downloads.json failed");
        }
    }

    /// Sends the whole (small, bounded) list to the Downloads view and the toolbar button.
    pub(super) fn push_downloads(&self) {
        let rows: Vec<Value> = self
            .downloads
            .iter()
            .map(|d| {
                json!({
                    "id": d.id, "name": d.name, "url": d.url, "path": d.path,
                    "status": d.status, "received": d.received, "total": d.total,
                    "speed": d.speed.round(), "ts": d.started, "missing": d.missing,
                })
            })
            .collect();
        let payload = serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into());
        let _ = self.sidebar.evaluate_script(&format!(
            "window.__setDownloads&&window.__setDownloads({payload})"
        ));
    }

    /// The Downloads view was opened: re-check which finished files are still there.
    pub(super) fn list_downloads(&mut self) {
        for entry in self
            .downloads
            .iter_mut()
            .filter(|d| d.status == DownloadStatus::Completed)
        {
            entry.missing = !Path::new(&entry.path).exists();
        }
        self.push_downloads();
    }

    pub(super) fn pause_download(&mut self, id: &str) {
        self.control_live_download(id, DownloadControl::Pause);
    }

    pub(super) fn resume_download(&mut self, id: &str) {
        self.control_live_download(id, DownloadControl::Resume);
    }

    pub(super) fn cancel_download(&mut self, id: &str) {
        if self.control_live_download(id, DownloadControl::Cancel) {
            return;
        }
        // No operation behind it (it is already gone): just settle the entry.
        if let Some(entry) = self
            .downloads
            .iter_mut()
            .find(|d| d.id == id && d.status.is_live())
        {
            entry.status = DownloadStatus::Cancelled;
            self.release_lingering();
            self.persist_downloads();
            self.push_downloads();
        }
    }

    /// Asks WebView2 to act on a live download; the state change comes back as an event.
    fn control_live_download(&mut self, id: &str, action: DownloadControl) -> bool {
        let live = self
            .downloads
            .iter()
            .any(|d| d.id == id && d.status.is_live());
        if !live {
            return false;
        }
        let done = control_download(id, action);
        if !done {
            self.toast("That download can’t be changed right now.");
        }
        done
    }

    pub(super) fn open_download(&mut self, id: &str) {
        let Some(path) = self.finished_download_path(id) else {
            return;
        };
        if path.exists() {
            open_with_default_app(&path);
        } else {
            self.mark_download_missing(id);
        }
    }

    /// Drags a finished download out of the window: dropped on a page it is uploaded, dropped in
    /// another app or a folder it is copied. The sidebar sends this once the pointer has moved
    /// far enough with the button held; the drag then runs until the button is released.
    pub(super) fn drag_download(&mut self, id: &str) {
        let Some(path) = self.finished_download_path(id) else {
            return;
        };
        if !path.is_file() {
            self.mark_download_missing(id);
            return;
        }
        #[cfg(any(windows, target_os = "macos"))]
        {
            let result = drag::start_drag(
                &self.window,
                drag::DragItem::Files(vec![path]),
                drag::Image::Raw(DRAG_PREVIEW.to_vec()),
                |_, _| {},
                drag::Options::default(),
            );
            if let Err(e) = result {
                log::warn!("download drag failed: {e}");
            }
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        log::info!("dragging downloads out is not supported on this platform: {path:?}");
    }

    pub(super) fn show_download(&mut self, id: &str) {
        let Some(path) = self
            .downloads
            .iter()
            .find(|d| d.id == id)
            .map(|d| PathBuf::from(&d.path))
        else {
            return;
        };
        if path.exists() {
            reveal_in_file_manager(&path);
        } else if let Some(folder) = path.parent().filter(|f| f.is_dir()) {
            open_with_default_app(folder);
        } else {
            self.mark_download_missing(id);
        }
    }

    fn finished_download_path(&self, id: &str) -> Option<PathBuf> {
        self.downloads
            .iter()
            .find(|d| d.id == id && d.status == DownloadStatus::Completed)
            .map(|d| PathBuf::from(&d.path))
    }

    fn mark_download_missing(&mut self, id: &str) {
        if let Some(entry) = self.downloads.iter_mut().find(|d| d.id == id) {
            entry.missing = true;
        }
        self.toast("That file was moved or deleted.");
        self.push_downloads();
    }

    /// Removes a download from the list (the file stays); a running one is cancelled first.
    pub(super) fn remove_download(&mut self, id: &str) {
        self.cancel_download(id);
        self.downloads.retain(|d| d.id != id || d.status.is_live());
        self.persist_downloads();
        self.push_downloads();
    }

    /// Empties the list of everything that has finished.
    pub(super) fn clear_downloads(&mut self) {
        self.downloads.retain(|d| d.status.is_live());
        self.persist_downloads();
        self.push_downloads();
    }

    /// Ctrl+J: opens or closes the Downloads view in the sidebar.
    pub(super) fn toggle_downloads_view(&self, tab: &str) {
        if tab.is_empty() || self.visible.as_deref() == Some(tab) {
            let _ = self.sidebar.focus();
            let _ = self
                .sidebar
                .evaluate_script("window.__toggleDownloads && window.__toggleDownloads()");
        }
    }

    /// True while a download started by this tab is still running.
    pub(super) fn tab_has_live_download(&self, tab: &str) -> bool {
        self.downloads
            .iter()
            .any(|d| d.status.is_live() && d.tab == tab)
    }

    /// Tabs whose WebView must not be put to sleep right now.
    pub(super) fn tabs_with_live_downloads(&self) -> HashSet<String> {
        self.downloads
            .iter()
            .filter(|d| d.status.is_live())
            .map(|d| d.tab.clone())
            .collect()
    }

    /// Hands over the WebView of a closed or unloaded tab: dropped, unless one of its
    /// downloads is still running, in which case it is hidden and kept until that ends.
    pub(super) fn retire_webview(&mut self, tab: &str, profile_id: &str, webview: Option<WebView>) {
        let Some(webview) = webview else { return };
        if !self.tab_has_live_download(tab) {
            return;
        }
        let _ = webview.set_visible(false);
        self.lingering.push(Lingering {
            tab: tab.to_string(),
            profile_id: profile_id.to_string(),
            _webview: webview,
        });
    }

    /// Drops the kept WebViews whose downloads have ended.
    pub(super) fn release_lingering(&mut self) {
        let downloads = &self.downloads;
        self.lingering.retain(|l| {
            downloads
                .iter()
                .any(|d| d.status.is_live() && d.tab == l.tab)
        });
        self.prune_ext_done();
    }

    /// A profile without any live WebView lost its WebView2 environment, and its downloads
    /// with it: they are shown as failed instead of running forever.
    pub(super) fn fail_orphaned_downloads(&mut self, live_profiles: &HashSet<String>) {
        let mut changed = false;
        for entry in self.downloads.iter_mut().filter(|d| d.status.is_live()) {
            if !entry.profile_id.is_empty() && !live_profiles.contains(&entry.profile_id) {
                control_download(&entry.id, DownloadControl::Forget);
                entry.status = DownloadStatus::Failed;
                changed = true;
            }
        }
        if changed {
            self.persist_downloads();
            self.push_downloads();
        }
    }

    /// Final write when the browser closes.
    pub(super) fn flush_downloads(&self) {
        self.persist_downloads();
    }
}
