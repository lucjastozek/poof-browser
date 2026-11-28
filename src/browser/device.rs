//! Responsive device mode and the one-click developer layouts.
//!
//! Device mode resizes the page WebView itself, so the page sees a real viewport of that size
//! (media queries, `100vw`, `visualViewport`) and no emulation sits between it and the engine.
//! The sidebar draws the toolbar and drag handles around the page (`js/device-chrome.js`). The
//! size is remembered per site and as the start size for sites without one.

use super::*;

impl Browser {
    /// [x, y, w, h] of the emulated page inside `area`: centred, below the toolbar strip, and
    /// never larger than what is left after the handle gutters.
    pub(super) fn device_frame(&self, area: [f64; 4]) -> Option<[f64; 4]> {
        let [w, h] = self.device?;
        let [x, y, aw, ah] = area;
        let (max_w, max_h) = device_limits(area);
        let (fw, fh) = (w.min(max_w).max(1.0), h.min(max_h).max(1.0));
        Some([(x + (aw - fw) / 2.0).round(), y + DEVICE_BAR_H, fw, fh])
    }

    /// Puts the page into the device frame when device mode is on (not in split view or fullscreen).
    pub(super) fn with_device(&self, mut out: Layout, area: [f64; 4]) -> Layout {
        if self.fullscreen || self.split.is_some() {
            return out;
        }
        if let Some(frame) = self.device_frame(area) {
            out.page = rect_of(frame);
            out.device = Some(DeviceChrome { area, frame });
        }
        out
    }

    pub(super) fn toggle_device_mode(&mut self) {
        if self.device.take().is_some() {
            self.relayout();
            self.focus_page();
            return;
        }
        if self.split.is_some() {
            self.toast("Close split view to use device mode.");
            return;
        }
        if self.responsive_mode {
            self.toast("Turn off DevTools device mode first.");
            return;
        }
        let Some(tab) = self.visible.clone().filter(|_| !self.overlay) else {
            self.toast("Open a page first.");
            return;
        };
        self.device = Some(self.device_start_size(&tab));
        self.relayout();
    }

    /// The site's own saved size, else the one used last, else a current phone.
    fn device_start_size(&self, tab: &str) -> [f64; 2] {
        let site = self
            .tabs
            .iter()
            .find(|t| t.id == tab)
            .and_then(|t| self.p.device_sizes.get(&t.host));
        let saved = site.or(self.p.device_last.as_ref());
        saved.map_or(DEVICE_DEFAULT, |s| [s[0] as f64, s[1] as f64])
    }

    /// Sets the emulated size from the drag handles or a preset. `persist` is true when the
    /// gesture ended: only then the size is written to disk and remembered for the site.
    pub(super) fn set_device_size(&mut self, w: f64, h: f64, persist: bool) {
        if self.device.is_none() || !w.is_finite() || !h.is_finite() {
            return;
        }
        let (max_w, max_h) = self
            .layout()
            .device
            .map_or((DEVICE_MAX, DEVICE_MAX), |d| device_limits(d.area));
        let fit = |v: f64, max: f64| v.round().min(max.min(DEVICE_MAX)).max(DEVICE_MIN.min(max));
        let size = [fit(w, max_w), fit(h, max_h)];
        self.device = Some(size);
        if persist {
            self.remember_device_size(size);
        }
        self.relayout();
    }

    fn remember_device_size(&mut self, size: [f64; 2]) {
        let stored = [size[0] as u32, size[1] as u32];
        self.p.device_last = Some(stored);
        let host = self
            .visible
            .as_ref()
            .and_then(|id| self.tabs.iter().find(|t| &t.id == id))
            .map(|t| t.host.clone())
            .filter(|h| !h.is_empty());
        if let Some(host) = host {
            self.p.device_sizes.insert(host.clone(), stored);
            while self.p.device_sizes.len() > DEVICE_SITES_MAX {
                let Some(old) = self.p.device_sizes.keys().find(|k| **k != host).cloned() else {
                    break;
                };
                self.p.device_sizes.remove(&old);
            }
        }
        self.save();
    }

