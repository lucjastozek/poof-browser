//! Window theme and the page scrollbar.

use super::*;

impl Browser {
    /// Colors the native window frame; the UI itself follows `theme` from the pushed state.
    pub(super) fn apply_theme(&self) {
        let t = match self.p.theme.as_str() {
            "light" => Some(Theme::Light),
            "dark" => Some(Theme::Dark),
            _ => None,
        };
        self.window.set_theme(t);
    }

    pub(super) fn toggle_scrollbar(&mut self) {
        self.scrollbar_visible = !self.scrollbar_visible;
        let js = format!(
            "window.__poofSbVisible&&window.__poofSbVisible({})",
            self.scrollbar_visible
        );
        for t in &self.tabs {
            if let Some(wv) = t.webview.as_ref() {
                let _ = wv.evaluate_script(&js);
            }
        }
    }

    /// Pushes the current space color into the visible page's scrollbar style.
    pub(super) fn sync_scrollbar(&self) {
        let c = &self.cur_space().color;
        if ok_color(c) {
            self.eval_visible(&format!("window.__poofSb&&window.__poofSb('{c}')"));
        }
    }
}
