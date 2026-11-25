//! Native new-window hooks: which requests become poof popups, and closing a popup when its
//! page calls `window.close()`.

use super::*;
use crate::events::PopupRequest;
use tao::window::WindowId;

/// Closes the popup when its page calls `window.close()`.
#[cfg(windows)]
pub(super) fn watch_close(webview: &WebView, proxy: &EventLoopProxy<UserEvent>, window: WindowId) {
    use webview2_com::WindowCloseRequestedEventHandler;
    let proxy = proxy.clone();
    unsafe {
        let Ok(core) = webview.controller().CoreWebView2() else {
            return;
        };
        let handler = WindowCloseRequestedEventHandler::create(Box::new(move |_, _| {
            let _ = proxy.send_event(UserEvent::PopupClose { window });
            Ok(())
        }));
        let mut token = 0;
        if let Err(error) = core.add_WindowCloseRequested(&handler, &mut token) {
            log::warn!("registering popup close handler failed: {error}");
        }
    }
}

/// Whether a new-window request should open as a poof popup. Pages opened by a tab also use
/// one for sign-in and blank windows; a popup only opens further popups the page sized itself.
#[cfg(windows)]
fn wants_popup(url: &str, has_geometry: bool, from_tab: bool) -> bool {
    if !from_tab {
        return has_geometry;
    }
    let is_blank = url == "about:blank";
    // Ctrl+click / middle click opens a background tab, which the tab's own handler does.
    if !has_geometry && !is_blank && background_click_modifier() {
        return false;
    }
    has_geometry || is_blank || super::webview::is_auth_url(url)
}

/// Hooks `NewWindowRequested` on the UI thread. Matching requests are deferred and handed to
/// the main loop, which builds the window and attaches it (`open_popup_window`).
#[cfg(windows)]
pub(super) fn watch_popups(wv: &WebView, proxy: &EventLoopProxy<UserEvent>, from_tab: bool) {
    use crate::events::ComSend;
    use webview2_com::NewWindowRequestedEventHandler;
    use windows::core::{BOOL, PWSTR};
    use windows::Win32::System::Com::CoTaskMemFree;
    let environment = wv.environment();
    let proxy = proxy.clone();
    unsafe {
        let Ok(core) = wv.controller().CoreWebView2() else {
            return;
        };
        let handler = NewWindowRequestedEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else { return Ok(()) };
            let mut uri = PWSTR::null();
            let mut url = String::new();
            if args.Uri(&mut uri).is_ok() && !uri.is_null() {
                url = uri.to_string().unwrap_or_default();
                CoTaskMemFree(Some(uri.0 as *const std::ffi::c_void));
            }
            let (mut has_size, mut has_position): (BOOL, BOOL) = (false.into(), false.into());
            let (mut width, mut height) = (0, 0);
            if let Ok(features) = args.WindowFeatures() {
                let _ = features.HasSize(&mut has_size);
                let _ = features.HasPosition(&mut has_position);
                let _ = features.Width(&mut width);
                let _ = features.Height(&mut height);
            }
            let size = has_size.as_bool().then_some((width as f64, height as f64));
            let has_geometry = has_size.as_bool() || has_position.as_bool();
            if !wants_popup(&url, has_geometry, from_tab) {
                return Ok(());
            }
            let Ok(deferral) = args.GetDeferral() else {
                return Ok(());
            };
            let request = PopupRequest {
                size,
                environment: ComSend(environment.clone()),
                args: ComSend(args.clone()),
                deferral: ComSend(deferral),
            };
            if proxy.send_event(UserEvent::Popup(request)).is_err() {
                log::warn!("popup request dropped: the event loop is gone");
            }
            Ok(())
        }));
        let mut token = 0;
        if let Err(error) = core.add_NewWindowRequested(&handler, &mut token) {
            log::warn!("registering popup handler failed: {error}");
        }
    }
}

#[cfg(not(windows))]
pub(super) fn watch_popups(_: &WebView, _: &EventLoopProxy<UserEvent>, _: bool) {}
