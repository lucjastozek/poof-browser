//! The DevTools panel: attaching to the page, the frontend URL and the layout state it reports.

use super::*;

#[cfg(windows)]
pub(super) fn devtools_target_id(webview: &WebView) -> Result<String> {
    use std::sync::Mutex;
    use webview2_com::CallDevToolsProtocolMethodCompletedHandler;
    use windows::core::w;

    let core = unsafe { webview.controller().CoreWebView2()? };
    let result = Arc::new(Mutex::new(None));
    let result_slot = Arc::clone(&result);
    CallDevToolsProtocolMethodCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            core.CallDevToolsProtocolMethod(w!("Target.getTargetInfo"), w!("{}"), &handler)?;
            Ok(())
        }),
        Box::new(move |status, value| {
            status?;
            if let Ok(mut slot) = result_slot.lock() {
                *slot = Some(value);
            }
            Ok(())
        }),
    )?;
    let raw = result
        .lock()
        .map_err(|_| anyhow::anyhow!("DevTools target result lock poisoned"))?
        .take()
        .ok_or_else(|| anyhow::anyhow!("WebView2 returned no DevTools target"))?;
    let parsed: Value = serde_json::from_str(&raw)?;
    parsed["targetInfo"]["targetId"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("WebView2 returned an invalid DevTools target ID"))
}

#[cfg(windows)]
pub(super) fn devtools_frontend_url(port: u16, target_id: &str) -> Result<String> {
    if port == 0
        || target_id.is_empty()
        || !target_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        anyhow::bail!("invalid WebView2 DevTools endpoint");
    }

    // WebView2 ships the real frontend behind its privileged devtools:// scheme.
    Ok(format!(
        "devtools://devtools/bundled/inspector.html?can_dock=true&ws=127.0.0.1:{port}/devtools/page/{target_id}"
    ))
}

