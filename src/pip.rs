//! Auto Picture-in-Picture: when you leave a tab that is playing video, pop it out.
//!
//! `requestPictureInPicture()` needs a user gesture. We can't get one from the page, so on
//! Windows we call the DevTools protocol `Runtime.evaluate` with `userGesture: true`.
use wry::WebView;

const ENTER_JS: &str = r#"(async () => {
  if (document.pictureInPictureElement) return;
  const vs = [...document.querySelectorAll('video')].filter(v =>
    !v.paused && !v.ended && v.readyState > 2 && v.videoWidth > 0 && !v.disablePictureInPicture);
  if (!vs.length) return;
  vs.sort((a, b) => b.videoWidth * b.videoHeight - a.videoWidth * a.videoHeight);
  try { await vs[0].requestPictureInPicture(); } catch (e) {}
})()"#;

const EXIT_JS: &str =
    "if (document.pictureInPictureElement) { document.exitPictureInPicture().catch(()=>{}); }";

/// Called right before a tab is hidden.
pub fn on_leave(wv: &WebView) {
    eval_with_gesture(wv, ENTER_JS);
}

/// Called when a tab becomes visible again: bring the video back into the page.
pub fn on_enter(wv: &WebView) {
    let _ = wv.evaluate_script(EXIT_JS);
}

#[cfg(windows)]
fn eval_with_gesture(wv: &WebView, js: &str) {
    use webview2_com::CallDevToolsProtocolMethodCompletedHandler;
    use windows::core::HSTRING;
    use wry::WebViewExtWindows;

    let params = serde_json::json!({
        "expression": js,
        "userGesture": true,
        "awaitPromise": false
    })
    .to_string();

    unsafe {
        let Ok(core) = wv.controller().CoreWebView2() else { return };
        let handler = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(|_, _| Ok(())));
        if let Err(e) = core.CallDevToolsProtocolMethod(
            &HSTRING::from("Runtime.evaluate"),
            &HSTRING::from(params),
            &handler,
        ) {
            log::warn!("auto-pip CDP call failed: {e}");
        }
    }
}

#[cfg(not(windows))]
fn eval_with_gesture(wv: &WebView, js: &str) {
    // TODO(linux/macos): no gesture-injection path yet; this will usually be rejected by the
    // engine. A CEF backend can use the same CDP trick.
    let _ = wv.evaluate_script(js);
}
