//! JavaScript injected into pages and into the DevTools frontend. The sources live in `js/`.

use super::*;

/// Builds a call of a script file that holds one function expression. Formatters such as Prettier
/// rewrite such a file to end in `;`, which would be a syntax error inside the parentheses.
pub(super) fn call_script(function: &str, args: impl std::fmt::Display) -> String {
    let function = function.trim_end().trim_end_matches(';');
    format!("({function})({args})")
}

/// Runs in every page (main frame) before the page scripts.
/// Reports live capture / calls so such tabs are never put to sleep.
pub(super) const LIVE_JS: &str = include_str!("js/live.js");
pub(super) const SLEEP_GUARD_JS: &str = include_str!("js/sleep-guard.js");

/// The sleep guard, with the snapshot to restore (if any) filled in.
pub(super) fn sleep_guard_script(snap: Option<&SleepSnap>) -> String {
    let json = snap
        .map(|s| json!({ "u": s.url, "y": s.y, "f": s.fields }).to_string())
        .unwrap_or_else(|| "null".into());
    SLEEP_GUARD_JS.replace("__SNAP__", &json)
}

/// Tabs on the local machine (dev servers) are never auto-slept.
pub(super) fn is_dev_url(url: &str) -> bool {
    let rest = url.split("://").nth(1).unwrap_or("");
    let auth = rest.split(['/', '?', '#']).next().unwrap_or("");
    let auth = auth.rsplit('@').next().unwrap_or("");
    let host = if let Some(v6) = auth.strip_prefix('[') {
        v6.split(']').next().unwrap_or("")
    } else {
        auth.split(':').next().unwrap_or("")
    }
    .to_ascii_lowercase();
    host == "localhost" || host.ends_with(".localhost") || host == "127.0.0.1" || host == "::1"
}
pub(super) const BOOST_JS: &str = include_str!("js/boost.js");
pub(super) const MEDIA_JS: &str = include_str!("js/media.js");
pub(super) const PEEK_JS: &str = include_str!("js/peek.js");
pub(super) const INIT_JS: &str = include_str!("js/init.js");
/// The page right-click menu, filled in by `context_menu.rs`.
pub(super) const CONTEXT_MENU_JS: &str = include_str!("js/context-menu.js");

/// Two-finger swipe over the page goes back / forward (main frame only).
pub(super) const SWIPE_NAV_JS: &str = include_str!("js/swipe-nav.js");

/// Fullscreen through the site's own player; also exposes `__poofToggleFullscreen`.
pub(super) const FULLSCREEN_JS: &str = include_str!("js/fullscreen.js");

/// Reports login forms and submitted logins to the password manager (top frame only).
pub(super) const PASSWORDS_JS: &str = include_str!("js/passwords.js");

const SCROLLBAR_JS: &str = include_str!("js/scrollbar.js");
const PDF_THEME_JS: &str = include_str!("js/pdf-theme.js");

/// Colours the built-in PDF viewer with the space's accent.
pub(super) fn pdf_theme_script(accent: &str) -> String {
    let accent = if ok_color(accent) { accent } else { "#6dc5e2" };
    PDF_THEME_JS.replace("__ACCENT__", accent)
}

pub(super) fn scrollbar_script(accent: &str, visible: bool) -> String {
    let accent = if ok_color(accent) { accent } else { "#6dc5e2" };
    SCROLLBAR_JS
        .replace("__ACCENT__", accent)
        .replace("__VISIBLE__", if visible { "true" } else { "false" })
}

/// Draws the DevTools resize handle and header strip. They live in the sidebar WebView, whose IPC
/// is known to work, so neither depends on the DevTools frontend. Called with a config object,
/// or null to hide both. The first call also sends the browser's own icon set to the native side,
/// which forwards it to the DevTools frontend (see `DEVTOOLS_SYNC_JS`).
pub(super) const DEVTOOLS_CHROME_JS: &str = include_str!("js/devtools-chrome.js");

/// Evaluated in the DevTools frontend by the native side every few milliseconds while DevTools is
/// open. The frontend has no IPC, so the result comes back through the evaluate callback.
///
/// The frontend must be docked: undocked, device mode opens in a separate toolbox window that
/// this WebView never shows, and the inspector cannot sit next to the emulated screen. Docked:
///  * panel mode: the main split is hidden, so the inspector fills its panel-sized WebView
///  * device mode: the main split is shown next to the inspector and reports where the emulated
///    screen is, so native code can put the real page WebView exactly there
///
/// Nothing is touched before the frontend has presented its UI: importing its modules earlier
/// would evaluate them out of order and break the frontend.
pub(super) const DEVTOOLS_SYNC_JS: &str = include_str!("js/devtools-sync.js");

/// Toolbar and drag handles around the page in device mode (runs in the sidebar).
pub(super) const DEVICE_CHROME_JS: &str = include_str!("js/device-chrome.js");
pub(super) const SPLIT_CHROME_JS: &str = include_str!("js/split-chrome.js");
