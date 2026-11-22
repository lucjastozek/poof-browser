//! Auto Picture-in-Picture: when you leave a tab that is playing video, pop it out.
//!
//! `requestPictureInPicture()` needs a user gesture. We can't get one from the page, so on
//! Windows `gesture::eval_with_gesture` supplies one through the DevTools protocol.
use crate::gesture::eval_with_gesture;
use wry::WebView;

/// Injected into every page (main frame). Remembers which videos the user started themselves,
/// so autoplaying stories, feed previews and ads are never popped out.
pub const TRACK_JS: &str = include_str!("track.js");

/// Pops the largest user-started, playing video out into a Picture-in-Picture window.
const ENTER_JS: &str = include_str!("enter.js");

/// Brings a popped-out video back into the page.
const EXIT_JS: &str = include_str!("exit.js");

/// Called right before a tab is hidden.
pub fn on_leave(wv: &WebView) {
    eval_with_gesture(wv, ENTER_JS);
}

/// Called when a tab becomes visible again: bring the video back into the page.
pub fn on_enter(wv: &WebView) {
    let _ = wv.evaluate_script(EXIT_JS);
}
