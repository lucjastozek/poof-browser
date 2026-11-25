//! Messages that wake the event loop from webview callback threads.

use crate::state::DownloadStatus;
use std::path::PathBuf;

/// Everything that can wake the event loop from a webview callback thread.
pub enum UserEvent {
    /// Raw JSON command from the (trusted) sidebar UI.
    Ui(String),
    /// Raw JSON from a content page. UNTRUSTED: validated in browser/page_messages.rs.
    Page {
        tab: String,
        body: String,
    },
    /// Native WebView2 notification that a page element entered or left fullscreen.
    Fullscreen {
        tab: String,
        active: bool,
    },
    Title {
        tab: String,
        title: String,
    },
    /// A page-specific DevTools frontend URL resolved from the profile's CDP endpoint.
    DevToolsReady {
        tab: String,
        url: String,
    },
    DevToolsFailed {
        tab: String,
        message: String,
    },
    OpenTab {
        url: String,
    },
    /// Result of a background extension install (Web Store download).
    ExtResult {
        ok: bool,
        msg: String,
        dir: Option<std::path::PathBuf>,
        id: Option<String>,
    },
    /// A WebView2 render process (or the whole browser process of a profile) died.
    TabCrashed {
        tab: String,
        browser: bool,
    },
    /// A newer release exists: ask the user.
    UpdateFound(crate::updater::Update),
    /// The installer was downloaded and verified.
    UpdateReady(std::path::PathBuf),
    UpdateFailed,
    /// A file download reported by WebView2.
    Download(DownloadEvent),
    /// Another launch handed over a link or file (the address is UNTRUSTED), or just asked
    /// the running browser to come to the front.
    External {
        url: Option<String>,
    },
    CloseWindow,
}

/// What WebView2 reports about one download. `id` is chosen by the native hook.
pub enum DownloadEvent {
    Started {
        id: String,
        tab: String,
        url: String,
        path: PathBuf,
        total: u64,
    },
    /// Throttled byte counts of a running download.
    Progress {
        id: String,
        received: u64,
        total: u64,
    },
    /// The download paused, resumed, finished, failed or was cancelled.
    State {
        id: String,
        status: DownloadStatus,
        /// The final file, known once the download completed.
        path: Option<PathBuf>,
        received: u64,
        total: u64,
    },
}
