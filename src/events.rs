//! Messages that wake the event loop from webview callback threads.

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
    /// A newer installer was downloaded and verified; it runs when the browser closes.
    UpdateReady(std::path::PathBuf),
    CloseWindow,
}
