//! Creating the WebView of a tab.

use super::*;

impl Browser {
    /// Creates the WebView of a tab on first use (tabs start without one, to save memory).
    pub(super) fn ensure_webview(&mut self, idx: usize) -> Result<()> {
        if self.tabs[idx].webview.is_some() {
            return Ok(());
        }
        let id = self.tabs[idx].id.clone();
        let profile_id = self.tabs[idx].profile_id.clone();
        let url = self.tabs[idx].url.clone();
        let bounds = self.content_rect();
        let accent = self.cur_space().color.clone();
        let dir = self.root.join("profiles").join(&profile_id);
        let sb_on = self.scrollbar_visible;
        let ctx = self
            .contexts
            .entry(profile_id.clone())
            .or_insert_with(|| WebContext::new(Some(dir)));

        let (p1, p2, p3) = (self.proxy.clone(), self.proxy.clone(), self.proxy.clone());
        let (id1, id2) = (id.clone(), id.clone());

        let builder = WebViewBuilder::new_with_web_context(ctx)
            .with_bounds(bounds)
            .with_visible(false)
            .with_devtools(false)
            // Pinch (touchpad / touch screen) and Ctrl+wheel zoom; pages such as Figma still
            // receive the wheel events first and can handle them themselves.
            .with_hotkeys_zoom(true)
            .with_url(&url)
            .with_initialization_script_for_main_only(INIT_JS, false)
            .with_initialization_script_for_main_only(scrollbar_script(&accent, sb_on), false)
            .with_initialization_script_for_main_only(pip::TRACK_JS, false)
            .with_initialization_script_for_main_only(LIVE_JS, false)
            .with_initialization_script_for_main_only(FULLSCREEN_JS, false)
            .with_initialization_script_for_main_only(PASSWORDS_JS, true)
            .with_ipc_handler(move |req| {
                let _ = p1.send_event(UserEvent::Page {
                    tab: id1.clone(),
                    body: req.into_body(),
                });
            })
            .with_document_title_changed_handler(move |title| {
                let _ = p2.send_event(UserEvent::Title {
                    tab: id2.clone(),
                    title,
                });
            })
            // target=_blank opens a new tab; popups and sign-in windows open as a poof popup window.
            .with_new_window_req_handler(move |url, features| {
                let is_popup = features.size.is_some() || features.position.is_some();
                match new_window_decision(url, is_popup, &p3) {
                    // Popups and sign-in windows open as a poof window, created by
                    // `popup::watch_popups`; the default popup stays only where that can't run.
                    NewWindowResponse::Allow if cfg!(windows) => NewWindowResponse::Deny,
                    other => other,
                }
            });

        #[cfg(windows)]
        let builder = {
            use wry::WebViewBuilderExtWindows;
            builder
                .with_browser_extensions_enabled(true)
                // Enables a random loopback CDP port for this isolated profile; the port is
                // read from DevToolsActivePort.
                .with_additional_browser_args(WEBVIEW2_BROWSER_ARGS)
        };

        let wv = builder.build_as_child(&self.window)?;

        attach_native_handlers(&wv, &self.proxy, &id);

        if self.ext_done.insert(profile_id) {
            extensions::load_all(&wv, &self.root.join("extensions"));
        }
        self.tabs[idx].webview = Some(wv);
        Ok(())
    }
}

/// Decides what to do with a `target=_blank` / `window.open` request from a page.
/// `Allow` means "open as a popup window".
fn new_window_decision(
    url: String,
    is_popup: bool,
    proxy: &EventLoopProxy<UserEvent>,
) -> NewWindowResponse {
    let is_blank = url == "about:blank";
    // Ctrl+click / middle click: open behind the current tab instead of switching to it.
    let in_background = !is_popup && !is_blank && background_click_modifier();
    if in_background {
        let body = json!({ "t": "open_background", "u": url }).to_string();
        let _ = proxy.send_event(UserEvent::Page {
            tab: String::new(),
            body,
        });
        NewWindowResponse::Deny
    } else if is_popup || is_blank || is_auth_url(&url) {
        NewWindowResponse::Allow
    } else {
        let _ = proxy.send_event(UserEvent::OpenTab { url });
        NewWindowResponse::Deny
    }
}

/// Sign-in pages must stay in a popup, or the login flow cannot hand its result back.
pub(super) fn is_auth_url(url: &str) -> bool {
    url.starts_with("https://accounts.google.com/")
        || url.starts_with("https://appleid.apple.com/")
        || url.starts_with("https://login.microsoftonline.com/")
        || url.starts_with("https://github.com/login/")
        || url.contains("/oauth")
        || url.contains("/authorize")
        || url.contains("/sso")
        || url.contains("/signin")
        || url.contains("/login")
}
