//! Small data types shared by the browser modules.

use super::*;

pub struct Tab {
    pub id: String,
    pub profile_id: String,
    pub space_id: Option<String>, // None for favorites
    pub folder_id: Option<String>,
    pub kind: Kind,
    pub home: String, // URL a pinned/favorite resets to when closed
    pub url: String,
    pub host: String, // last host seen (for visit stats)
    pub title: String,
    pub favicon: Option<String>,
    pub webview: Option<WebView>, // None = not loaded yet (lazy) or put to sleep
    pub last_active: SystemTime,
}

/// Where everything goes for the current window / DevTools state.
pub(super) struct Layout {
    pub(super) page: Rect,
    pub(super) page_visible: bool,
    pub(super) tools: Option<Rect>,
    /// [x, y, w, h] of the strip with the DevTools title and close button.
    pub(super) header: Option<[f64; 4]>,
    /// [x, y, w, h] of the resize handle (docked mode only).
    pub(super) handle: Option<[f64; 4]>,
    pub(super) other: Option<Rect>,
    pub(super) split_handle: Option<[f64; 4]>,
}

pub(super) struct Switcher {
    pub(super) ids: Vec<String>,
    pub(super) index: usize,
    /// A sidebar view (new tab bar etc.) was already open when the switcher started.
    pub(super) prev_overlay: bool,
}

#[derive(Clone)]
pub(super) struct Split {
    pub(super) a: String,            // first pane (left / top)
    pub(super) b: String,            // second pane (right / bottom)
    pub(super) stacked: bool,        // true = top/bottom
    pub(super) ratio: f64,           // share of the first pane
    pub(super) prev_collapsed: bool, // sidebar state to restore when leaving split view
}

impl Tab {
    /// A saved tab, restored without a WebView (it is created when the tab is first shown).
    pub(super) fn from_saved(saved: &Saved) -> Self {
        Self {
            id: saved.id.clone(),
            profile_id: saved.profile_id.clone(),
            space_id: saved.space_id.clone(),
            folder_id: saved.folder_id.clone(),
            kind: saved.kind,
            home: saved.url.clone(),
            url: saved.url.clone(),
            host: String::new(),
            title: saved.title.clone(),
            favicon: saved.favicon.clone(),
            webview: None,
            last_active: SystemTime::now(),
        }
    }
}
