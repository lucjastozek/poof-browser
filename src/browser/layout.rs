//! Where everything goes: sidebar, page, DevTools panel and split panes.

use super::*;

impl Browser {
    pub(super) fn sidebar_left(&self) -> f64 {
        if self.p.sidebar_collapsed {
            0.0
        } else if self.p.sidebar_compact {
            SIDEBAR_COMPACT_W
        } else {
            self.p.sidebar_width
        }
    }

    /// [x, y, w, h] of the area normally occupied by web content.
    pub(super) fn content_box(&self) -> [f64; 4] {
        let (w, h) = logical(&self.window);
        let left = self.sidebar_left();
        [
            left + INSET,
            TOPBAR_H,
            (w - left - 2.0 * INSET).max(1.0),
            (h - TOPBAR_H - INSET).max(1.0),
        ]
    }

    pub(super) fn content_rect(&self) -> Rect {
        if self.fullscreen {
            full_rect(&self.window)
        } else {
            rect_of(self.content_box())
        }
    }

    /// [x, y, w, h] the DevTools frontend occupies in device mode: the content area minus the header.
    pub(super) fn responsive_tools_box(&self) -> [f64; 4] {
        let [x, y, w, h] = self.content_box();
        [x, y + HEADER_H, w, (h - HEADER_H).max(1.0)]
    }

    /// Largest panel size that still leaves a usable page.
    pub(super) fn devtools_limit(&self) -> f64 {
        let [_, _, w, h] = self.content_box();
        let avail = if matches!(self.p.devtools_position.as_str(), "left" | "right") {
            w
        } else {
            h
        };
        (avail - PANEL_GAP - PAGE_MIN).clamp(PANEL_MIN, PANEL_MAX)
    }

    pub(super) fn devtools_size(&self) -> f64 {
        self.p.devtools_size.clamp(PANEL_MIN, self.devtools_limit())
    }

    pub(super) fn layout(&self) -> Layout {
        let mut out = Layout {
            page: self.content_rect(),
            page_visible: true,
            tools: None,
            header: None,
            handle: None,
            other: None,
            split_handle: None,
        };

        if !self.fullscreen && !self.devtools_open {
            if let Some((first, second, handle)) = self.split_rects() {
                let second_focused = self
                    .split
                    .as_ref()
                    .map(|s| self.visible.as_deref() == Some(s.b.as_str()))
                    .unwrap_or(false);
                let (mine, theirs) = if second_focused {
                    (second, first)
                } else {
                    (first, second)
                };
                out.page = rect_of(mine);
                out.other = Some(rect_of(theirs));
                out.split_handle = Some(handle);
                return out;
            }
        }

        if !self.devtools_open || self.fullscreen {
            return out;
        }
        let [x, y, w, h] = self.content_box();

        if self.responsive_mode {
            // The frontend draws the device frame, rulers and toolbars over the whole content
            // area; the real page WebView sits exactly on the emulated screen.
            let [tx, ty, tw, th] = self.responsive_tools_box();
            out.header = Some([x, y, w, HEADER_H]);
            out.tools = Some(rect(tx, ty, tw, th));
            out.page_visible = false;
            if let Some([bx, by, bw, bh]) = self.responsive_bounds {
                let (left, top) = ((tx + bx).max(tx), (ty + by).max(ty));
                let (right, bottom) = ((tx + bx + bw).min(tx + tw), (ty + by + bh).min(ty + th));
                if right - left >= 1.0 && bottom - top >= 1.0 {
                    out.page = rect(left, top, right - left, bottom - top);
                    out.page_visible = true;
                }
            }
            return out;
        }

        let size = self.devtools_size();
        let gap = PANEL_GAP;
        let (page, panel, handle) = match self.p.devtools_position.as_str() {
            "left" => (
                [x + size + gap, y, w - size - gap, h],
                [x, y, size, h],
                [x + size, y, gap, h],
            ),
            "top" => (
                [x, y + size + gap, w, h - size - gap],
                [x, y, w, size],
                [x, y + size, w, gap],
            ),
            "bottom" => (
                [x, y, w, h - size - gap],
                [x, y + h - size, w, size],
                [x, y + h - size - gap, w, gap],
            ),
            _ => (
                [x, y, w - size - gap, h],
                [x + w - size, y, size, h],
                [x + w - size - gap, y, gap, h],
            ),
        };
        out.page = rect_of(page);
        out.header = Some([panel[0], panel[1], panel[2], HEADER_H]);
        out.tools = Some(rect(
            panel[0],
            panel[1] + HEADER_H,
            panel[2],
            panel[3] - HEADER_H,
        ));
        out.handle = Some(handle);
        out
    }

