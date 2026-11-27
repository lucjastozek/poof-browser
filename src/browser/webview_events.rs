//! WebView2 event hooks that report page state to the main loop. Windows only; elsewhere the
//! page-side script covers what it can. (Downloads are hooked in `download_hooks.rs`.)

use super::*;

/// Hooks everything a tab's WebView reports natively: shortcuts, address, favicon, fullscreen,
/// focus and crashes. `tab` is the tab id baked into every message.
#[cfg(windows)]
pub(super) fn attach_native_handlers(wv: &WebView, proxy: &EventLoopProxy<UserEvent>, tab: &str) {
    register_browser_shortcuts(wv, proxy.clone(), tab.to_string());
    watch_source_changes(wv, proxy, tab);
    watch_favicons(wv, proxy, tab);
    watch_fullscreen(wv, proxy, tab);
    watch_focus(wv, proxy, tab);
    watch_crashes(wv, proxy, tab);
    watch_downloads(wv, proxy, tab);
    popup_watch::watch_popups(wv, proxy, true);
    context_menu::watch_context_menu(wv, proxy, tab);
    show_link_status(wv);
}

#[cfg(not(windows))]
pub(super) fn attach_native_handlers(
    _wv: &WebView,
    _proxy: &EventLoopProxy<UserEvent>,
    _tab: &str,
) {
}

/// Shows the real destination of a hovered link in the lower-left corner of the page.
///
/// wry switches WebView2's own status bubble off, so it is switched back on here. It is drawn by
/// the browser process, not the page: a site can neither fake nor hide where a link really goes,
/// which is what makes it useful against phishing. Popups call this too, as they have no URL bar.
#[cfg(windows)]
pub(super) fn show_link_status(wv: &WebView) {
    let enabled = unsafe {
        wv.controller()
            .CoreWebView2()
            .and_then(|core| core.Settings())
            .and_then(|settings| settings.SetIsStatusBarEnabled(true))
    };
    if let Err(error) = enabled {
        log::warn!("showing the link status bubble failed: {error}");
    }
}

#[cfg(not(windows))]
pub(super) fn show_link_status(_wv: &WebView) {}

/// Reports every top-level URL change (link clicks, redirects, history.pushState) from WebView2 itself,
/// so the address bar does not depend on the page-side script.
#[cfg(windows)]
fn watch_source_changes(wv: &WebView, proxy: &EventLoopProxy<UserEvent>, tab: &str) {
    use webview2_com::SourceChangedEventHandler;
    use windows::core::PWSTR;
    use windows::Win32::System::Com::CoTaskMemFree;

    let source_proxy = proxy.clone();
    let source_tab = tab.to_string();
    unsafe {
        match wv.controller().CoreWebView2() {
            Ok(core) => {
                let handler = SourceChangedEventHandler::create(Box::new(move |sender, _| {
                    let Some(sender) = sender else { return Ok(()) };
                    let mut uri = PWSTR::null();
                    if sender.Source(&mut uri).is_err() || uri.is_null() {
                        return Ok(());
                    }
                    let value = uri.to_string().ok();
                    CoTaskMemFree(Some(uri.0 as *const std::ffi::c_void));
                    if let Some(value) = value {
                        let body = json!({ "t": "meta", "u": value }).to_string();
                        let _ = source_proxy.send_event(UserEvent::Page {
                            tab: source_tab.clone(),
                            body,
                        });
                    }
                    Ok(())
                }));
                let mut token = 0;
                if let Err(error) = core.add_SourceChanged(&handler, &mut token) {
                    log::warn!("registering WebView2 source handler failed: {error}");
                }
            }
            Err(error) => {
                log::warn!("getting WebView2 core for source handler failed: {error}")
            }
        }
    }
}

/// Reports the page's original favicon URI, so the sidebar can render SVGs and multi-resolution
/// ICO/PNG sources without rasterizing every icon to a small PNG snapshot first.
#[cfg(windows)]
fn watch_favicons(wv: &WebView, proxy: &EventLoopProxy<UserEvent>, tab: &str) {
    use webview2_com::{FaviconChangedEventHandler, Microsoft::Web::WebView2::Win32::*};
    use windows::core::{Interface, PWSTR};
    use windows::Win32::System::Com::CoTaskMemFree;

    fn current_favicon_uri(core15: &ICoreWebView2_15) -> Option<String> {
        unsafe {
            let mut uri = PWSTR::null();
            core15.FaviconUri(&mut uri).ok()?;
            if uri.is_null() {
                return None;
            }
            let value = uri.to_string().ok();
            CoTaskMemFree(Some(uri.0 as *const std::ffi::c_void));
            value.filter(|value| ok_url(value, 4096, false))
        }
    }

    let favicon_proxy = proxy.clone();
    let favicon_tab = tab.to_string();
    let initial_proxy = favicon_proxy.clone();
    let initial_tab = favicon_tab.clone();
    unsafe {
        if let Ok(core) = wv.controller().CoreWebView2() {
            if let Ok(core15) = core.cast::<ICoreWebView2_15>() {
                let handler = FaviconChangedEventHandler::create(Box::new(move |sender, _| {
                    let Some(sender) = sender else { return Ok(()) };
                    let Ok(core15) = sender.cast::<ICoreWebView2_15>() else {
                        return Ok(());
                    };
                    let favicon = current_favicon_uri(&core15);
                    let body = json!({"t": "favicon", "f": favicon}).to_string();
                    let _ = favicon_proxy.send_event(UserEvent::Page {
                        tab: favicon_tab.clone(),
                        body,
                    });
                    Ok(())
                }));
                let mut token = 0;
                if let Err(error) = core15.add_FaviconChanged(&handler, &mut token) {
                    log::warn!("registering WebView2 favicon handler failed: {error}");
                }
                // The initial navigation may have started before the handler was attached.
                let favicon = current_favicon_uri(&core15);
                let body = json!({"t": "favicon", "f": favicon}).to_string();
                let _ = initial_proxy.send_event(UserEvent::Page {
                    tab: initial_tab,
                    body,
                });
            } else {
                log::warn!("WebView2 runtime lacks native favicon support (ICoreWebView2_15)");
            }
        }
    }
}

