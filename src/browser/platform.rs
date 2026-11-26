//! Thin wrappers over WebView2 / Win32. On other platforms they are no-ops.

use super::*;

#[cfg(windows)]
pub(super) fn apply_webview_bounds(wv: &WebView, bounds: Rect, scale: f64) -> Result<()> {
    use windows::Win32::{
        Foundation::{HWND, RECT},
        UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER},
    };

    wv.set_bounds(bounds)?;
    let position = bounds.position.to_physical(scale);
    let size = bounds.size.to_physical::<i32>(scale);
    let mut hwnd = HWND::default();
    unsafe {
        wv.controller().ParentWindow(&mut hwnd)?;
        if hwnd.0.is_null() {
            anyhow::bail!("WebView2 child window handle is null");
        }
        SetWindowPos(
            hwnd,
            None,
            position.x,
            position.y,
            size.width,
            size.height,
            SWP_NOACTIVATE | SWP_NOZORDER,
        )?;
        // Set the controller's own bounds synchronously too, so the renderer's viewport
        // follows the window right away instead of keeping its previous size.
        wv.controller().SetBounds(RECT {
            left: 0,
            top: 0,
            right: size.width,
            bottom: size.height,
        })?;
        wv.controller().NotifyParentWindowPositionChanged()?;
    }
    Ok(())
}

#[cfg(not(windows))]
pub(super) fn apply_webview_bounds(wv: &WebView, bounds: Rect, _scale: f64) -> Result<()> {
    wv.set_bounds(bounds)?;
    Ok(())
}

/// In device mode the page WebView overlaps the DevTools WebView and must stay on top.
#[cfg(windows)]
pub(super) fn raise_webview(wv: &WebView) {
    use windows::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{
            SetWindowPos, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE,
        },
    };

    let mut hwnd = HWND::default();
    unsafe {
        if wv.controller().ParentWindow(&mut hwnd).is_ok() && !hwnd.0.is_null() {
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOP),
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOOWNERZORDER | SWP_NOSIZE,
            );
        }
    }
}

#[cfg(windows)]
pub(super) fn apply_webview_radius(
    wv: &WebView,
    width: f64,
    height: f64,
    scale: f64,
    rounded: bool,
    holes: &[[f64; 4]], // [x, y, w, h] boxes cut out of the page, relative to the WebView
) {
    use windows::Win32::{
        Foundation::HWND,
        Graphics::Gdi::{
            CombineRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject, SetWindowRgn, HGDIOBJ,
            RGN_DIFF,
        },
    };

    let mut hwnd = HWND::default();
    unsafe {
        if wv.controller().ParentWindow(&mut hwnd).is_err() || hwnd.0.is_null() {
            return;
        }
        if !rounded && holes.is_empty() {
            let _ = SetWindowRgn(hwnd, None, true);
            return;
        }
        let w = (width * scale).round().max(1.0) as i32;
        let h = (height * scale).round().max(1.0) as i32;
        let rad = (8.0 * scale).round().max(1.0) as i32;
        let region = if rounded {
            CreateRoundRectRgn(0, 0, w, h, rad * 2, rad * 2)
        } else {
            CreateRectRgn(0, 0, w, h)
        };
        if region.0.is_null() {
            return;
        }
        for &[hx, hy, hw, hh] in holes {
            let l = (hx * scale).round() as i32;
            let t = (hy * scale).round() as i32;
            let r = ((hx + hw) * scale).round() as i32;
            let b = ((hy + hh) * scale).round() as i32;
            if r > 0 && l < w {
                let cut = CreateRoundRectRgn(l, t, r, b, rad * 2, rad * 2);
                if !cut.0.is_null() {
                    let _ = CombineRgn(Some(region), Some(region), Some(cut), RGN_DIFF);
                    let _ = DeleteObject(HGDIOBJ(cut.0));
                }
            }
        }
        if SetWindowRgn(hwnd, Some(region), true) == 0 {
            let _ = DeleteObject(HGDIOBJ(region.0));
        }
    }
}

