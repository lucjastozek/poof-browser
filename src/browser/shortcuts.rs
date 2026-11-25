//! Keyboard shortcuts that must work while a page owns keyboard focus.

use super::*;

/// Maps WebView2 accelerator keys (Win32 VK_* values) to browser commands.
pub(super) fn browser_shortcut(
    key: u32,
    ctrl: bool,
    alt: bool,
    shift: bool,
) -> Option<&'static str> {
    const VK_TAB: u32 = 0x09;
    const VK_LEFT: u32 = 0x25;
    const VK_RIGHT: u32 = 0x27;

    if key == 0x7B && !ctrl && !alt && !shift {
        return Some("devtools_toggle");
    }
    if ctrl && alt && !shift {
        return match key {
            VK_LEFT => Some("space_prev"),
            VK_RIGHT => Some("space_next"),
            _ => None,
        };
    }
    if alt && !ctrl && !shift {
        return match key {
            VK_LEFT => Some("back"),
            VK_RIGHT => Some("forward"),
            _ => None,
        };
    }
    if !ctrl || alt {
        return None;
    }
    if shift {
        return match key {
            0x49 => Some("devtools_toggle"), // I
            0x54 => Some("reopen"),          // T
            0x52 => Some("reload"),          // R
            0x45 => Some("split_toggle"),    // E
            0x4F => Some("split_orient"),    // O
            VK_TAB => Some("tab_prev"),
            _ => None,
        };
    }
    match key {
        0x54 => Some("new_tab"),   // T
        0x57 => Some("close"),     // W
        0x4C => Some("focus_url"), // L
        0x52 => Some("reload"),    // R
        0x50 => Some("print"),     // P
        0x44 => Some("favorite"),  // D
        0x48 => Some("history"),   // H
        0x4A => Some("downloads"), // J
        0x31 => Some("tab_1"),
        0x32 => Some("tab_2"),
        0x33 => Some("tab_3"),
        0x34 => Some("tab_4"),
        0x35 => Some("tab_5"),
        0x36 => Some("tab_6"),
        0x37 => Some("tab_7"),
        0x38 => Some("tab_8"),
        0x39 => Some("tab_last"),
        0x53 => Some("toggle_scrollbar"), // S
        VK_TAB => Some("tab_next"),
        _ => None,
    }
}

#[cfg(windows)]
pub(super) fn register_browser_shortcuts(
    webview: &WebView,
    proxy: EventLoopProxy<UserEvent>,
    tab: String,
) {
    use webview2_com::{AcceleratorKeyPressedEventHandler, Microsoft::Web::WebView2::Win32::*};
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;

    unsafe {
        let handler = AcceleratorKeyPressedEventHandler::create(Box::new(move |_sender, args| {
            let Some(args) = args else { return Ok(()) };
            let mut kind = COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN;
            args.KeyEventKind(&mut kind)?;
            if kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN
                && kind != COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN
            {
                return Ok(());
            }

            let mut key_event_lparam = 0;
            args.KeyEventLParam(&mut key_event_lparam)?;
            let mut virtual_key = 0;
            args.VirtualKey(&mut virtual_key)?;
            let is_down = |virtual_key| GetAsyncKeyState(virtual_key) < 0;
            let command = browser_shortcut(
                virtual_key,
                is_down(0x11), // VK_CONTROL
                is_down(0x12), // VK_MENU
                is_down(0x10), // VK_SHIFT
            );

            if let Some(command) = command {
                args.SetHandled(true)?;
                // Ignore auto-repeat after suppressing WebView2's default action.
                if key_event_lparam & (1 << 30) != 0 {
                    return Ok(());
                }
                let _ = proxy.send_event(UserEvent::Page {
                    tab: tab.clone(),
                    body: json!({ "t": "key", "k": command }).to_string(),
                });
            }
            Ok(())
        }));
        let mut token = 0;
        if let Err(error) = webview
            .controller()
            .add_AcceleratorKeyPressed(&handler, &mut token)
        {
            log::warn!("registering WebView2 keyboard shortcuts failed: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_j_opens_downloads_and_plain_j_does_not() {
        assert_eq!(
            browser_shortcut(0x4A, true, false, false),
            Some("downloads")
        );
        assert_eq!(browser_shortcut(0x4A, false, false, false), None);
        assert_eq!(browser_shortcut(0x4A, true, false, true), None);
    }
}