/// Reports fullscreen requests from the native host, so cross-origin and nested frames work
/// without JavaScript IPC or postMessage relays.
#[cfg(windows)]
fn watch_fullscreen(wv: &WebView, proxy: &EventLoopProxy<UserEvent>, tab: &str) {
    use webview2_com::ContainsFullScreenElementChangedEventHandler;
    use windows::core::BOOL;

    let fullscreen_proxy = proxy.clone();
    let fullscreen_tab = tab.to_string();
    unsafe {
        match wv.controller().CoreWebView2() {
            Ok(core) => {
                let handler = ContainsFullScreenElementChangedEventHandler::create(Box::new(
                    move |sender, _| {
                        let Some(sender) = sender else { return Ok(()) };
                        let mut contains = BOOL::default();
                        if let Err(error) = sender.ContainsFullScreenElement(&mut contains) {
                            log::warn!("reading WebView2 fullscreen state failed: {error}");
                            return Ok(());
                        }
                        let _ = fullscreen_proxy.send_event(UserEvent::Fullscreen {
                            tab: fullscreen_tab.clone(),
                            active: contains.as_bool(),
                        });
                        Ok(())
                    },
                ));
                let mut token = 0;
                if let Err(error) = core.add_ContainsFullScreenElementChanged(&handler, &mut token)
                {
                    log::warn!("registering WebView2 fullscreen handler failed: {error}");
                }
            }
            Err(error) => {
                log::warn!("getting WebView2 core for fullscreen handler failed: {error}")
            }
        }
    }
}

/// Reports when the page's WebView takes keyboard focus (used to track the active split pane).
#[cfg(windows)]
fn watch_focus(wv: &WebView, proxy: &EventLoopProxy<UserEvent>, tab: &str) {
    use webview2_com::FocusChangedEventHandler;
    let focus_proxy = proxy.clone();
    let focus_tab = tab.to_string();
    unsafe {
        let handler = FocusChangedEventHandler::create(Box::new(move |_, _| {
            let _ = focus_proxy.send_event(UserEvent::Page {
                tab: focus_tab.clone(),
                body: r#"{"t":"focus"}"#.to_string(),
            });
            Ok(())
        }));
        let mut token = 0;
        if let Err(error) = wv.controller().add_GotFocus(&handler, &mut token) {
            log::warn!("registering WebView2 focus handler failed: {error}");
        }
    }
}

/// Crash recovery: a dead renderer (or browser process) is reported to the main loop, which
/// recreates the WebView instead of leaving a blank or frozen tab.
#[cfg(windows)]
fn watch_crashes(wv: &WebView, proxy: &EventLoopProxy<UserEvent>, tab: &str) {
    use webview2_com::{Microsoft::Web::WebView2::Win32::*, ProcessFailedEventHandler};

    let crash_proxy = proxy.clone();
    let crash_tab = tab.to_string();
    unsafe {
        match wv.controller().CoreWebView2() {
            Ok(core) => {
                let handler = ProcessFailedEventHandler::create(Box::new(move |_sender, args| {
                    let Some(args) = args else { return Ok(()) };
                    let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED;
                    args.ProcessFailedKind(&mut kind)?;
                    let browser = kind == COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED;
                    let render = kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED;
                    if browser || render {
                        let _ = crash_proxy.send_event(UserEvent::TabCrashed {
                            tab: crash_tab.clone(),
                            browser,
                        });
                    }
                    Ok(())
                }));
                let mut token = 0;
                if let Err(error) = core.add_ProcessFailed(&handler, &mut token) {
                    log::warn!("registering WebView2 crash handler failed: {error}");
                }
            }
            Err(error) => {
                log::warn!("getting WebView2 core for crash handler failed: {error}")
            }
        }
    }
}
