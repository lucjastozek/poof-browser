//! Extra entries in the page's native right-click menu. Windows only; WebView2 draws the menu
//! itself, so entries are added to it from the `ContextMenuRequested` event.

use super::*;

/// Longest image address accepted for "Open image in new tab" (same bound as every tab URL).
#[cfg(windows)]
const IMAGE_URL_MAX: usize = 4096;

/// Adds "Open image in new tab" to the menu WebView2 shows for an image.
///
/// The image opens in a background tab next to the current one, like Ctrl+click on a link. It
/// goes through the same internal `open_background` message as that case, which re-checks the
/// address, so a page cannot open anything but `http(s)` this way.
#[cfg(windows)]
pub(super) fn watch_context_menu(wv: &WebView, proxy: &EventLoopProxy<UserEvent>) {
    use webview2_com::{ContextMenuRequestedEventHandler, Microsoft::Web::WebView2::Win32::*};
    use windows::core::Interface;

    let environment = wv.environment();
    let proxy = proxy.clone();
    unsafe {
        let Ok(core) = wv.controller().CoreWebView2() else {
            return;
        };
        let Ok(core11) = core.cast::<ICoreWebView2_11>() else {
            log::warn!("WebView2 runtime lacks context menu support (ICoreWebView2_11)");
            return;
        };
        let handler = ContextMenuRequestedEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else { return Ok(()) };
            log::debug!("context menu requested");
            if let Some(url) = image_url(&args) {
                if let Err(error) = add_open_image_item(&environment, &args, &proxy, url) {
                    log::warn!("adding the image menu entry failed: {error}");
                }
            }
            Ok(())
        }));
        let mut token = 0;
        match core11.add_ContextMenuRequested(&handler, &mut token) {
            Ok(()) => log::debug!("context menu handler registered"),
            Err(error) => log::warn!("registering WebView2 context menu handler failed: {error}"),
        }
    }
}

/// The address of the image under the pointer, if the menu is for an image with a web address.
#[cfg(windows)]
fn image_url(
    args: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2ContextMenuRequestedEventArgs,
) -> Option<String> {
    use webview2_com::Microsoft::Web::WebView2::Win32::*;
    use windows::core::PWSTR;
    use windows::Win32::System::Com::CoTaskMemFree;

    unsafe {
        let target = match args.ContextMenuTarget() {
            Ok(target) => target,
            Err(error) => {
                return None;
            }
        };
        let mut kind = COREWEBVIEW2_CONTEXT_MENU_TARGET_KIND_PAGE;
        if let Err(error) = target.Kind(&mut kind) {
            return None;
        }
        if kind != COREWEBVIEW2_CONTEXT_MENU_TARGET_KIND_IMAGE {
            return None;
        }
        let mut uri = PWSTR::null();
        if let Err(error) = target.SourceUri(&mut uri) {
            return None;
        }
        if uri.is_null() {
            return None;
        }
        let value = uri.to_string().ok();
        CoTaskMemFree(Some(uri.0 as *const std::ffi::c_void));
        let scheme = value.as_deref().and_then(|v| v.split(':').next()).unwrap_or("");
        value.filter(|value| ok_url(value, IMAGE_URL_MAX, false))
    }
}

/// Puts the entry (and a separator under it) at the top of the menu and wires its click.
#[cfg(windows)]
unsafe fn add_open_image_item(
    environment: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Environment,
    args: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2ContextMenuRequestedEventArgs,
    proxy: &EventLoopProxy<UserEvent>,
    url: String,
) -> windows::core::Result<()> {
    use webview2_com::{CustomItemSelectedEventHandler, Microsoft::Web::WebView2::Win32::*};
    use windows::core::{w, Interface};
    use windows::Win32::System::Com::IStream;

    let environment9 = environment.cast::<ICoreWebView2Environment9>()?;
    let items = args.MenuItems()?;
    let entry = environment9.CreateContextMenuItem(
        w!("Open image in new tab"),
        None::<&IStream>,
        COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_COMMAND,
    )?;
    log::debug!("created the image menu entry");

    let proxy = proxy.clone();
    let handler = CustomItemSelectedEventHandler::create(Box::new(move |_, _| {
        let body = json!({ "t": "open_background", "u": url }).to_string();
        let _ = proxy.send_event(UserEvent::Page {
            tab: String::new(),
            body,
        });
        Ok(())
    }));
    let mut token = 0;
    entry.add_CustomItemSelected(&handler, &mut token)?;

    items.InsertValueAtIndex(0, &entry)?;
    // The separator is cosmetic: the entry must still show if it cannot be made.
    let separator = environment9.CreateContextMenuItem(
        w!(""),
        None::<&IStream>,
        COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SEPARATOR,
    );
    match separator {
        Ok(separator) => {
            if let Err(error) = items.InsertValueAtIndex(1, &separator) {
                log::warn!("inserting the menu separator failed: {error}");
            }
        }
        Err(error) => log::warn!("creating the menu separator failed: {error}"),
    }
    Ok(())
}