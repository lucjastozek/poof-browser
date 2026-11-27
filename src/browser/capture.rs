//! Capturing a chosen area, an element or the whole page, and screenshots made with the system
//! (they show up in Downloads by themselves).
//!
//! The overlay (`js/capture.js`) runs in the page and only reports a rectangle. The picture is
//! taken here with the DevTools protocol, so nothing the page draws can alter or read it. The page
//! is untrusted: a report counts only with the one-time token generated for this capture.

use super::*;
use base64::Engine;
use std::sync::Mutex;

const CAPTURE_JS: &str = include_str!("js/capture.js");
/// Longest side of a picture (Chromium refuses more).
const MAX_SIDE: f64 = 16_000.0;
const MAX_PIXELS: f64 = 120_000_000.0;
/// (tab, token) of the capture waiting for a rectangle.
/// File-name time of the capture in progress, from the page (sanitized).
pub(super) static STAMP: Mutex<String> = Mutex::new(String::new());
static PENDING: Mutex<Option<(String, String)>> = Mutex::new(None);

/// A rectangle in page coordinates (CSS pixels, document origin) and the display scale.
#[derive(Clone, Copy)]
pub(super) struct Region {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub dpr: f64,
}

impl Region {
    fn valid(&self) -> bool {
        [self.x, self.y, self.w, self.h, self.dpr]
            .iter()
            .all(|v| v.is_finite())
            && self.w >= 1.0
            && self.h >= 1.0
            && self.dpr > 0.0
    }
}

impl Browser {
    /// Starts a capture on the visible page: the overlay lets the user pick area, element or full page.
    pub(super) fn start_capture(&mut self, mode: &str) {
        let Some(tab) = self.visible.clone() else {
            return;
        };
        if !cfg!(windows) {
            self.toast("Capturing the page isn’t available on this system yet.");
            return;
        }
        let token = uuid::Uuid::new_v4().simple().to_string();
        if let Ok(mut pending) = PENDING.lock() {
            *pending = Some((tab, token.clone()));
        }
        let mode = if matches!(mode, "area" | "element" | "full") {
            mode
        } else {
            "area"
        };
        let accent = self.cur_space().color.clone();
        let accent = if ok_color(&accent) {
            accent
        } else {
            "#6dc5e2".into()
        };
        let script = CAPTURE_JS
            .replace("__NONCE__", &token)
            .replace("__ACCENT__", &accent)
            .replace("__MODE__", mode);
        self.eval_visible(&script);
    }

    /// The overlay reported a rectangle (`None` for cancel). UNTRUSTED: needs the pending token.
    pub(super) fn on_page_capture(&mut self, tab: &str, token: &str, region: Option<Region>) {
        let taken = match PENDING.lock() {
            Ok(mut pending) => match pending.as_ref() {
                Some((t, n)) if t == tab && n == token => pending.take(),
                _ => None,
            },
            Err(_) => None,
        };
        if taken.is_none() {
            return;
        }
        let Some(mut region) = region.filter(Region::valid) else {
            return;
        };
        // Keep the picture within what the browser engine can make.
        region.dpr = region.dpr.clamp(0.5, 4.0);
        region.w = region.w.min(MAX_SIDE / region.dpr);
        region.h = region.h.min(MAX_SIDE / region.dpr);
        let pixels = region.w * region.h * region.dpr * region.dpr;
        if pixels > MAX_PIXELS {
            region.dpr = (MAX_PIXELS / (region.w * region.h)).sqrt().max(0.25);
        }
        let Some(wv) = self.visible_webview() else {
            return;
        };
        if !capture_region(wv, region, &self.proxy) {
            self.toast("Couldn’t capture this page.");
        }
    }

    /// A finished picture: saved next to the system's screenshots and added to Downloads.
    pub(super) fn on_capture_done(&mut self, png: Option<Vec<u8>>) {
        let Some(png) = png.filter(|p| !p.is_empty()) else {
            self.toast("Couldn’t capture this page.");
            return;
        };
        let dir = shots_dir().join("Poof");
        let stamp = STAMP
            .lock()
            .map(|s| s.trim().to_string())
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| now_secs().to_string());
        let path = unique_path(&dir, &format!("Poof Capture {stamp}"), "png");
        let saved = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, &png));
        match saved {
            Ok(()) => {
                self.add_screenshot(&path);
                self.toast("Screenshot saved to Downloads");
            }
            Err(e) => {
                log::warn!("saving the capture failed: {e}");
                self.toast("Couldn’t save the screenshot.");
            }
        }
    }

    /// Lists a screenshot file as a finished download (once per file).
    pub(super) fn add_screenshot(&mut self, path: &Path) {
        let text = path.to_string_lossy();
        if self.downloads.iter().any(|d| d.path == text) {
            return;
        }
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let mut entry = DownloadEntry::new(
            uuid::Uuid::new_v4().to_string(),
            "screenshot:".into(),
            path,
            size,
            now_secs(),
        );
        entry.status = DownloadStatus::Completed;
        entry.received = size;
        self.downloads.insert(0, entry);
        self.trim_downloads();
        self.persist_downloads();
        self.push_downloads();
    }
}

