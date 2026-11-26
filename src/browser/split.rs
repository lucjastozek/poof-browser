//! Split view: two tabs side by side or stacked.

use super::*;

impl Browser {
    pub(super) fn split_has(&self, id: &str) -> bool {
        self.split
            .as_ref()
            .map(|s| s.a == id || s.b == id)
            .unwrap_or(false)
    }

    pub(super) fn split_other_id(&self) -> Option<String> {
        let sp = self.split.as_ref()?;
        match self.visible.as_deref() {
            Some(v) if v == sp.b => Some(sp.a.clone()),
            Some(v) if v == sp.a => Some(sp.b.clone()),
            _ => None,
        }
    }

    /// [first, second, divider] in window coordinates.
    pub(super) fn split_rects(&self) -> Option<([f64; 4], [f64; 4], [f64; 4])> {
        let sp = self.split.as_ref()?;
        let [x, y, w, h] = self.content_box();
        let gap = INSET;
        Some(if sp.stacked {
            let h1 = ((h - gap) * sp.ratio).round().max(1.0);
            let h2 = (h - gap - h1).max(1.0);
            ([x, y, w, h1], [x, y + h1 + gap, w, h2], [x, y + h1, w, gap])
        } else {
            let w1 = ((w - gap) * sp.ratio).round().max(1.0);
            let w2 = (w - gap - w1).max(1.0);
            ([x, y, w1, h], [x + w1 + gap, y, w2, h], [x + w1, y, gap, h])
        })
    }

    /// The sidebar card while peeking, relative to a WebView whose origin is (px, py).
    pub(super) fn peek_hole(&self, px: f64, py: f64) -> Option<[f64; 4]> {
        if !(self.peek && self.p.sidebar_collapsed && !self.fullscreen) {
            return None;
        }
        let (_, h) = logical(&self.window);
        Some([
            PEEK_X - px,
            TOPBAR_H - py,
            self.p.sidebar_width,
            h - TOPBAR_H - INSET,
        ])
    }

    /// Everything cut out of a page WebView whose origin is (px, py), so the sidebar's own
    /// cards (the peeking sidebar, the Downloads popover) show above the live page.
    pub(super) fn page_holes(&self, px: f64, py: f64) -> Vec<[f64; 4]> {
        let mut holes: Vec<[f64; 4]> = self.peek_hole(px, py).into_iter().collect();
        if let Some([x, y, w, h]) = self.downloads_hole.filter(|_| !self.fullscreen) {
            holes.push([x - px, y - py, w, h]);
        }
        holes
    }

    pub(super) fn open_split(
        &mut self,
        other: &str,
        base: Option<&str>,
        stacked: bool,
        first: bool,
    ) -> Result<()> {
        // Dropped onto a tab that isn't the visible one: show that tab first.
        if let Some(b) = base {
            if b != other && self.visible.as_deref() != Some(b) {
                self.activate(b)?;
            }
        }
        let Some(cur) = self.visible.clone() else {
            self.toast("Open a page first.");
            return Ok(());
        };
        if cur == other || !self.space_tab_ids().iter().any(|x| x == other) {
            return Ok(());
        }
        self.end_split(); // replaces an existing split; restores the sidebar state first
        if self.devtools_open {
            self.close_devtools();
        }
        let Some(idx) = self.tabs.iter().position(|t| t.id == other) else {
            return Ok(());
        };
        self.ensure_webview(idx)?;
        if let Some(wv) = &self.tabs[idx].webview {
            set_memory_level(wv, false);
        }
        self.tabs[idx].last_active = SystemTime::now();
        let (a, b) = if first {
            (other.to_string(), cur)
        } else {
            (cur, other.to_string())
        };
        self.split = Some(Split {
            a,
            b,
            stacked,
            ratio: 0.5,
            prev_collapsed: self.p.sidebar_collapsed,
        });
        self.tab_drag = false;
        if let Some(sp) = self.split.clone() {
            self.remember_split(&sp);
        }
        self.p.sidebar_collapsed = true;
        self.peek = false;
        self.overlay = false;
        self.visible = Some(other.to_string());
        self.mru.retain(|x| x != other);
        self.mru.insert(0, other.to_string());
        self.active
            .insert(self.p.current_space.clone(), other.to_string());
        self.relayout();
        if let Some(wv) = self.visible_webview() {
            let _ = wv.focus();
        }
        self.sync_scrollbar();
        Ok(())
    }

