//! Runs page JavaScript as if the user had just clicked, for APIs that require a user gesture
//! (Picture-in-Picture, fullscreen). On Windows this goes through the DevTools protocol.
use wry::WebView;

#[cfg(windows)]
pub fn eval_with_gesture(wv: &WebView, js: &str) {
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
        let Ok(core) = wv.controller().CoreWebView2() else {
            return;
        };
        let handler = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(|_, _| Ok(())));
        if let Err(e) = core.CallDevToolsProtocolMethod(
            &HSTRING::from("Runtime.evaluate"),
            &HSTRING::from(params),
            &handler,
        ) {
            log::warn!("gesture CDP call failed: {e}");
        }
    }
}

#[cfg(not(windows))]
pub fn eval_with_gesture(wv: &WebView, js: &str) {
    // No gesture-injection path yet; the engine will usually reject gesture-only APIs.
    let _ = wv.evaluate_script(js);
}
