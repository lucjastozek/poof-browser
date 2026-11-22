//! HTML fullscreen: the page takes the whole window and the sidebar steps aside.

use super::*;

impl Browser {
    /// Switches the whole native window, not just the embedded WebView, into fullscreen.
    pub(super) fn handle_fullscreen(&mut self, tab: &str, active: bool) {
        if self.visible.as_deref() != Some(tab) || self.fullscreen == active {
            return;
        }
        self.fullscreen = active;
        if active {
            self.was_maximized = self.window.is_maximized();
            if self.was_maximized {
                self.window.set_maximized(false);
            }
            self.window
                .set_fullscreen(Some(tao::window::Fullscreen::Borderless(None)));
        } else {
            self.window.set_fullscreen(None);
            if self.was_maximized {
                self.window.set_maximized(true);
                self.was_maximized = false;
            }
        }
        self.relayout();
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            for ms in [30u64, 120, 300, 700] {
                std::thread::sleep(Duration::from_millis(ms));
                let _ = proxy.send_event(UserEvent::Page {
                    tab: String::new(),
                    body: r#"{"t":"relayout"}"#.to_string(),
                });
            }
        });
    }

    /// Fullscreens the active page through its own player, or leaves fullscreen. Works from any
    /// focus (sidebar, address bar) because it acts on the page, not on the key's target.
    pub(super) fn toggle_page_fullscreen(&self) {
        if let Some(wv) = self.visible_webview() {
            // The page must own keyboard focus: its player ignores keys otherwise, and Esc
            // would go to the sidebar instead of leaving fullscreen.
            let _ = wv.focus();
            crate::gesture::eval_with_gesture(
                wv,
                "window.__poofToggleFullscreen && window.__poofToggleFullscreen()",
            );
        }
    }

    pub(super) fn leave_fullscreen(&mut self) {
        if self.fullscreen {
            if let Some(wv) = self.visible_webview() {
                let _ = wv.evaluate_script(
                    "if (document.fullscreenElement) document.exitFullscreen().catch(()=>{});",
                );
            }
            self.fullscreen = false;
            self.window.set_fullscreen(None);
            if self.was_maximized {
                self.window.set_maximized(true);
                self.was_maximized = false;
            }
            self.sync_fullscreen_backdrop();
        }
    }

    /// While a page is fullscreen the sidebar WebView stays in place but turns solid black with
    /// its UI hidden, so any frame the page hasn't painted yet shows black, not the bare window.
    pub(super) fn sync_fullscreen_backdrop(&self) {
        if self.fs_css.get() == self.fullscreen {
            return;
        }
        self.fs_css.set(self.fullscreen);
        let js = format!(
            r#"(()=>{{let s=document.getElementById("__poof_fs");if(!s){{s=document.createElement("style");s.id="__poof_fs";s.textContent="html.poof-fs,html.poof-fs body{{background:#000!important}}html.poof-fs body>*{{visibility:hidden!important}}";document.head.appendChild(s);}}document.documentElement.classList.toggle("poof-fs",{});}})()"#,
            self.fullscreen
        );
        let _ = self.sidebar.evaluate_script(&js);
    }
}