    pub fn relayout(&self) {
        let (w, h) = logical(&self.window);
        if w < 1.0 || h < 1.0 {
            return; // minimized
        }
        let scale = self.window.scale_factor();
        let _ = self.sidebar.set_bounds(full_rect(&self.window));
        let _ = self.sidebar.set_visible(true);
        self.layout_peek();
        self.sync_fullscreen_backdrop();
        let layout = self.layout();

        let hide_pages = self.overlay || self.tab_drag;
        let page_shown = !hide_pages && layout.page_visible;
        if let Some(wv) = self.visible_webview() {
            if let Err(error) = apply_webview_bounds(wv, layout.page, scale) {
                log::warn!("resizing content WebView failed: {error:#}");
            }
            let _ = wv.set_visible(!hide_pages);
            #[cfg(windows)]
            {
                let (px, py, pw, ph) = rect_parts(layout.page, scale);
                apply_webview_radius(
                    wv,
                    pw,
                    ph,
                    scale,
                    !self.fullscreen && !self.devtools_open,
                    &self.page_holes(px, py),
                );
                unsafe {
                    let _ = wv.controller().NotifyParentWindowPositionChanged();
                }
                if page_shown && self.devtools_open && self.responsive_mode {
                    raise_webview(wv);
                }
            }
        }
        if let Some(wv) = self
            .split_other_id()
            .and_then(|id| self.tabs.iter().find(|t| t.id == id))
            .and_then(|t| t.webview.as_ref())
        {
            match layout.other {
                Some(r) => {
                    if let Err(error) = apply_webview_bounds(wv, r, scale) {
                        log::warn!("resizing split WebView failed: {error:#}");
                    }
                    #[cfg(windows)]
                    {
                        let (px, py, pw, ph) = rect_parts(r, scale);
                        apply_webview_radius(wv, pw, ph, scale, true, &self.page_holes(px, py));
                        unsafe {
                            let _ = wv.controller().NotifyParentWindowPositionChanged();
                        }
                    }
                    let _ = wv.set_visible(!self.overlay);
                }
                None => {
                    let _ = wv.set_visible(false);
                }
            }
        }

        if let Some(devtools) = self.devtools.as_ref() {
            match layout.tools {
                Some(bounds) => {
                    if let Err(error) = apply_webview_bounds(devtools, bounds, scale) {
                        log::warn!("resizing DevTools WebView failed: {error:#}");
                    }
                    #[cfg(windows)]
                    {
                        let size = bounds.size.to_logical::<f64>(scale);
                        apply_webview_radius(devtools, size.width, size.height, scale, true, &[]);
                    }
                    let show = self.devtools_open
                        && !self.overlay
                        && !self.fullscreen
                        && self.devtools_tab.is_some()
                        && self.devtools_tab == self.visible;
                    let _ = devtools.set_visible(show);
                }
                None => {
                    let _ = devtools.set_visible(false);
                }
            }
        }

        let want_polling = self.devtools_open && !self.overlay && !self.fullscreen;
        let was_polling = self.devtools_polling.swap(want_polling, Ordering::Relaxed);
        if want_polling && !was_polling {
            self.poll_thread.unpark();
        }
        self.sync_devtools_chrome(&layout);
        self.sync_split_chrome(&layout);
    }
}
