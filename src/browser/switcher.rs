//! The Ctrl+Tab tab switcher.

use super::*;

impl Browser {
    pub(super) fn switcher_ids(&self) -> Vec<String> {
        let reach = self.space_tab_ids();
        let mut ids: Vec<String> = self
            .mru
            .iter()
            .filter(|id| reach.contains(id))
            .cloned()
            .collect();
        for id in reach {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        // The tab you are on is always first, so index 1 is "the previous tab".
        if let Some(v) = self.visible.clone() {
            if let Some(p) = ids.iter().position(|x| *x == v) {
                let id = ids.remove(p);
                ids.insert(0, id);
            }
        }
        ids
    }

    pub(super) fn switcher_step(&mut self, dir: i32) -> Result<()> {
        if let Some(sw) = self.switcher.as_mut() {
            let n = sw.ids.len() as i32;
            sw.index = (sw.index as i32 + dir).rem_euclid(n) as usize;
            let i = sw.index;
            let _ = self.sidebar.evaluate_script(&format!(
                "window.__switcherSelect&&window.__switcherSelect({i})"
            ));
            return Ok(());
        }
        let ids = self.switcher_ids();
        if ids.len() < 2 {
            return Ok(());
        }
        let index = if dir > 0 { 1 } else { ids.len() - 1 };
        let cards: Vec<Value> = ids
            .iter()
            .filter_map(|id| self.tabs.iter().find(|t| &t.id == id))
            .map(|t| {
                json!({
                    "id": t.id, "title": t.title, "url": t.url, "favicon": t.favicon,
                    "loaded": t.webview.is_some(), "pinned": t.kind != Kind::Normal
                })
            })
            .collect();
        let payload = json!({ "cards": cards, "index": index }).to_string();
        self.switcher = Some(Switcher {
            ids,
            index,
            prev_overlay: self.overlay,
        });
        // Hides the native page WebView so the sidebar's overlay is visible above it.
        self.overlay = true;
        self.relayout();
        let _ = self.sidebar.focus();
        let _ = self
            .sidebar
            .evaluate_script(&format!("window.__switcher&&window.__switcher({payload})"));
        Ok(())
    }

    pub(super) fn switcher_close(&mut self, commit: bool) -> Result<()> {
        let Some(sw) = self.switcher.take() else {
            return Ok(());
        };
        let _ = self
            .sidebar
            .evaluate_script("window.__switcherClose&&window.__switcherClose()");
        self.overlay = sw.prev_overlay;
        let target = if commit {
            sw.ids.get(sw.index).cloned()
        } else {
            None
        };
        match target {
            Some(id) if self.visible.as_deref() != Some(id.as_str()) => self.activate(&id)?,
            _ => {
                self.relayout();
                if !self.overlay {
                    if let Some(wv) = self.visible_webview() {
                        let _ = wv.focus();
                    }
                }
            }
        }
        Ok(())
    }
}
