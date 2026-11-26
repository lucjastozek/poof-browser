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
    /// Everything read from Arc (or why it failed); applied on the main thread.
    ImportDone(Box<Result<crate::import::ArcImport, String>>),
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
    /// PNG of the page behind a full-area view (`None` when capturing failed).
    PageSnapshot(Option<Vec<u8>>),
    /// A file download reported by WebView2.
    Download(DownloadEvent),
    /// Another launch handed over a link or file (the address is UNTRUSTED), or just asked
    /// the running browser to come to the front.
    External {
        url: Option<String>,
    },
    CloseWindow,
    /// A page asked for a popup window (sign-in, `window.open`); the main thread builds it.
    Popup(PopupRequest),
    /// The title of a popup window's page changed.
    /// A button or drag on a popup window's title bar (UNTRUSTED text: matched exactly).
    PopupCmd {
        window: tao::window::WindowId,
        cmd: String,
    },
    /// The popup's page called `window.close()` (for example after sign-in finished).
    PopupClose {
        window: tao::window::WindowId,
    },
    PopupTitle {
        window: tao::window::WindowId,
        title: String,
    },
}

/// Wraps a WebView2 COM object so it can cross to the main thread. WebView2 objects are agile
/// and are only used by the thread that receives them.
#[cfg(windows)]
pub struct ComSend<T>(pub T);
#[cfg(windows)]
unsafe impl<T> Send for ComSend<T> {}

/// Asks the main thread for a popup window. The page's `NewWindowRequested` event stays
/// deferred until the window exists and is attached (all on the UI thread).
pub struct PopupRequest {
    /// Content size the page asked for, in logical pixels.
    pub size: Option<(f64, f64)>,
    #[cfg(windows)]
    pub environment:
        ComSend<webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Environment>,
    #[cfg(windows)]
    pub args: ComSend<
        webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2NewWindowRequestedEventArgs,
    >,
    #[cfg(windows)]
    pub deferral: ComSend<webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Deferral>,
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
