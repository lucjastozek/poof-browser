//! Layout metrics, limits and defaults.

use super::*;

pub(super) const SIDEBAR_MIN_W: f64 = 180.0;
pub(super) const SIDEBAR_MAX_W: f64 = 480.0;
pub(super) const SIDEBAR_COMPACT_W: f64 = 56.0;
pub(super) const INSET: f64 = 8.0;
pub(super) const TOPBAR_H: f64 = 40.0;
pub(super) const PANEL_MIN: f64 = 240.0;
pub(super) const PANEL_MAX: f64 = 800.0;
pub(super) const PANEL_DEFAULT: f64 = 420.0;
/// The gap between page and panel holds the resize handle.
pub(super) const PANEL_GAP: f64 = INSET;
pub(super) const PAGE_MIN: f64 = 160.0;
/// Strip above the DevTools frontend holding the title and the close button.
pub(super) const HEADER_H: f64 = 28.0;
/// Largest favicon (URL or data: URI) kept in memory, pushed to the UI and persisted.
pub(super) const FAVICON_MAX: usize = 48_000;
/// Favicons larger than this are kept in memory but not written to state.json.
pub(super) const FAVICON_PERSIST_MAX: usize = 16_000;
/// State writes closer together than this are coalesced and flushed by the event-loop timer.
pub(super) const SAVE_MIN_INTERVAL: Duration = Duration::from_secs(2);
/// Title-driven sidebar updates closer together than this are coalesced.
pub(super) const STATE_MIN_INTERVAL: Duration = Duration::from_millis(150);
/// History entries kept at most, across all profiles (oldest dropped first).
pub(super) const HISTORY_MAX: usize = 5000;
/// History rows sent to the history view at once.
pub(super) const HISTORY_SHOWN: usize = 500;
/// Most icons / largest single icon accepted from the sidebar for the DevTools frontend.
pub(super) const DEVTOOLS_ICONS_MAX: usize = 120;
pub(super) const DEVTOOLS_ICON_BYTES: usize = 12_000;
/// Rendered size (px) of every icon in the DevTools frontend; tune to match the sidebar icons.
pub(super) const DEVTOOLS_ICON_PX: f64 = 14.0;
/// Downloads kept in the list; the oldest finished ones are dropped first.
pub(super) const DOWNLOADS_MAX: usize = 200;
/// WebView2 reports received bytes many times a second; progress is passed on at most this often.
pub(super) const DOWNLOAD_PROGRESS_INTERVAL: Duration = Duration::from_millis(250);
/// Longest link accepted from another launch of the browser (see `crate::instance`).
pub(super) const EXTERNAL_URL_MAX: usize = 4096;
/// Popular sites shown in the new-tab bar until the user's own history takes over.
pub(super) const DEFAULT_SITES: &[(&str, &str)] = &[
    ("YouTube", "https://www.youtube.com/"),
    ("Google", "https://www.google.com/"),
    ("Gmail", "https://mail.google.com/"),
    ("GitHub", "https://github.com/"),
    ("Reddit", "https://www.reddit.com/"),
    ("Wikipedia", "https://www.wikipedia.org/"),
    ("Netflix", "https://www.netflix.com/"),
    ("Spotify", "https://open.spotify.com/"),
    ("Twitch", "https://www.twitch.tv/"),
    ("Instagram", "https://www.instagram.com/"),
    ("Chrome Web Store", "https://chromewebstore.google.com/"),
];
pub(super) const PEEK_X: f64 = INSET; // left edge of the peek card; lines up with the page edge
                                      // Extra disabled features (hints/model downloads, media router, translate) trim background
                                      // work and memory. AudioServiceOutOfProcess keeps audio in the browser process instead of a
                                      // separate utility process, so it belongs to poof's own process tree: apps that capture
                                      // "this app's audio" (Discord, OBS) then find it. --in-process-gpu drops one GPU process per
                                      // profile. There must be exactly one --disable-features switch (the last one wins), and these
                                      // arguments replace wry's defaults, so the sidebar uses the same list.
#[cfg(windows)]
pub(super) const WEBVIEW2_BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,OptimizationHints,OptimizationGuideModelDownloading,MediaRouter,Translate,AudioServiceOutOfProcess --in-process-gpu --disable-blink-features=AutomationControlled --remote-debugging-address=127.0.0.1 --remote-debugging-port=0 --remote-allow-origins=devtools://devtools";
/// The sidebar's own WebView2 environment: same process-saving switches, no debugging port.
#[cfg(windows)]
pub(super) const WEBVIEW2_UI_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,OptimizationHints,OptimizationGuideModelDownloading,MediaRouter,Translate,AudioServiceOutOfProcess --in-process-gpu";