    /// Uses the saved size of the visible tab's site, if it has one (tab switch, navigation).
    pub(super) fn apply_site_device_size(&mut self) {
        if self.device.is_none() {
            return;
        }
        let saved = self
            .visible
            .as_ref()
            .and_then(|id| self.tabs.iter().find(|t| &t.id == id))
            .and_then(|t| self.p.device_sizes.get(&t.host));
        if let Some(s) = saved {
            self.device = Some([s[0] as f64, s[1] as f64]);
        }
    }

    /// Draws or removes the toolbar and handles around the emulated page.
    pub(super) fn sync_device_chrome(&self, layout: &Layout) {
        let cfg = layout
            .device
            .filter(|_| !self.overlay && !self.fullscreen)
            .map(|d| {
                json!({
                    "area": d.area,
                    "frame": d.frame,
                    "size": self.device,
                    "bar": DEVICE_BAR_H,
                    "gutter": DEVICE_GUTTER,
                    "tools": self.devtools_open,
                })
            });
        if cfg.is_none() && !self.device_chrome_shown.get() {
            return;
        }
        self.device_chrome_shown.set(cfg.is_some());
        let arg = cfg.map(|c| c.to_string()).unwrap_or_else(|| "null".into());
        let _ = self
            .sidebar
            .evaluate_script(&call_script(DEVICE_CHROME_JS, arg));
    }

    fn focus_page(&self) {
        if let Some(wv) = self.visible_webview() {
            let _ = wv.focus();
        }
    }

    /// One-click layouts for developers: "tools" (page + DevTools docked right) and "widths"
    /// (the page twice, a phone width next to the full width).
    pub(super) fn dev_layout(&mut self, kind: &str) {
        match kind {
            "tools" => self.layout_page_and_tools(),
            "widths" => self.layout_two_widths(),
            _ => {}
        }
    }

    fn layout_page_and_tools(&mut self) {
        if self.devtools_open && self.p.devtools_position == "right" {
            self.close_devtools();
            return;
        }
        if self.split.is_some() {
            self.close_split();
        }
        self.p.devtools_position = "right".into();
        self.save();
        if self.devtools_open {
            self.relayout();
        } else if let Err(e) = self.toggle_devtools() {
            log::warn!("could not open DevTools for the layout: {e:#}");
        }
    }

    fn layout_two_widths(&mut self) {
        if self.split.is_some() {
            self.close_split();
            return;
        }
        let url = self
            .visible
            .as_ref()
            .and_then(|id| self.tabs.iter().find(|t| &t.id == id))
            .map(|t| t.url.clone())
            .filter(|u| u.starts_with("http://") || u.starts_with("https://"));
        let Some(url) = url else {
            self.toast("Open a web page to compare widths.");
            return;
        };
        let narrow = self.device.map_or(DEVICE_DEFAULT[0], |d| d[0]);
        let known: HashSet<String> = self.tabs.iter().map(|t| t.id.clone()).collect();
        if let Err(e) = self.open_background_tab(&url) {
            log::warn!("could not open the second width: {e:#}");
            return;
        }
        let Some(copy) = self
            .tabs
            .iter()
            .find(|t| !known.contains(&t.id))
            .map(|t| t.id.clone())
        else {
            return;
        };
        self.bg_tabs.remove(&copy);
        if let Err(e) = self.open_split(&copy, None, false, true) {
            log::warn!("could not split for the widths layout: {e:#}");
            return;
        }
        let [_, _, w, _] = self.content_box();
        if let Some(sp) = self.split.as_mut() {
            sp.ratio = (narrow / (w - INSET).max(1.0)).clamp(0.15, 0.85);
        }
        self.relayout();
    }
}

/// Largest emulated page that fits `area` next to its handles and toolbar.
fn device_limits(area: [f64; 4]) -> (f64, f64) {
    (
        (area[2] - 2.0 * DEVICE_GUTTER).max(1.0),
        (area[3] - DEVICE_BAR_H - DEVICE_GUTTER).max(1.0),
    )
}