    /// Leaves split view without relayout: hides the unfocused pane, restores the sidebar.
    pub(super) fn end_split(&mut self) {
        let Some(sp) = self.split.take() else { return };
        self.remember_split(&sp); // keeps ratio + orientation for when you come back
        let vis = self.visible.clone();
        for id in [&sp.a, &sp.b] {
            if vis.as_deref() == Some(id.as_str()) {
                continue;
            }
            if let Some(t) = self.tabs.iter_mut().find(|t| &t.id == id) {
                t.last_active = SystemTime::now();
                if let Some(wv) = t.webview.as_ref() {
                    let _ = wv.set_visible(false);
                    if !is_playing_audio(wv) {
                        set_memory_level(wv, true);
                    }
                }
            }
        }
        self.p.sidebar_collapsed = sp.prev_collapsed;
        self.peek = false;
    }

    pub(super) fn remember_split(&mut self, sp: &Split) {
        self.saved_splits
            .retain(|s| s.a != sp.a && s.b != sp.a && s.a != sp.b && s.b != sp.b);
        self.saved_splits.push(sp.clone());
    }

    /// Re-enters a remembered split, focusing `focus`.
    pub(super) fn restore_split(&mut self, pair: Split, focus: &str) -> Result<()> {
        if self.devtools_open {
            self.close_devtools();
        }
        self.hide_visible();
        for id in [pair.a.clone(), pair.b.clone()] {
            if let Some(idx) = self.tabs.iter().position(|t| t.id == id) {
                self.ensure_webview(idx)?;
                self.tabs[idx].last_active = SystemTime::now();
                if let Some(wv) = &self.tabs[idx].webview {
                    set_memory_level(wv, false);
                }
                self.bg_tabs.remove(&id);
            }
        }
        self.split = Some(Split {
            prev_collapsed: self.p.sidebar_collapsed,
            ..pair
        });
        self.p.sidebar_collapsed = true;
        self.peek = false;
        self.overlay = false;
        self.tab_drag = false;
        self.visible = Some(focus.to_string());
        self.mru.retain(|x| x != focus);
        self.mru.insert(0, focus.to_string());
        self.active
            .insert(self.p.current_space.clone(), focus.to_string());
        self.relayout();
        if let Some(wv) = self.visible_webview() {
            let _ = wv.focus();
        }
        self.sync_scrollbar();
        Ok(())
    }

    pub(super) fn close_split(&mut self) {
        let ids = self.split.as_ref().map(|s| (s.a.clone(), s.b.clone()));
        self.end_split();
        if let Some((a, b)) = ids {
            self.saved_splits
                .retain(|s| s.a != a && s.b != a && s.a != b && s.b != b);
        }
        self.relayout();
    }

    pub(super) fn focus_split_pane(&mut self, id: &str) -> Result<()> {
        self.overlay = false;
        self.visible = Some(id.to_string());
        self.mru.retain(|x| x != id);
        self.mru.insert(0, id.to_string());
        self.active
            .insert(self.p.current_space.clone(), id.to_string());
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            t.last_active = SystemTime::now();
        }
        self.relayout();
        if let Some(wv) = self.visible_webview() {
            let _ = wv.focus();
        }
        self.sync_scrollbar();
        Ok(())
    }

    pub(super) fn toggle_split(&mut self) -> Result<()> {
        if self.split.is_some() {
            self.close_split();
            return Ok(());
        }
        match self.switcher_ids().get(1).cloned() {
            Some(id) => self.open_split(&id, None, false, false),
            None => {
                self.toast("Open another tab to use split view.");
                Ok(())
            }
        }
    }

    pub(super) fn toggle_split_orientation(&mut self) {
        if let Some(sp) = self.split.as_mut() {
            sp.stacked = !sp.stacked;
            self.relayout();
        }
    }

    pub(super) fn sync_split_chrome(&self, layout: &Layout) {
        let cfg = layout
            .split_handle
            .filter(|_| !self.overlay && !self.fullscreen)
            .map(|h| {
                json!({
                    "handle": h,
                    "stacked": self.split.as_ref().map(|s| s.stacked).unwrap_or(false),
                    "gap": INSET,
                    "box": self.content_box(),
                })
            });
        if cfg.is_none() && !self.split_chrome_shown.get() {
            return;
        }
        self.split_chrome_shown.set(cfg.is_some());
        let arg = cfg.map(|c| c.to_string()).unwrap_or_else(|| "null".into());
        let _ = self
            .sidebar
            .evaluate_script(&format!("({SPLIT_CHROME_JS})({arg})"));
    }
}