#[cfg(windows)]
pub(super) fn resolve_devtools_frontend(profile_dir: &Path, target_id: &str) -> Result<String> {
    let port_file = [
        profile_dir.join("EBWebView").join("DevToolsActivePort"),
        profile_dir.join("DevToolsActivePort"),
    ];
    for _ in 0..100 {
        for path in &port_file {
            let Ok(contents) = fs::read_to_string(path) else {
                continue;
            };
            let Some(port) = contents
                .lines()
                .next()
                .and_then(|line| line.trim().parse::<u16>().ok())
            else {
                continue;
            };
            if let Ok(url) = devtools_frontend_url(port, target_id) {
                return Ok(url);
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    anyhow::bail!("WebView2 DevTools endpoint did not become ready for the active tab")
}

/// Spawns the DevTools poll thread. It sleeps (parked, zero wakeups) until `active` is set, then
/// asks the main loop for a poll every 40 ms. Returns the handle used to wake it.
pub(super) fn spawn_devtools_poll(
    proxy: EventLoopProxy<UserEvent>,
    active: Arc<AtomicBool>,
) -> std::thread::Thread {
    let handle = std::thread::spawn(move || loop {
        while !active.load(Ordering::Relaxed) {
            std::thread::park();
        }
        std::thread::sleep(Duration::from_millis(40));
        let ev = UserEvent::Page {
            tab: String::new(),
            body: r#"{"t":"devtools_tick"}"#.to_string(),
        };
        if proxy.send_event(ev).is_err() {
            break;
        }
    });
    handle.thread().clone()
}

impl Browser {
    pub(super) fn toggle_devtools(&mut self) -> Result<()> {
        if !self.devtools_open && self.split.is_some() {
            self.toast("Close split view to use Developer Tools.");
            return Ok(());
        }
        if self.devtools_open {
            self.close_devtools();
            return Ok(());
        }
        self.devtools_open = true;
        self.responsive_mode = false;
        self.responsive_bounds = None;
        self.relayout();
        if let Err(error) = self.attach_devtools() {
            self.close_devtools();
            return Err(error);
        }
        Ok(())
    }

    /// Closes the panel and drops the frontend WebView (freeing its memory); dropping its
    /// connection to the page also removes any device-mode emulation from the page.
    pub(super) fn close_devtools(&mut self) {
        self.devtools_open = false;
        self.devtools_tab = None;
        self.responsive_mode = false;
        self.responsive_bounds = None;
        if let Some(devtools) = self.devtools.take() {
            let _ = devtools.set_visible(false);
            drop(devtools);
        }
        self.relayout();
        if let Some(wv) = self.visible_webview() {
            let _ = wv.focus();
        }
    }

    pub(super) fn attach_devtools(&mut self) -> Result<()> {
        let Some(tab_id) = self.visible.clone() else {
            self.close_devtools();
            self.toast("Open a page before opening Developer Tools.");
            return Ok(());
        };
        if self.devtools_tab.as_deref() == Some(tab_id.as_str()) && self.devtools.is_some() {
            self.relayout();
            return Ok(());
        }
        #[cfg(windows)]
        {
            let Some(tab) = self.tabs.iter().find(|tab| tab.id == tab_id) else {
                self.close_devtools();
                return Ok(());
            };
            let profile_id = tab.profile_id.clone();
            let target = tab.webview.as_ref().map(devtools_target_id);
            let target_id = match target {
                None => {
                    self.close_devtools();
                    return Ok(());
                }
                Some(Err(error)) => {
                    self.close_devtools();
                    self.toast(&format!(
                        "Could not identify the active DevTools target: {error}"
                    ));
                    return Ok(());
                }
                Some(Ok(target_id)) => target_id,
            };
            let profile_dir = self.root.join("profiles").join(&profile_id);
            let proxy = self.proxy.clone();
            std::thread::spawn(move || {
                let event = match resolve_devtools_frontend(&profile_dir, &target_id) {
                    Ok(url) => UserEvent::DevToolsReady { tab: tab_id, url },
                    Err(error) => UserEvent::DevToolsFailed {
                        tab: tab_id,
                        message: format!("Developer Tools could not attach: {error}"),
                    },
                };
                let _ = proxy.send_event(event);
            });
        }
        #[cfg(not(windows))]
        {
            self.close_devtools();
            self.toast("Embedded Developer Tools are available on Windows only.");
        }
        Ok(())
    }

    pub(super) fn open_devtools_view(&mut self, tab: &str, url: &str) -> Result<()> {
        // A fresh (or reloaded) frontend has none of our styles or icons yet.
        self.devtools_icons_applied = false;
        if let Some(devtools) = self.devtools.as_ref() {
            devtools.load_url(url)?;
            return Ok(());
        }
        let layout = self.layout();
        let proxy = self.proxy.clone();
        // No init script and no IPC: everything is driven from the native side
        // (see `DEVTOOLS_SYNC_JS`).
        let devtools = WebViewBuilder::new_with_web_context(&mut self.ui_ctx)
            .with_bounds(layout.tools.unwrap_or(layout.page))
            .with_visible(false)
            .with_devtools(false)
            .with_url(url)
            // Any navigation away from the frontend is refused so web content can never load
            // into this WebView.
            .with_navigation_handler(|target| {
                target.starts_with("devtools://devtools/") || target == "about:blank"
            })
            .with_new_window_req_handler(move |url, _features| {
                let _ = proxy.send_event(UserEvent::OpenTab { url });
                NewWindowResponse::Deny
            })
            .build_as_child(&self.window)?;
        #[cfg(windows)]
        register_browser_shortcuts(&devtools, self.proxy.clone(), tab.to_string());
        #[cfg(not(windows))]
        let _ = tab;
        self.devtools = Some(devtools);
        Ok(())
    }

    pub(super) fn show_devtools(&mut self, tab: &str, url: &str) {
        if !self.devtools_open || self.visible.as_deref() != Some(tab) {
            return;
        }
        if self.devtools_tab.as_deref() == Some(tab) && self.devtools.is_some() {
            self.relayout();
            return;
        }
        match self.open_devtools_view(tab, url) {
            Ok(()) => {
                self.devtools_tab = Some(tab.to_string());
                self.relayout();
                if !self.overlay && !self.fullscreen {
                    if let Some(devtools) = self.devtools.as_ref() {
                        let _ = devtools.focus();
                    }
                }
            }
            Err(error) => {
                self.close_devtools();
                self.toast(&format!("Could not open Developer Tools: {error}"));
            }
        }
    }

    pub(super) fn devtools_sync_call(&self) -> String {
        let [_, _, width, height] = self.responsive_tools_box();
        let dark = match self.p.theme.as_str() {
            "dark" => true,
            "light" => false,
            _ => self.window.theme() == Theme::Dark,
        };
        // The frontend rebuilds the ui/styles/foundation/theme.css token formulas from these values. The (large)
        // icon set is only included until the frontend reports that it has applied it.
        let mut cfg = json!({
            "position": self.p.devtools_position,
            "size": self.devtools_size(),
            "width": width.round(),
            "height": height.round(),
            "dark": dark,
            "accent": self.cur_space().color,
            "icons_key": self.devtools_icons_version,
            "icon_px": DEVTOOLS_ICON_PX,
        });
        if self.devtools_icons_version != 0 && !self.devtools_icons_applied {
            cfg["icons"] = json!(self.devtools_icons);
        }
        format!("({DEVTOOLS_SYNC_JS})({cfg})")
    }

    /// Asks the frontend for its state and enforces the layout.
    pub(super) fn poll_devtools(&self) {
        if !self.devtools_open || self.fullscreen || self.overlay {
            return;
        }
        if self.devtools_tab.is_none() || self.devtools_tab != self.visible {
            return;
        }
        let Some(devtools) = self.devtools.as_ref() else {
            return;
        };

        // One poll in flight at a time (with a timeout in case a callback never arrives).
        let now = now_ms();
        let started = self.poll_busy.load(Ordering::Acquire);
        if started != 0 && now.saturating_sub(started) < 1500 {
            return;
        }
        self.poll_busy.store(now, Ordering::Release);

        let busy = Arc::clone(&self.poll_busy);
        let proxy = self.proxy.clone();
        let queued =
            devtools.evaluate_script_with_callback(&self.devtools_sync_call(), move |result| {
                busy.store(0, Ordering::Release);
                let payload = if result.trim().is_empty() {
                    "null"
                } else {
                    result.as_str()
                };
                let body = format!(r#"{{"t":"devtools_state","s":{payload}}}"#);
                let _ = proxy.send_event(UserEvent::Page {
                    tab: String::new(),
                    body,
                });
            });
        if queued.is_err() {
            self.poll_busy.store(0, Ordering::Release);
        }
    }

    pub(super) fn apply_devtools_report(&mut self, report: DevToolsReport) {
        if !self.devtools_open
            || self.fullscreen
            || self.devtools_tab.is_none()
            || self.devtools_tab != self.visible
        {
            return;
        }
        if report.ready && report.icons_key == self.devtools_icons_version {
            self.devtools_icons_applied = true;
        }
        for name in report.icon_misses.iter().take(60) {
            if self.devtools_icon_misses.insert(clip(name, 60)) {
                log::info!("DevTools icon without a browser equivalent: {name}");
            }
        }
        let responsive = report.ready && report.responsive;
        // The emulated-screen bounds belong to the previous layout until the frontend has been
        // resized to the WebView size native code asked for.
        let [_, _, w, h] = self.responsive_tools_box();
        let in_sync = (report.iw - w).abs() <= 4.0 && (report.ih - h).abs() <= 4.0;
        let bounds = if responsive && in_sync {
            report.bounds.filter(|b| b[2] >= 1.0 && b[3] >= 1.0)
        } else {
            None
        };
        if responsive != self.responsive_mode || bounds != self.responsive_bounds {
            self.responsive_mode = responsive;
            self.responsive_bounds = bounds;
            self.relayout();
        }
    }

    /// Shows, moves or hides the resize handle and header strip drawn by the sidebar WebView.
    pub(super) fn sync_devtools_chrome(&self, layout: &Layout) {
        let visible = self.devtools_open && !self.overlay && !self.fullscreen;
        let cfg = layout.header.filter(|_| visible).map(|header| {
            json!({
                "header": header,
                "handle": layout.handle,
                "position": self.p.devtools_position,
                "gap": PANEL_GAP,
                "box": self.content_box(),
            })
        });
        if cfg.is_none() && !self.chrome_shown.get() {
            return;
        }
        self.chrome_shown.set(cfg.is_some());
        let arg = cfg.map(|c| c.to_string()).unwrap_or_else(|| "null".into());
        let _ = self
            .sidebar
            .evaluate_script(&format!("({DEVTOOLS_CHROME_JS})({arg})"));
    }

    /// Merges the sidebar's own icons over the bundled defaults for the DevTools frontend.
    pub(super) fn set_devtools_icons(&mut self, icons: HashMap<String, String>) {
        // The sidebar's own icons win over the bundled Mingcute defaults.
        let mut merged = default_devtools_icons();
        merged.extend(clean_icons(icons));
        if merged != self.devtools_icons {
            self.devtools_icons = merged;
            self.devtools_icons_version += 1;
            self.devtools_icons_applied = false;
        }
    }
}