/// Where the system saves screenshots (and where Poof's own captures go, in a sub-folder).
fn shots_dir() -> PathBuf {
    watched_dirs()
        .into_iter()
        .next()
        .unwrap_or_else(|| std::env::temp_dir())
}

fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let mut path = dir.join(format!("{stem}.{ext}"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{stem} ({n}).{ext}"));
        n += 1;
    }
    path
}

#[cfg(windows)]
fn capture_region(wv: &WebView, r: Region, proxy: &EventLoopProxy<UserEvent>) -> bool {
    use webview2_com::CallDevToolsProtocolMethodCompletedHandler;
    use windows::core::{w, HSTRING};

    let params = json!({
        "format": "png",
        "captureBeyondViewport": true,
        "fromSurface": true,
        "clip": { "x": r.x, "y": r.y, "width": r.w, "height": r.h, "scale": r.dpr },
    })
    .to_string();
    unsafe {
        let Ok(core) = wv.controller().CoreWebView2() else {
            return false;
        };
        let proxy = proxy.clone();
        let handler =
            CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |result, reply| {
                let png = result.ok().and_then(|_| {
                    let value: Value = serde_json::from_str(&reply).ok()?;
                    let data = value["data"].as_str()?;
                    base64::engine::general_purpose::STANDARD.decode(data).ok()
                });
                let _ = proxy.send_event(UserEvent::CaptureDone(png));
                Ok(())
            }));
        match core.CallDevToolsProtocolMethod(
            w!("Page.captureScreenshot"),
            &HSTRING::from(params),
            &handler,
        ) {
            Ok(()) => true,
            Err(error) => {
                log::warn!("capturing the page failed: {error}");
                false
            }
        }
    }
}

#[cfg(not(windows))]
fn capture_region(_wv: &WebView, _r: Region, _proxy: &EventLoopProxy<UserEvent>) -> bool {
    false
}

/* ---------- screenshots made with the system ---------- */

/// Folders where the system saves screenshots: the first is also where Poof saves its own.
fn watched_dirs() -> Vec<PathBuf> {
    let mut dirs_out = Vec::new();
    #[cfg(target_os = "macos")]
    {
        let custom = std::process::Command::new("defaults")
            .args(["read", "com.apple.screencapture", "location"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty());
        if let Some(path) = custom {
            let path = match path.strip_prefix("~/") {
                Some(rest) => dirs::home_dir().map(|h| h.join(rest)).unwrap_or_default(),
                None => PathBuf::from(path),
            };
            dirs_out.push(path);
        }
        dirs_out.extend(dirs::desktop_dir());
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(pictures) = dirs::picture_dir() {
        dirs_out.push(pictures.join("Screenshots"));
    }
    dirs_out.retain(|d| d.is_dir() || d.parent().is_some_and(Path::is_dir));
    dirs_out
}

/// A file the system's screenshot tool made: Windows “Screenshot (3).png” / “Screenshot 2026-…”,
/// macOS “Screenshot …” (older: “Screen Shot …”). Hidden temporary files are ignored.
fn is_system_screenshot(name: &str) -> bool {
    let lower = name.to_lowercase();
    !name.starts_with('.')
        && (name.starts_with("Screenshot") || name.starts_with("Screen Shot"))
        && [".png", ".jpg", ".jpeg"].iter().any(|e| lower.ends_with(e))
}

fn scan(dir: &Path) -> HashMap<PathBuf, u64> {
    let mut found = HashMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !is_system_screenshot(&name) {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            if meta.is_file() {
                found.insert(entry.path(), meta.len());
            }
        }
    }
    found
}

/// Watches the screenshot folders and reports each new screenshot once its file is complete
/// (its size stayed the same between two checks). Files that exist at start are not reported.
pub(super) fn spawn_screenshot_watch(proxy: EventLoopProxy<UserEvent>) {
    let spawned = std::thread::Builder::new()
        .name("screenshot-watch".into())
        .spawn(move || {
            let mut seen: HashSet<PathBuf> = HashSet::new();
            let mut waiting: HashMap<PathBuf, u64> = HashMap::new();
            let mut first = true;
            loop {
                for dir in watched_dirs() {
                    for (path, size) in scan(&dir) {
                        if first {
                            seen.insert(path);
                        } else if !seen.contains(&path) && size > 0 {
                            if waiting.get(&path) == Some(&size) {
                                waiting.remove(&path);
                                seen.insert(path.clone());
                                if proxy.send_event(UserEvent::SystemScreenshot(path)).is_err() {
                                    return;
                                }
                            } else {
                                waiting.insert(path, size);
                            }
                        }
                    }
                }
                first = false;
                std::thread::sleep(Duration::from_secs(2));
            }
        });
    if let Err(e) = spawned {
        log::warn!("screenshot watcher not started: {e}");
    }
}