pub(super) fn rect_parts(r: Rect, scale: f64) -> (f64, f64, f64, f64) {
    let p = r.position.to_logical::<f64>(scale);
    let s = r.size.to_logical::<f64>(scale);
    (p.x, p.y, s.width, s.height)
}

/// Asks WebView2 to trim (LOW) or restore (NORMAL) the memory it keeps for this page. Hidden
/// tabs get LOW so the engine drops caches and compresses memory right away; best effort, and
/// silently ignored on runtimes that predate the API.
#[cfg(windows)]
pub(super) fn set_memory_level(wv: &WebView, low: bool) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
    };
    use windows::core::Interface;

    unsafe {
        let Ok(core) = wv.controller().CoreWebView2() else {
            return;
        };
        let Ok(core19) = core.cast::<ICoreWebView2_19>() else {
            return;
        };
        let level = if low {
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
        } else {
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
        };
        let _ = core19.SetMemoryUsageTargetLevel(level);
    }
}

#[cfg(not(windows))]
pub(super) fn set_memory_level(_wv: &WebView, _low: bool) {}

/// True while the page is producing sound; such tabs are never put to sleep.
#[cfg(windows)]
pub(super) fn is_playing_audio(wv: &WebView) -> bool {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_8;
    use windows::core::{Interface, BOOL};

    unsafe {
        let Ok(core) = wv.controller().CoreWebView2() else {
            return false;
        };
        let Ok(core8) = core.cast::<ICoreWebView2_8>() else {
            return false;
        };
        let mut playing = BOOL::default();
        core8.IsDocumentPlayingAudio(&mut playing).is_ok() && playing.as_bool()
    }
}

#[cfg(not(windows))]
pub(super) fn is_playing_audio(_wv: &WebView) -> bool {
    false
}

/// Ctrl or the middle mouse button is held: a link opened now belongs in a background tab.
#[cfg(windows)]
pub(super) fn background_click_modifier() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    unsafe { GetAsyncKeyState(0x11) < 0 || GetAsyncKeyState(0x04) < 0 } // VK_CONTROL, VK_MBUTTON
}

#[cfg(not(windows))]
pub(super) fn background_click_modifier() -> bool {
    false
}

#[cfg(windows)]
pub(super) fn ctrl_held() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    unsafe { GetAsyncKeyState(0x11) < 0 } // VK_CONTROL
}

#[cfg(not(windows))]
pub(super) fn ctrl_held() -> bool {
    true // there, only the sidebar's keyup (Ctrl released) closes the switcher
}

/// Runs a helper program without flashing a console window. Failures are only logged.
fn spawn_quietly(program: &str, args: &[&std::ffi::OsStr]) {
    let mut command = std::process::Command::new(program);
    command.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    match command.spawn() {
        // Reaped on a short-lived thread so no zombie process is left behind.
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(error) => log::warn!("could not start {program}: {error}"),
    }
}

/// Opens a file with the program the system associates with it.
pub(super) fn open_with_default_app(path: &Path) {
    #[cfg(windows)]
    spawn_quietly(
        "rundll32.exe",
        &["url.dll,FileProtocolHandler".as_ref(), path.as_os_str()],
    );
    #[cfg(target_os = "macos")]
    spawn_quietly("open", &[path.as_os_str()]);
    #[cfg(all(not(windows), not(target_os = "macos")))]
    spawn_quietly("xdg-open", &[path.as_os_str()]);
}

/// Shows a file selected in the system file manager (its folder, where it cannot select).
pub(super) fn reveal_in_file_manager(path: &Path) {
    #[cfg(windows)]
    spawn_quietly("explorer.exe", &["/select,".as_ref(), path.as_os_str()]);
    #[cfg(target_os = "macos")]
    spawn_quietly("open", &["-R".as_ref(), path.as_os_str()]);
    #[cfg(all(not(windows), not(target_os = "macos")))]
    spawn_quietly("xdg-open", &[path.parent().unwrap_or(path).as_os_str()]);
}
