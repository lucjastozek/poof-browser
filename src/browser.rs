use crate::{
    extensions, pip,
    state::{new_id, valid_icon, Kind, Persisted, Profile, Saved, Space, TabFolder},
    store, UserEvent,
};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    cell::{Cell, RefCell},
    collections::{hash_map::DefaultHasher, HashMap, HashSet},
    fs,
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tao::{
    event_loop::EventLoopProxy,
    window::{Theme, Window},
};
use wry::{
    dpi::{LogicalPosition, LogicalSize},
    NewWindowResponse, Rect, WebContext, WebView, WebViewBuilder,
};

#[cfg(windows)]
use std::path::Path;
#[cfg(windows)]
use wry::WebViewExtWindows;

#[cfg(windows)]
fn apply_webview_bounds(wv: &WebView, bounds: Rect, scale: f64) -> Result<()> {
    use windows::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER},
    };

    // Wry's set_bounds positions the child HWND asynchronously on Windows, which visibly lags
    // behind rapid pointer-move events, so the HWND is also positioned synchronously.
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
    }
    Ok(())
}

#[cfg(not(windows))]
fn apply_webview_bounds(wv: &WebView, bounds: Rect, _scale: f64) -> Result<()> {
    wv.set_bounds(bounds)?;
    Ok(())
}

/// In device mode the page WebView overlaps the DevTools WebView and must stay on top.
#[cfg(windows)]
fn raise_webview(wv: &WebView) {
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
fn apply_webview_radius(wv: &WebView, width: f64, height: f64, scale: f64, rounded: bool) {
    use windows::Win32::{
        Foundation::HWND,
        Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, SetWindowRgn, HGDIOBJ},
    };

    let mut hwnd = HWND::default();
    unsafe {
        if wv.controller().ParentWindow(&mut hwnd).is_err() || hwnd.0.is_null() {
            return;
        }
        if !rounded {
            let _ = SetWindowRgn(hwnd, None, true);
            return;
        }

        let width = (width * scale).round().max(1.0) as i32;
        let height = (height * scale).round().max(1.0) as i32;
        let radius = (8.0 * scale).round().max(1.0) as i32;
        let region = CreateRoundRectRgn(0, 0, width, height, radius * 2, radius * 2);
        if region.0.is_null() {
            return;
        }
        // SetWindowRgn takes ownership of the region only when it succeeds.
        if SetWindowRgn(hwnd, Some(region), true) == 0 {
            let _ = DeleteObject(HGDIOBJ(region.0));
        }
    }
}

/// Asks WebView2 to trim (LOW) or restore (NORMAL) the memory it keeps for this page. Hidden
/// tabs get LOW so the engine drops caches and compresses memory right away; best effort, and
/// silently ignored on runtimes that predate the API.
#[cfg(windows)]
fn set_memory_level(wv: &WebView, low: bool) {
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
fn set_memory_level(_wv: &WebView, _low: bool) {}

/// True while the page is producing sound; such tabs are never put to sleep.
#[cfg(windows)]
fn is_playing_audio(wv: &WebView) -> bool {
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
fn is_playing_audio(_wv: &WebView) -> bool {
    false
}

const SIDEBAR_MIN_W: f64 = 180.0;
const SIDEBAR_MAX_W: f64 = 480.0;
const SIDEBAR_COMPACT_W: f64 = 56.0;
const INSET: f64 = 8.0;
const TOPBAR_H: f64 = 40.0;

const PANEL_MIN: f64 = 240.0;
const PANEL_MAX: f64 = 800.0;
const PANEL_DEFAULT: f64 = 420.0;
/// The gap between page and panel holds the resize handle.
const PANEL_GAP: f64 = INSET;
const PAGE_MIN: f64 = 160.0;
/// Strip above the DevTools frontend holding the title and the close button.
const HEADER_H: f64 = 28.0;

/// Largest favicon (URL or data: URI) kept in memory, pushed to the UI and persisted.
const FAVICON_MAX: usize = 48_000;
/// Favicons larger than this are kept in memory but not written to state.json.
const FAVICON_PERSIST_MAX: usize = 16_000;
/// State writes closer together than this are coalesced and flushed by the event-loop timer.
const SAVE_MIN_INTERVAL: Duration = Duration::from_secs(2);

/// Popular sites shown in the new-tab bar until the user's own history takes over.
const DEFAULT_SITES: &[(&str, &str)] = &[
    ("YouTube", "https://www.youtube.com/"),
    ("Google", "https://www.google.com/"),
    ("Gmail", "https://mail.google.com/"),
    ("GitHub", "https://github.com/"),
    ("Reddit", "https://www.reddit.com/"),
    ("Wikipedia", "https://www.wikipedia.org/"),
    ("Netflix", "https://www.netflix.com/"),
    ("Spotify", "https://open.spotify.com/"),
    ("Twitch", "https://www.twitch.tv/"),
    ("Instagram", "https://www.instagram.com/"),
    ("Chrome Web Store", "https://chromewebstore.google.com/"),
];

/// Injected into every content page (top frame only). Reports URL/favicon, forwards shortcuts.
const INIT_JS: &str = include_str!("browser/init_script.js");

/// Draws the DevTools resize handle and header strip. They live in the sidebar WebView, whose IPC
/// is known to work, so neither depends on the DevTools frontend. Called with a config object,
/// or null to hide both.
const DEVTOOLS_CHROME_JS: &str = r##"
(cfg => {
  let ui = window.__poofDevtoolsUi;
  if (!cfg) {
    if (ui) ui.handle.style.display = ui.header.style.display = "none";
    return;
  }
  if (!ui) {
    const post = msg => window.ipc.postMessage(JSON.stringify(msg));

    const handle = document.createElement("div");
    handle.style.cssText = "position:fixed;z-index:2147483647;touch-action:none;user-select:none;";
    const line = document.createElement("div");
    line.style.cssText = "position:absolute;border-radius:2px;background:rgba(128,128,128,0);transition:background .12s;";
    handle.appendChild(line);
    const paint = on => { line.style.background = on ? "rgba(128,128,128,.55)" : "rgba(128,128,128,0)"; };

    let drag = null, raf = 0, last = 0;
    const send = (size, persist) => post({ cmd: "dev_tools_size", size, persist });
    const sizeAt = e => {
      const [cx, cy, cw, ch] = ui.cfg.box, gap = ui.cfg.gap;
      switch (ui.cfg.position) {
        case "left": return e.clientX - cx - gap / 2;
        case "top": return e.clientY - cy - gap / 2;
        case "bottom": return cy + ch - e.clientY - gap / 2;
        default: return cx + cw - e.clientX - gap / 2;
      }
    };
    handle.addEventListener("pointerenter", () => paint(true));
    handle.addEventListener("pointerleave", () => { if (drag === null) paint(false); });
    handle.addEventListener("pointerdown", e => {
      if (e.button !== 0) return;
      drag = e.pointerId;
      handle.setPointerCapture(e.pointerId);
      paint(true);
      e.preventDefault();
    });
    handle.addEventListener("pointermove", e => {
      if (drag !== e.pointerId) return;
      last = sizeAt(e);
      if (!raf) raf = requestAnimationFrame(() => { raf = 0; send(last, false); });
    });
    const end = e => {
      if (drag !== e.pointerId) return;
      drag = null;
      if (raf) { cancelAnimationFrame(raf); raf = 0; }
      send(sizeAt(e), true);
      paint(false);
    };
    handle.addEventListener("pointerup", end);
    handle.addEventListener("pointercancel", end);

    const header = document.createElement("div");
    header.style.cssText = "position:fixed;z-index:2147483647;align-items:center;justify-content:space-between;box-sizing:border-box;padding:0 4px 0 10px;border-radius:8px 8px 0 0;background:rgba(128,128,128,.12);font:12px system-ui,sans-serif;user-select:none;";
    const title = document.createElement("span");
    title.textContent = "Developer Tools";
    title.style.cssText = "opacity:.7;flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;";
    const close = document.createElement("button");
    close.textContent = "\u00d7";
    close.title = "Close Developer Tools (F12)";
    close.setAttribute("aria-label", "Close Developer Tools");
    close.style.cssText = "all:unset;width:22px;height:22px;line-height:22px;text-align:center;border-radius:4px;cursor:pointer;font-size:16px;";
    close.addEventListener("pointerenter", () => { close.style.background = "rgba(128,128,128,.3)"; });
    close.addEventListener("pointerleave", () => { close.style.background = "transparent"; });
    close.addEventListener("click", () => post({ cmd: "dev_tools_toggle" }));
    header.append(title, close);

    document.body.append(handle, header);
    ui = window.__poofDevtoolsUi = { handle, line, header, title, cfg };
  }
  ui.cfg = cfg;

  const place = (el, b) => Object.assign(el.style, {
    display: "block", left: b[0] + "px", top: b[1] + "px", width: b[2] + "px", height: b[3] + "px",
  });
  place(ui.header, cfg.header);
  ui.header.style.display = "flex";

  if (!cfg.handle) { ui.handle.style.display = "none"; return; }
  const vertical = cfg.position === "left" || cfg.position === "right";
  place(ui.handle, cfg.handle);
  ui.handle.style.cursor = vertical ? "col-resize" : "row-resize";
  Object.assign(ui.line.style, vertical
    ? { left: "50%", top: "0", width: "2px", height: "100%", transform: "translateX(-50%)" }
    : { top: "50%", left: "0", height: "2px", width: "100%", transform: "translateY(-50%)" });
})
"##;

/// Evaluated in the DevTools frontend by the native side every few milliseconds while DevTools is
/// open. The frontend has no IPC, so the result comes back through the evaluate callback.
///
/// The frontend must be docked: undocked, device mode opens in a separate toolbox window that
/// this WebView never shows, and the inspector cannot sit next to the emulated screen. Docked:
///  * panel mode: the main split is hidden, so the inspector fills its panel-sized WebView
///  * device mode: the main split is shown next to the inspector and reports where the emulated
///    screen is, so native code can put the real page WebView exactly there
///
/// Nothing is touched before the frontend has presented its UI: importing its modules earlier
/// would evaluate them out of order and break the frontend.
const DEVTOOLS_SYNC_JS: &str = r##"
(cfg => {
  const st = window.__poof || (window.__poof = {
    dock: null, loading: false, failed: false, pos: null, resp: null, settle: 0,
    emu: null, emuTried: false, want: 0, from: false, lastOn: false, hooked: false, hideAt: 0,
  });
  const wait = () => ({ ready: false });
  try {
    if (!document.querySelector(".root-view")) return wait();

    // Closing is handled by the native header; the frontend's own button does nothing here.
    if (!document.getElementById("__poof_style")) {
      const style = document.createElement("style");
      style.id = "__poof_style";
      style.textContent = ".close-devtools{display:none!important}";
      document.head.appendChild(style);
    }

    // A click on the device emulation button (or Ctrl+Shift+M) expands the frontend immediately.
    // composedPath() sees through shadow roots.
    if (!st.hooked) {
      st.hooked = true;
      const mark = () => { st.want = Date.now(); st.from = st.lastOn; };
      const isToggle = n => n && n.getAttribute && /device (emulation|toolbar)/i.test(
        (n.getAttribute("aria-label") || "") + " " + (n.getAttribute("title") || ""));
      document.addEventListener("click", e => {
        if (e.composedPath().some(isToggle)) mark();
      }, true);
      document.addEventListener("keydown", e => {
        if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === "M" || e.key === "m")) mark();
      }, true);
    }

    // Toolbar buttons live in shadow roots, so a document-level stylesheet can't reach them.
    // Hide the screencast toggle and the built-in close button (the native header has its own)
    // by walking the shadow roots (throttled; the toolbar can re-render).
    if (Date.now() - st.hideAt > 300) {
      st.hideAt = Date.now();
      const attrs = ["title", "aria-label", "jslog"];
      const hide = root => {
        for (const el of root.querySelectorAll("*")) {
          if (el.shadowRoot) hide(el.shadowRoot);
          const cls = typeof el.className === "string" ? el.className : "";
          if (!/close-devtools/.test(cls) && !attrs.some(a => /screencast/i.test(el.getAttribute(a) || ""))) continue;
          let target = el;
          for (;;) {
            const r = target.getRootNode();
            if (r instanceof ShadowRoot && /button/i.test(r.host.localName)) target = r.host;
            else break;
          }
          target.style.setProperty("display", "none", "important");
        }
      };
      hide(document);
    }

    if (!st.emu && !st.emuTried) {
      st.emuTried = true;
      import("./models/emulation/emulation.js").then(m => { st.emu = m; }).catch(() => {});
    }

    const advanced = window.Emulation && window.Emulation.AdvancedApp;
    const app = advanced && advanced.instance();
    const split = app && app.rootSplitWidget;
    if (!split) return wait();

    const sides = ["right", "bottom", "left", "undocked"];
    const dockSide = () => sides.find(s => document.body.classList.contains(s)) || "none";
    if (dockSide() === "undocked" || dockSide() === "none") {
      if (!st.dock) {
        if (st.failed) return wait();
        if (!st.loading) {
          st.loading = true;
          import("./ui/legacy/legacy.js")
            .then(m => { st.dock = m.DockController.DockController.instance(); })
            .catch(() => { st.failed = true; });
        }
        return wait();
      }
      // The frontend only knows right, bottom and left; top is bottom with the sides swapped below.
      st.dock.setDockSide(cfg.position === "top" ? "bottom" : cfg.position);
      st.pos = null;
    }

    const wrapper = app.deviceModeView;
    let on = false;
    try { on = Boolean(wrapper && wrapper.isDeviceModeOn && wrapper.isDeviceModeOn()); } catch (e) {}
    if (!on) {
      try {
        const model = st.emu && st.emu.DeviceModeModel.DeviceModeModel.instance();
        on = Boolean(model && model.enabledSetting().get());
      } catch (e) {}
    }
    if (!on) {
      const bar = document.querySelector(".device-mode-toolbar");
      on = Boolean(bar && bar.offsetParent);
    }

    // Expand right away on a click that turns device mode on; drop it if nothing confirms in 1.5s.
    const wantFull = on || (!st.from && Date.now() - st.want < 1500);
    const full = Math.abs(window.innerWidth - cfg.width) <= 4 && Math.abs(window.innerHeight - cfg.height) <= 4;

    const changed = st.pos !== cfg.position || st.resp !== wantFull;
    const mode = split.showMode && split.showMode();
    if (changed || (mode !== undefined && mode !== (wantFull ? "Both" : "OnlySidebar"))) {
      split.setVertical(cfg.position === "left" || cfg.position === "right");
      split.setSecondIsSidebar(cfg.position === "right" || cfg.position === "bottom");
      if (wantFull) split.showBoth(); else split.hideMain();
    }
    st.pos = cfg.position;

    // The split clamps the sidebar while the WebView is still panel-sized, so re-apply the
    // panel size for a few ticks once the WebView has really grown. After that the user can
    // drag the frontend's own splitter freely.
    if (changed || !wantFull || !full) st.settle = 0;
    if (wantFull && full && st.settle < 6) {
      st.settle++;
      split.setSidebarSize(cfg.size);
    }
    st.resp = wantFull;
    st.lastOn = on;

    let bounds = null;
    const el = app.inspectedPagePlaceholder && app.inspectedPagePlaceholder.element;
    if (on && el && el.isConnected) {
      const r = el.getBoundingClientRect();
      bounds = [Math.round(r.left), Math.round(r.top), Math.round(r.width), Math.round(r.height)];
    }
    return { ready: true, responsive: wantFull, bounds, iw: window.innerWidth, ih: window.innerHeight };
  } catch (e) {
    return wait();
  }
})
"##;

/// What the DevTools frontend reports back to the native side.
#[derive(Deserialize, Default)]
struct DevToolsReport {
    #[serde(default)]
    ready: bool,
    #[serde(default)]
    responsive: bool,
    /// [x, y, w, h] of the emulated screen, in the frontend's own coordinates.
    #[serde(default)]
    bounds: Option<[f64; 4]>,
    #[serde(default)]
    iw: f64,
    #[serde(default)]
    ih: f64,
}

pub struct Tab {
    pub id: String,
    pub profile_id: String,
    pub space_id: Option<String>, // None for favorites
    pub folder_id: Option<String>,
    pub kind: Kind,
    pub home: String, // URL a pinned/favorite resets to when closed
    pub url: String,
    pub host: String, // last host seen (for visit stats)
    pub title: String,
    pub favicon: Option<String>,
    pub webview: Option<WebView>, // None = not loaded yet (lazy) or put to sleep
    pub last_active: SystemTime,
}

#[derive(Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum Cmd {
    Ready,
    SidebarCollapsed {
        collapsed: bool,
    },
    SidebarCompact {
        compact: bool,
    },
    SidebarWidth {
        width: f64,
        #[serde(default)]
        persist: bool,
    },
    WindowMinimize,
    WindowToggleMaximize,
    WindowDrag,
    WindowClose,
    /// The sidebar is showing a full-area view (new tab bar, dialogs): hide/show the page.
    Overlay {
        open: bool,
    },
    NewTab {
        input: String,
    },
    Navigate {
        input: String,
    },
    SelectTab {
        id: String,
    },
    CloseTab {
        id: String,
    },
    PinTab {
        id: String,
    },
    UnpinTab {
        id: String,
    },
    FavTab {
        id: String,
    },
    UnfavTab {
        id: String,
    },
    SwitchSpace {
        id: String,
    },
    CycleSpace {
        dir: i32,
    },
    AddSpace {
        name: String,
        icon: String,
        color: String,
        #[serde(default)]
        profile_id: Option<String>,
        #[serde(default)]
        new_profile: Option<String>,
    },
    EditSpace {
        id: String,
        name: String,
        icon: String,
        color: String,
    },
    /// Drag & drop: put a tab into the pinned list, the tab list or the favorites at `index`.
    MoveTab {
        id: String,
        to: String,
        index: usize,
    },
    CreateFolder {
        name: String,
    },
    RenameFolder {
        id: String,
        name: String,
    },
    DeleteFolder {
        id: String,
    },
    ToggleFolderCollapsed {
        id: String,
    },
    MoveFolder {
        id: String,
        to: String,
        index: usize,
    },
    MoveTabToFolder {
        id: String,
        folder_id: Option<String>,
    },
    SetTheme {
        theme: String,
    },
    SetRestoreTabs {
        on: bool,
    },
    SetAutoPip {
        on: bool,
    },
    UpdateProfileCleanup {
        tab_retention_hours: u64,
        cookie_retention_hours: u64,
        cache_retention_hours: u64,
        history_retention_hours: u64,
    },
    DeleteSpace {
        id: String,
    },
    ListExt,
    InstallExt {
        input: String,
    },
    RemoveExt {
        dir: String,
    },
    Back,
    Forward,
    Reload,
    DevToolsToggle,
    DevToolsPosition {
        position: String,
    },
    DevToolsSize {
        size: f64,
        #[serde(default)]
        persist: bool,
    },
}

#[derive(Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
enum PageMsg {
    Meta {
        u: Option<String>,
    },
    Favicon {
        f: Option<String>,
    },
    Key {
        k: String,
    },
    Fullscreen {
        active: bool,
    },
    /// Internal (empty tab id only): periodic DevTools poll trigger.
    #[serde(rename = "devtools_tick")]
    DevToolsTick,
    /// Internal (empty tab id only): result of a DevTools poll.
    #[serde(rename = "devtools_state")]
    DevToolsState {
        s: Option<DevToolsReport>,
    },
}

/// Where everything goes for the current window / DevTools state.
struct Layout {
    page: Rect,
    page_visible: bool,
    tools: Option<Rect>,
    /// [x, y, w, h] of the strip with the DevTools title and close button.
    header: Option<[f64; 4]>,
    /// [x, y, w, h] of the resize handle (docked mode only).
    handle: Option<[f64; 4]>,
}

pub struct Browser {
    window: Window,
    proxy: EventLoopProxy<UserEvent>,
    root: PathBuf,
    sidebar: WebView,
    ui_ctx: WebContext,
    /// One WebContext (= one user-data folder = one browser profile) per profile.
    contexts: HashMap<String, WebContext>,
    ext_done: HashSet<String>,
    p: Persisted,
    tabs: Vec<Tab>,
    /// Last active tab per space.
    active: HashMap<String, String>,
    visible: Option<String>,
    overlay: bool,
    fullscreen: bool,
    closed_tabs: Vec<String>,
    devtools_open: bool,
    devtools: Option<WebView>,
    devtools_tab: Option<String>,
    /// DevTools device mode is on: the frontend covers the whole content area and the page
    /// WebView is placed over the emulated screen.
    responsive_mode: bool,
    responsive_bounds: Option<[f64; 4]>,
    /// The poll thread only ticks while this is set (and is parked otherwise).
    devtools_polling: Arc<AtomicBool>,
    poll_thread: std::thread::Thread,
    /// Start time (ms) of the poll in flight, 0 when idle.
    poll_busy: Arc<AtomicU64>,
    chrome_shown: Cell<bool>,
    /// Hash of the last state pushed to the sidebar; identical states are not re-sent.
    last_state_hash: Cell<u64>,
    /// Cached new-tab suggestions, rebuilt only when visit stats change.
    sugg_cache: RefCell<Option<Vec<Value>>>,
    save_dirty: bool,
    last_save: Instant,
    /// Recent crashes per tab (window start, count), to avoid reload loops.
    crashes: HashMap<String, (Instant, u32)>,
}

fn logical(w: &Window) -> (f64, f64) {
    let s: tao::dpi::LogicalSize<f64> = w.inner_size().to_logical(w.scale_factor());
    (s.width, s.height)
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect {
        position: LogicalPosition::new(x, y).into(),
        size: LogicalSize::new(w.max(1.0), h.max(1.0)).into(),
    }
}

fn rect_of(b: [f64; 4]) -> Rect {
    rect(b[0], b[1], b[2], b[3])
}

fn full_rect(w: &Window) -> Rect {
    let (ww, hh) = logical(w);
    rect(0.0, 0.0, ww, hh)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(1)
        .max(1)
}

pub fn normalize(input: &str) -> String {
    let s = input.trim();
    if s.is_empty() {
        return "about:blank".into();
    }
    if let Ok(u) = url::Url::parse(s) {
        if matches!(u.scheme(), "http" | "https") || s == "about:blank" {
            return s.to_string();
        }
    }
    if !s.contains(char::is_whitespace) {
        if s.starts_with("localhost") || s.starts_with("127.0.0.1") {
            return format!("http://{s}");
        }
        if s.contains('.') {
            return format!("https://{s}");
        }
    }
    let q: String = url::form_urlencoded::byte_serialize(s.as_bytes()).collect();
    format!("https://duckduckgo.com/?q={q}")
}

fn ok_url(s: &str, max: usize, allow_data: bool) -> bool {
    s.len() <= max
        && (s.starts_with("http://")
            || s.starts_with("https://")
            || (allow_data && s.starts_with("data:image/")))
}

/// Keeps the last usable icon when WebView2 reports a transient empty result.
fn update_favicon(current: &mut Option<String>, incoming: Option<String>) -> bool {
    let Some(favicon) = incoming.filter(|value| ok_url(value, FAVICON_MAX, true)) else {
        return false;
    };
    if current.as_deref() == Some(favicon.as_str()) {
        return false;
    }
    *current = Some(favicon);
    true
}

fn ok_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Maps WebView2 accelerator keys (Win32 VK_* values) to browser commands.
fn browser_shortcut(key: u32, ctrl: bool, alt: bool, shift: bool) -> Option<&'static str> {
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
        0x31 => Some("tab_1"),
        0x32 => Some("tab_2"),
        0x33 => Some("tab_3"),
        0x34 => Some("tab_4"),
        0x35 => Some("tab_5"),
        0x36 => Some("tab_6"),
        0x37 => Some("tab_7"),
        0x38 => Some("tab_8"),
        0x39 => Some("tab_last"),
        VK_TAB => Some("tab_next"),
        _ => None,
    }
}

fn clean_icon(s: &str) -> String {
    if valid_icon(s) {
        s.to_string()
    } else {
        "house".into()
    }
}

fn clip(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

#[cfg(windows)]
fn devtools_target_id(webview: &WebView) -> Result<String> {
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

// Extra disabled features (hints/model downloads, media router, translate) trim background
// work and memory. There must be exactly one --disable-features switch (the last one wins).
#[cfg(windows)]
const WEBVIEW2_BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,OptimizationHints,OptimizationGuideModelDownloading,MediaRouter,Translate --disable-blink-features=AutomationControlled --remote-debugging-address=127.0.0.1 --remote-debugging-port=0 --remote-allow-origins=devtools://devtools";

#[cfg(windows)]
fn devtools_frontend_url(port: u16, target_id: &str) -> Result<String> {
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
fn resolve_devtools_frontend(profile_dir: &Path, target_id: &str) -> Result<String> {
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

#[cfg(windows)]
fn register_browser_shortcuts(webview: &WebView, proxy: EventLoopProxy<UserEvent>, tab: String) {
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

impl Browser {
    pub fn new(window: Window, proxy: EventLoopProxy<UserEvent>) -> Result<Self> {
        let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
        let root = base.join("Poof");
        // The browser used to be called ArcRS: carry the user's tabs, logins and extensions over.
        let old = base.join("ArcRS");
        if old.is_dir() && !root.exists() {
            let _ = fs::rename(&old, &root);
        }
        fs::create_dir_all(root.join("extensions"))?;
        let mut p = Persisted::load(&root.join("state.json"));
        if !matches!(
            p.devtools_position.as_str(),
            "right" | "left" | "bottom" | "top"
        ) {
            p.devtools_position = "right".into();
        }
        if !p.devtools_size.is_finite() || p.devtools_size < PANEL_MIN {
            p.devtools_size = PANEL_DEFAULT;
        }

        // The sidebar is our own trusted UI (HTML), living in its own data folder.
        let html = include_str!("../ui/sidebar.html")
            .replace(
                "/*STYLES*/",
                &format!(
                    "<style>{}</style>",
                    [
                        include_str!("../ui/theme.css"),
                        include_str!("../ui/styles/base.css"),
                        include_str!("../ui/styles/components.css"),
                        include_str!("../ui/styles/topbar.css"),
                        include_str!("../ui/styles/sidebar.css"),
                        include_str!("../ui/styles/stage.css"),
                        include_str!("../ui/styles/start-page.css")
                    ]
                    .concat()
                ),
            )
            .replace("/*ICONS*/", include_str!("../ui/icons.js"))
            .replace("/*CORE*/", include_str!("../ui/scripts/core.js"))
            .replace("/*SIDEBAR*/", include_str!("../ui/scripts/sidebar.js"))
            .replace(
                "/*START_PAGE*/",
                include_str!("../ui/scripts/start-page.js"),
            )
            .replace("/*DIALOGS*/", include_str!("../ui/scripts/dialogs.js"))
            .replace("/*SETTINGS*/", include_str!("../ui/scripts/settings.js"));
        let mut ui_ctx = WebContext::new(Some(root.join("ui")));
        let ptx = proxy.clone();
        let sidebar = WebViewBuilder::new_with_web_context(&mut ui_ctx)
            .with_bounds(full_rect(&window))
            .with_devtools(false)
            .with_html(html)
            .with_ipc_handler(move |req| {
                let _ = ptx.send_event(UserEvent::Ui(req.into_body()));
            })
            .build_as_child(&window)?;

        // The sidebar is a separate WebView, so it needs its own native
        // accelerator handler when it owns keyboard focus.
        #[cfg(windows)]
        register_browser_shortcuts(&sidebar, proxy.clone(), String::new());

        let tabs = p
            .saved
            .iter()
            .map(|s| Tab {
                id: s.id.clone(),
                profile_id: s.profile_id.clone(),
                space_id: s.space_id.clone(),
                folder_id: s.folder_id.clone(),
                kind: s.kind,
                home: s.url.clone(),
                url: s.url.clone(),
                host: String::new(),
                title: s.title.clone(),
                favicon: s.favicon.clone(),
                webview: None,
                last_active: SystemTime::now(),
            })
            .collect();

        // Only ticks while DevTools is open (see `relayout`); parked (zero wakeups) otherwise.
        let devtools_polling = Arc::new(AtomicBool::new(false));
        let poll_thread = {
            let flag = Arc::clone(&devtools_polling);
            let tick = proxy.clone();
            let handle = std::thread::spawn(move || loop {
                while !flag.load(Ordering::Relaxed) {
                    std::thread::park();
                }
                std::thread::sleep(Duration::from_millis(40));
                let ev = UserEvent::Page {
                    tab: String::new(),
                    body: r#"{"t":"devtools_tick"}"#.to_string(),
                };
                if tick.send_event(ev).is_err() {
                    break;
                }
            });
            handle.thread().clone()
        };

        let initial_active = if p.restore_tabs {
            p.active_tabs.clone()
        } else {
            HashMap::new()
        };

        let mut b = Self {
            window,
            proxy,
            root,
            sidebar,
            ui_ctx,
            contexts: HashMap::new(),
            ext_done: HashSet::new(),
            p,
            tabs,
            active: initial_active,
            visible: None,
            overlay: false,
            fullscreen: false,
            closed_tabs: Vec::new(),
            devtools_open: false,
            devtools: None,
            devtools_tab: None,
            responsive_mode: false,
            responsive_bounds: None,
            devtools_polling,
            poll_thread,
            poll_busy: Arc::new(AtomicU64::new(0)),
            chrome_shown: Cell::new(false),
            last_state_hash: Cell::new(0),
            sugg_cache: RefCell::new(None),
            save_dirty: false,
            last_save: Instant::now() - SAVE_MIN_INTERVAL,
            crashes: HashMap::new(),
        };
        b.apply_theme();
        b.preinstall();
        Ok(b)
    }

    /// Colors the native window frame; the UI itself follows `theme` from the pushed state.
    fn apply_theme(&self) {
        let t = match self.p.theme.as_str() {
            "light" => Some(Theme::Light),
            "dark" => Some(Theme::Dark),
            _ => None,
        };
        self.window.set_theme(t);
    }

    fn preinstall(&mut self) {
        for id in store::PREINSTALL {
            let id = id.to_string();
            if self.p.preinstalled.contains(&id) {
                continue;
            }
            if self
                .root
                .join("extensions")
                .join(&id)
                .join("manifest.json")
                .is_file()
            {
                self.p.preinstalled.push(id);
            } else {
                self.spawn_install(id);
            }
        }
    }

    fn spawn_install(&self, id: String) {
        let root = self.root.clone();
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let ev = match store::install(&root, &id) {
                Ok((dir, name)) => UserEvent::ExtResult {
                    ok: true,
                    msg: format!("Installed {name}"),
                    dir: Some(dir),
                    id: Some(id),
                },
                Err(e) => UserEvent::ExtResult {
                    ok: false,
                    msg: format!("Couldn’t install: {e}"),
                    dir: None,
                    id: Some(id),
                },
            };
            let _ = proxy.send_event(ev);
        });
    }

    fn toast(&self, msg: &str) {
        let m = serde_json::to_string(msg).unwrap_or_default();
        let _ = self
            .sidebar
            .evaluate_script(&format!("window.__toast&&window.__toast({m})"));
    }

    fn push_exts(&self) {
        let list = serde_json::to_string(&store::list(&self.root)).unwrap_or_else(|_| "[]".into());
        let _ = self
            .sidebar
            .evaluate_script(&format!("window.__setExts&&window.__setExts({list})"));
    }

    pub fn on_user_event(&mut self, ev: UserEvent) {
        match ev {
            UserEvent::CloseWindow => {}
            UserEvent::DevToolsReady { tab, url } => self.show_devtools(&tab, &url),
            UserEvent::DevToolsFailed { tab, message } => {
                if self.devtools_open && self.visible.as_deref() == Some(tab.as_str()) {
                    self.close_devtools();
                    self.toast(&message);
                }
            }
            UserEvent::TabCrashed { tab, browser } => {
                self.recover_tab(&tab, browser);
                self.push_state();
            }
            UserEvent::Ui(raw) => self.handle_ui(&raw),
            UserEvent::Page { tab, body } => self.handle_page(&tab, &body),
            UserEvent::Fullscreen { tab, active } => self.handle_fullscreen(&tab, active),
            UserEvent::Title { tab, title } => {
                if let Some(t) = self.tabs.iter_mut().find(|t| t.id == tab) {
                    t.title = clip(&title, 300);
                }
                self.push_state();
            }
            UserEvent::OpenTab { url } => {
                if ok_url(&url, 4096, false) {
                    if let Err(e) = self.new_tab(&url) {
                        log::error!("{e:#}");
                    }
                    self.push_state();
                }
            }
            UserEvent::ExtResult { ok, msg, dir, id } => {
                self.toast(&msg);
                if ok {
                    if let Some(id) = id {
                        if !self.p.preinstalled.contains(&id) {
                            self.p.preinstalled.push(id);
                        }
                    }
                    if let Some(dir) = dir {
                        // Hot-load into every profile that already has a live webview;
                        // the rest pick it up when their first webview is created.
                        let loaded: HashSet<String> = self
                            .tabs
                            .iter()
                            .filter(|t| t.webview.is_some())
                            .map(|t| t.profile_id.clone())
                            .collect();
                        for pid in &loaded {
                            if let Some(wv) = self
                                .tabs
                                .iter()
                                .find(|t| &t.profile_id == pid && t.webview.is_some())
                                .and_then(|t| t.webview.as_ref())
                            {
                                extensions::add(wv, &dir);
                            }
                        }
                        self.ext_done.retain(|p| loaded.contains(p));
                    }
                    self.save();
                    self.push_exts();
                }
            }
        }
    }

    fn handle_ui(&mut self, raw: &str) {
        match serde_json::from_str::<Cmd>(raw) {
            Ok(cmd) => {
                if let Err(e) = self.run(cmd) {
                    log::error!("{e:#}");
                }
            }
            Err(e) => log::warn!("bad ui message: {e}"),
        }
        self.push_state();
    }

    /// Switches the whole native window, not just the embedded WebView, into fullscreen.
    fn handle_fullscreen(&mut self, tab: &str, active: bool) {
        if self.visible.as_deref() != Some(tab) || self.fullscreen == active {
            return;
        }
        self.fullscreen = active;
        self.window.set_fullscreen(if active {
            Some(tao::window::Fullscreen::Borderless(None))
        } else {
            None
        });
        let _ = self.sidebar.set_visible(!active);
        self.relayout();
    }

    /// Messages from web pages are untrusted: strict parsing, size limits, own-tab only.
    fn handle_page(&mut self, tab: &str, body: &str) {
        if body.len() > 700_000 {
            return;
        }
        let Ok(msg) = serde_json::from_str::<PageMsg>(body) else {
            return;
        };
        match msg {
            // Internal messages only come from native code (empty tab id). A page always has
            // its own non-empty id baked in, so it cannot forge them.
            PageMsg::DevToolsTick => {
                if tab.is_empty() {
                    self.poll_devtools();
                }
                return;
            }
            PageMsg::DevToolsState { s } => {
                if tab.is_empty() {
                    if let Some(report) = s {
                        self.apply_devtools_report(report);
                    }
                }
                return;
            }
            PageMsg::Meta { u } => {
                let mut persist = false;
                let mut sites_dirty = false;
                let restore = self.p.restore_tabs;
                if let Some(t) = self.tabs.iter_mut().find(|t| t.id == tab) {
                    let mut url_changed = false;
                    if let Some(u) = u.filter(|u| ok_url(u, 4096, false)) {
                        if t.url != u {
                            url_changed = true;
                        }
                        if let Ok(parsed) = url::Url::parse(&u) {
                            if let Some(h) = parsed.host_str() {
                                let host = h.trim_start_matches("www.").to_string();
                                if t.host != host {
                                    t.favicon = None;
                                    t.host = host.clone();
                                    let s = self.p.sites.entry(host).or_default();
                                    s.count += 1;
                                    s.url = format!("{}://{}/", parsed.scheme(), h);
                                    sites_dirty = true;
                                }
                            }
                        }
                        t.url = u;
                    }
                    if let Some(fv) = &t.favicon {
                        if let Some(s) = self.p.sites.get_mut(&t.host) {
                            if s.favicon.as_ref() != Some(fv) {
                                s.favicon = Some(fv.clone());
                                sites_dirty = true;
                            }
                        }
                    }
                    persist = url_changed && (t.kind != Kind::Normal || restore);
                }
                self.p.prune_sites();
                if sites_dirty {
                    self.sugg_cache.replace(None);
                }
                if persist {
                    self.save();
                }
            }
            PageMsg::Favicon { f } => {
                let mut persist = false;
                let mut sites_dirty = false;
                let restore = self.p.restore_tabs;
                if let Some(t) = self.tabs.iter_mut().find(|t| t.id == tab) {
                    if update_favicon(&mut t.favicon, f) {
                        if let Some(s) = self.p.sites.get_mut(&t.host) {
                            s.favicon = t.favicon.clone();
                            sites_dirty = true;
                        }
                        persist = t.kind != Kind::Normal || restore;
                    }
                }
                if sites_dirty {
                    self.sugg_cache.replace(None);
                }
                if persist {
                    self.save();
                }
            }
            PageMsg::Fullscreen { active } => self.handle_fullscreen(tab, active),
            PageMsg::Key { k } => {
                // An empty tab ID means the shortcut originated in the sidebar.
                let active_tab = self.visible.clone().unwrap_or_default();
                let tab = if tab.is_empty() {
                    active_tab.as_str()
                } else {
                    tab
                };
                if self.visible.as_deref() != Some(tab) {
                    return;
                }
                let r = match k.as_str() {
                    "new_tab" => self.new_tab("about:blank"),
                    "focus_url" => {
                        let _ = self.sidebar.focus();
                        let _ = self
                            .sidebar
                            .evaluate_script("window.__focusUrl && window.__focusUrl()");
                        Ok(())
                    }
                    "close" => self.close_tab(tab),
                    "reopen" => self.reopen_closed_tab(),
                    "back" => {
                        self.eval_visible("history.back()");
                        Ok(())
                    }
                    "forward" => {
                        self.eval_visible("history.forward()");
                        Ok(())
                    }
                    "reload" => {
                        self.eval_visible("location.reload()");
                        Ok(())
                    }
                    "space_next" => self.cycle_space(1),
                    "space_prev" => self.cycle_space(-1),
                    "tab_next" => self.cycle_tab(1),
                    "tab_prev" => self.cycle_tab(-1),
                    "tab_1" | "tab_2" | "tab_3" | "tab_4" | "tab_5" | "tab_6" | "tab_7"
                    | "tab_8" | "tab_last" => {
                        let index = if k == "tab_last" {
                            usize::MAX
                        } else {
                            k[4..].parse::<usize>().unwrap_or(1).saturating_sub(1)
                        };
                        self.activate_tab_index(index)
                    }
                    "print" => {
                        self.eval_visible("window.print()");
                        Ok(())
                    }
                    "favorite" => {
                        self.set_kind(tab, Kind::Favorite);
                        Ok(())
                    }
                    "devtools_toggle" => self.toggle_devtools(),
                    _ => Ok(()),
                };
                if let Err(e) = r {
                    log::error!("{e:#}");
                }
            }
        }
        // The sidebar is a separate WebView and only sees tab changes through pushed state.
        // (Identical states are dropped inside push_state.)
        self.push_state();
    }

    fn run(&mut self, cmd: Cmd) -> Result<()> {
        match cmd {
            Cmd::Ready => {
                // The sidebar just (re)loaded and has no state yet: force a full push.
                self.last_state_hash.set(0);
                self.restore_space_tab()?
            }
            Cmd::SidebarCollapsed { collapsed } => {
                self.p.sidebar_collapsed = collapsed;
                self.save();
                self.relayout();
            }
            Cmd::SidebarCompact { compact } => {
                self.p.sidebar_compact = compact;
                if compact {
                    self.p.sidebar_collapsed = false;
                }
                self.save();
                self.relayout();
            }
            Cmd::SidebarWidth { width, persist } => {
                if width.is_finite() {
                    self.p.sidebar_width = width.clamp(SIDEBAR_MIN_W, SIDEBAR_MAX_W);
                    if persist {
                        self.save();
                    }
                    self.relayout();
                }
            }
            Cmd::WindowMinimize => self.window.set_minimized(true),
            Cmd::WindowToggleMaximize => self.window.set_maximized(!self.window.is_maximized()),
            Cmd::WindowDrag => {
                let _ = self.window.drag_window();
            }
            Cmd::WindowClose => {
                let _ = self.proxy.send_event(UserEvent::CloseWindow);
            }
            Cmd::Overlay { open } => {
                self.overlay = open;
                if open {
                    let _ = self.sidebar.focus();
                }
                self.relayout();
                if !open {
                    if let Some(wv) = self.visible_webview() {
                        let _ = wv.focus();
                    }
                }
            }
            Cmd::NewTab { input } => self.new_tab(&normalize(&input))?,
            Cmd::Navigate { input } => {
                let url = normalize(&input);
                if let Some(wv) = self.visible_webview() {
                    wv.load_url(&url)?;
                } else {
                    self.new_tab(&url)?;
                }
            }
            Cmd::SelectTab { id } => self.activate(&id)?,
            Cmd::CloseTab { id } => self.close_tab(&id)?,
            Cmd::PinTab { id } => self.set_kind(&id, Kind::Pinned),
            Cmd::UnpinTab { id } | Cmd::UnfavTab { id } => self.set_kind(&id, Kind::Normal),
            Cmd::FavTab { id } => self.set_kind(&id, Kind::Favorite),
            Cmd::SwitchSpace { id } => self.switch_space(&id)?,
            Cmd::CycleSpace { dir } => self.cycle_space(dir)?,
            Cmd::AddSpace {
                name,
                icon,
                color,
                profile_id,
                new_profile,
            } => {
                let pid = match new_profile
                    .map(|n| clip(n.trim(), 40))
                    .filter(|n| !n.is_empty())
                {
                    Some(n) => {
                        let id = new_id();
                        self.p.profiles.push(Profile {
                            id: id.clone(),
                            name: n,
                            tab_retention_hours: 48,
                            cookie_retention_hours: 0,
                            cache_retention_hours: 0,
                            history_retention_hours: 0,
                            last_cleanup_at: 0,
                        });
                        id
                    }
                    None => profile_id
                        .filter(|id| self.p.profiles.iter().any(|p| &p.id == id))
                        .unwrap_or_else(|| self.cur_space().profile_id.clone()),
                };
                let id = new_id();
                self.p.spaces.push(Space {
                    id: id.clone(),
                    profile_id: pid,
                    name: clip(name.trim(), 40),
                    icon: clean_icon(&icon),
                    color: if ok_color(&color) {
                        color
                    } else {
                        "#6dc5e2".into()
                    },
                });
                self.switch_space(&id)?;
            }
            Cmd::EditSpace {
                id,
                name,
                icon,
                color,
            } => {
                if let Some(s) = self.p.spaces.iter_mut().find(|s| s.id == id) {
                    s.name = clip(name.trim(), 40);
                    s.icon = clean_icon(&icon);
                    if ok_color(&color) {
                        s.color = color;
                    }
                }
                self.save();
            }
            Cmd::DeleteSpace { id } => self.delete_space(&id)?,
            Cmd::MoveTab { id, to, index } => self.move_tab(&id, &to, index),
            Cmd::CreateFolder { name } => {
                let name = clip(name.trim(), 40);
                if !name.is_empty() {
                    let space_id = self.cur_space().id.clone();
                    self.p.folders.push(TabFolder {
                        id: new_id(),
                        space_id,
                        name,
                        collapsed: false,
                        pinned: false,
                        order: usize::MAX,
                    });
                    self.save();
                }
            }
            Cmd::RenameFolder { id, name } => {
                let name = clip(name.trim(), 40);
                if !name.is_empty() {
                    if let Some(f) = self
                        .p
                        .folders
                        .iter_mut()
                        .find(|f| f.id == id && f.space_id == self.p.current_space)
                    {
                        f.name = name;
                        self.save();
                    }
                }
            }
            Cmd::DeleteFolder { id } => {
                if self
                    .p
                    .folders
                    .iter()
                    .any(|f| f.id == id && f.space_id == self.p.current_space)
                {
                    self.p.folders.retain(|f| f.id != id);
                    for t in self
                        .tabs
                        .iter_mut()
                        .filter(|t| t.folder_id.as_deref() == Some(id.as_str()))
                    {
                        t.folder_id = None;
                    }
                    self.save();
                }
            }
            Cmd::ToggleFolderCollapsed { id } => {
                if let Some(f) = self
                    .p
                    .folders
                    .iter_mut()
                    .find(|f| f.id == id && f.space_id == self.p.current_space)
                {
                    f.collapsed = !f.collapsed;
                    self.save();
                }
            }
            Cmd::MoveFolder { id, to, index } => {
                let space = self.p.current_space.clone();
                let pinned = to == "pinned";
                if pinned || to == "tab" {
                    if let Some(from) = self
                        .p
                        .folders
                        .iter()
                        .position(|f| f.id == id && f.space_id == space)
                    {
                        let mut folder = self.p.folders.remove(from);
                        let old_order = folder.order;
                        let old_pinned = folder.pinned;
                        // Close the gap in the source section, then make room at the exact
                        // mixed tab/folder position selected by the pointer.
                        for peer in self.p.folders.iter_mut().filter(|f| {
                            f.space_id == space && f.pinned == old_pinned && f.order > old_order
                        }) {
                            peer.order = peer.order.saturating_sub(1);
                        }
                        for peer in self.p.folders.iter_mut().filter(|f| {
                            f.space_id == space && f.pinned == pinned && f.order >= index
                        }) {
                            peer.order = peer.order.saturating_add(1);
                        }
                        folder.pinned = pinned;
                        folder.order = index;
                        self.p.folders.push(folder);
                        let kind = if pinned { Kind::Pinned } else { Kind::Normal };
                        for tab in self.tabs.iter_mut().filter(|t| {
                            t.folder_id.as_deref() == Some(id.as_str())
                                && t.space_id.as_deref() == Some(space.as_str())
                        }) {
                            if tab.kind != kind {
                                tab.home = tab.url.clone();
                                tab.kind = kind;
                            }
                        }
                        self.save();
                    }
                }
            }
            Cmd::MoveTabToFolder { id, folder_id } => {
                let target_pinned = folder_id
                    .as_ref()
                    .and_then(|fid| {
                        self.p
                            .folders
                            .iter()
                            .find(|f| f.id == *fid && f.space_id == self.p.current_space)
                    })
                    .map(|f| f.pinned);
                if folder_id.is_none() || target_pinned.is_some() {
                    if let Some(t) = self.tabs.iter_mut().find(|t| {
                        t.id == id && t.space_id.as_deref() == Some(self.p.current_space.as_str())
                    }) {
                        if let Some(pinned) = target_pinned {
                            let kind = if pinned { Kind::Pinned } else { Kind::Normal };
                            if t.kind != kind {
                                t.home = t.url.clone();
                                t.kind = kind;
                            }
                        }
                        t.folder_id = folder_id;
                        self.save();
                    }
                }
            }
            Cmd::SetTheme { theme } => {
                if matches!(theme.as_str(), "system" | "light" | "dark") {
                    self.p.theme = theme;
                    self.apply_theme();
                    self.save();
                }
            }
            Cmd::SetRestoreTabs { on } => {
                self.p.restore_tabs = on;
                self.save();
            }
            Cmd::SetAutoPip { on } => {
                self.p.auto_pip = on;
                self.save();
            }
            Cmd::UpdateProfileCleanup {
                tab_retention_hours,
                cookie_retention_hours,
                cache_retention_hours,
                history_retention_hours,
            } => {
                let valid = |v: u64, allow_off: bool| {
                    (allow_off && v == 0) || matches!(v, 24 | 48 | 72 | 168 | 336)
                };
                let profile_id = self.cur_space().profile_id.clone();
                if let Some(profile) = self.p.profiles.iter_mut().find(|p| p.id == profile_id) {
                    if valid(tab_retention_hours, false) {
                        profile.tab_retention_hours = tab_retention_hours;
                    }
                    if valid(cookie_retention_hours, true) {
                        profile.cookie_retention_hours = cookie_retention_hours;
                    }
                    if valid(cache_retention_hours, true) {
                        profile.cache_retention_hours = cache_retention_hours;
                    }
                    if valid(history_retention_hours, true) {
                        profile.history_retention_hours = history_retention_hours;
                    }
                    profile.last_cleanup_at = 0;
                }
                self.save();
            }
            Cmd::ListExt => self.push_exts(),
            Cmd::InstallExt { input } => match store::parse_id(&input) {
                Some(id) => {
                    self.toast("Installing extension…");
                    self.spawn_install(id);
                }
                None => self.toast("That doesn’t look like a Web Store link or extension ID."),
            },
            Cmd::RemoveExt { dir } => match store::remove(&self.root, &dir) {
                Ok(()) => {
                    self.toast("Removed. Restart the browser to finish unloading it.");
                    self.push_exts();
                }
                Err(e) => self.toast(&format!("Couldn’t remove: {e}")),
            },
            Cmd::Back => self.eval_visible("history.back()"),
            Cmd::Forward => self.eval_visible("history.forward()"),
            Cmd::Reload => self.eval_visible("location.reload()"),
            Cmd::DevToolsToggle => self.toggle_devtools()?,
            Cmd::DevToolsPosition { position } => {
                if matches!(position.as_str(), "right" | "left" | "bottom" | "top") {
                    self.p.devtools_position = position;
                    self.p.devtools_size = self.devtools_size();
                    self.save();
                    self.relayout();
                }
            }
            Cmd::DevToolsSize { size, persist } => {
                if size.is_finite() {
                    self.p.devtools_size = size.clamp(PANEL_MIN, self.devtools_limit());
                    if persist {
                        self.save();
                    }
                    self.relayout();
                }
            }
        }
        Ok(())
    }

    fn toggle_devtools(&mut self) -> Result<()> {
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
    fn close_devtools(&mut self) {
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

    fn attach_devtools(&mut self) -> Result<()> {
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

    fn open_devtools_view(&mut self, tab: &str, url: &str) -> Result<()> {
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

    fn show_devtools(&mut self, tab: &str, url: &str) {
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

    fn devtools_sync_call(&self) -> String {
        let [_, _, width, height] = self.responsive_tools_box();
        let cfg = json!({
            "position": self.p.devtools_position,
            "size": self.devtools_size(),
            "width": width.round(),
            "height": height.round(),
        });
        format!("({DEVTOOLS_SYNC_JS})({cfg})")
    }

    /// Asks the frontend for its state and enforces the layout.
    fn poll_devtools(&self) {
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

    fn apply_devtools_report(&mut self, report: DevToolsReport) {
        if !self.devtools_open
            || self.fullscreen
            || self.devtools_tab.is_none()
            || self.devtools_tab != self.visible
        {
            return;
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

    fn cur_space(&self) -> &Space {
        self.p
            .spaces
            .iter()
            .find(|s| s.id == self.p.current_space)
            .unwrap_or(&self.p.spaces[0])
    }

    fn visible_webview(&self) -> Option<&WebView> {
        let id = self.visible.as_ref()?;
        self.tabs.iter().find(|t| &t.id == id)?.webview.as_ref()
    }

    fn eval_visible(&self, js: &str) {
        if let Some(wv) = self.visible_webview() {
            let _ = wv.evaluate_script(js);
        }
    }

    fn set_kind(&mut self, id: &str, kind: Kind) {
        let cur = self.p.current_space.clone();
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            t.kind = kind;
            t.space_id = if kind == Kind::Favorite {
                None
            } else {
                Some(t.space_id.clone().unwrap_or(cur))
            };
            t.home = t.url.clone();
        }
        self.save();
    }

    /// Drops a tab into a list. `to` is "pinned", "tab" or "fav"; `index` is its position in that list.
    fn move_tab(&mut self, id: &str, to: &str, index: usize) {
        let sp = self.cur_space().clone();
        let Some(from) = self.tabs.iter().position(|t| t.id == id) else {
            return;
        };
        let mut t = self.tabs.remove(from);
        let kind = match to {
            "pinned" => Kind::Pinned,
            "fav" => Kind::Favorite,
            _ => Kind::Normal,
        };
        t.folder_id = None;
        if t.kind != kind {
            t.home = t.url.clone();
        }
        t.kind = kind;
        t.space_id = if kind == Kind::Favorite {
            None
        } else {
            Some(sp.id.clone())
        };
        let profile = t.profile_id.clone();
        let peers: Vec<usize> = self
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, x)| {
                x.kind == kind
                    && if kind == Kind::Favorite {
                        x.profile_id == profile
                    } else {
                        x.space_id.as_deref() == Some(sp.id.as_str())
                            && (kind != Kind::Pinned || x.folder_id.is_none())
                    }
            })
            .map(|(i, _)| i)
            .collect();
        let pos = match peers.get(index) {
            Some(&i) => i,
            None => peers.last().map(|i| i + 1).unwrap_or(self.tabs.len()),
        };
        self.tabs.insert(pos, t);
        self.save();
    }

    fn new_tab(&mut self, url: &str) -> Result<()> {
        let sp = self.cur_space().clone();
        let id = new_id();
        self.tabs.push(Tab {
            id: id.clone(),
            profile_id: sp.profile_id,
            space_id: Some(sp.id),
            kind: Kind::Normal,
            folder_id: None,
            home: url.to_string(),
            url: url.to_string(),
            host: String::new(),
            title: String::new(),
            favicon: None,
            webview: None,
            last_active: SystemTime::now(),
        });
        self.activate(&id)
    }

    fn ensure_webview(&mut self, idx: usize) -> Result<()> {
        if self.tabs[idx].webview.is_some() {
            return Ok(());
        }
        let id = self.tabs[idx].id.clone();
        let profile_id = self.tabs[idx].profile_id.clone();
        let url = self.tabs[idx].url.clone();
        let bounds = self.content_rect();
        let dir = self.root.join("profiles").join(&profile_id);
        let ctx = self
            .contexts
            .entry(profile_id.clone())
            .or_insert_with(|| WebContext::new(Some(dir)));

        let (p1, p2, p3) = (self.proxy.clone(), self.proxy.clone(), self.proxy.clone());
        let (id1, id2) = (id.clone(), id.clone());

        let builder = WebViewBuilder::new_with_web_context(ctx)
            .with_bounds(bounds)
            .with_visible(false)
            .with_devtools(false)
            .with_url(&url)
            .with_initialization_script_for_main_only(INIT_JS, false)
            .with_ipc_handler(move |req| {
                let _ = p1.send_event(UserEvent::Page {
                    tab: id1.clone(),
                    body: req.into_body(),
                });
            })
            .with_document_title_changed_handler(move |title| {
                let _ = p2.send_event(UserEvent::Title {
                    tab: id2.clone(),
                    title,
                });
            })
            // target=_blank / window.open opens a new tab (OAuth popups need more work).
            .with_new_window_req_handler(move |url, features| {
                let is_popup = features.size.is_some() || features.position.is_some();
                let is_blank = url == "about:blank";
                let is_auth_url = url.starts_with("https://accounts.google.com/")
                    || url.starts_with("https://appleid.apple.com/")
                    || url.starts_with("https://login.microsoftonline.com/")
                    || url.starts_with("https://github.com/login/")
                    || url.contains("/oauth")
                    || url.contains("/authorize")
                    || url.contains("/sso")
                    || url.contains("/signin")
                    || url.contains("/login");
                if is_popup || is_blank || is_auth_url {
                    NewWindowResponse::Allow
                } else {
                    let _ = p3.send_event(UserEvent::OpenTab { url });
                    NewWindowResponse::Deny
                }
            });

        #[cfg(windows)]
        let builder = {
            use wry::WebViewBuilderExtWindows;
            builder
                .with_browser_extensions_enabled(true)
                // Enables a random loopback CDP port for this isolated profile; the port is
                // read from DevToolsActivePort.
                .with_additional_browser_args(WEBVIEW2_BROWSER_ARGS)
        };

        let wv = builder.build_as_child(&self.window)?;

        #[cfg(windows)]
        register_browser_shortcuts(&wv, self.proxy.clone(), id.clone());

        // WebView2's original favicon URI lets the sidebar render SVGs and multi-resolution
        // ICO/PNG sources without rasterizing every icon to a small PNG snapshot first.
        #[cfg(windows)]
        {
            use webview2_com::{FaviconChangedEventHandler, Microsoft::Web::WebView2::Win32::*};
            use windows::core::{Interface, PWSTR};
            use windows::Win32::System::Com::CoTaskMemFree;

            fn current_favicon_uri(core15: &ICoreWebView2_15) -> Option<String> {
                unsafe {
                    let mut uri = PWSTR::null();
                    core15.FaviconUri(&mut uri).ok()?;
                    if uri.is_null() {
                        return None;
                    }
                    let value = uri.to_string().ok();
                    CoTaskMemFree(Some(uri.0 as *const std::ffi::c_void));
                    value.filter(|value| ok_url(value, 4096, false))
                }
            }

            let favicon_proxy = self.proxy.clone();
            let favicon_tab = id.clone();
            let initial_proxy = favicon_proxy.clone();
            let initial_tab = favicon_tab.clone();
            unsafe {
                if let Ok(core) = wv.controller().CoreWebView2() {
                    if let Ok(core15) = core.cast::<ICoreWebView2_15>() {
                        let handler =
                            FaviconChangedEventHandler::create(Box::new(move |sender, _| {
                                let Some(sender) = sender else { return Ok(()) };
                                let Ok(core15) = sender.cast::<ICoreWebView2_15>() else {
                                    return Ok(());
                                };
                                let favicon = current_favicon_uri(&core15);
                                let body = json!({"t": "favicon", "f": favicon}).to_string();
                                let _ = favicon_proxy.send_event(UserEvent::Page {
                                    tab: favicon_tab.clone(),
                                    body,
                                });
                                Ok(())
                            }));
                        let mut token = 0;
                        if let Err(error) = core15.add_FaviconChanged(&handler, &mut token) {
                            log::warn!("registering WebView2 favicon handler failed: {error}");
                        }
                        // The initial navigation may have started before the handler was attached.
                        let favicon = current_favicon_uri(&core15);
                        let body = json!({"t": "favicon", "f": favicon}).to_string();
                        let _ = initial_proxy.send_event(UserEvent::Page {
                            tab: initial_tab,
                            body,
                        });
                    } else {
                        log::warn!(
                            "WebView2 runtime lacks native favicon support (ICoreWebView2_15)"
                        );
                    }
                }
            }
        }

        // Reported by WebView2's native host, so fullscreen requests from cross-origin or
        // nested frames work without JavaScript IPC or postMessage relays.
        #[cfg(windows)]
        {
            use webview2_com::ContainsFullScreenElementChangedEventHandler;
            use windows::core::BOOL;

            let fullscreen_proxy = self.proxy.clone();
            let fullscreen_tab = id.clone();
            unsafe {
                match wv.controller().CoreWebView2() {
                    Ok(core) => {
                        let handler = ContainsFullScreenElementChangedEventHandler::create(
                            Box::new(move |sender, _| {
                                let Some(sender) = sender else { return Ok(()) };
                                let mut contains = BOOL::default();
                                if let Err(error) = sender.ContainsFullScreenElement(&mut contains)
                                {
                                    log::warn!("reading WebView2 fullscreen state failed: {error}");
                                    return Ok(());
                                }
                                let _ = fullscreen_proxy.send_event(UserEvent::Fullscreen {
                                    tab: fullscreen_tab.clone(),
                                    active: contains.as_bool(),
                                });
                                Ok(())
                            }),
                        );
                        let mut token = 0;
                        if let Err(error) =
                            core.add_ContainsFullScreenElementChanged(&handler, &mut token)
                        {
                            log::warn!("registering WebView2 fullscreen handler failed: {error}");
                        }
                    }
                    Err(error) => {
                        log::warn!("getting WebView2 core for fullscreen handler failed: {error}")
                    }
                }
            }
        }

        // Crash recovery: a dead renderer (or browser process) is reported to the main loop,
        // which recreates the WebView instead of leaving a blank/frozen tab.
        #[cfg(windows)]
        {
            use webview2_com::{Microsoft::Web::WebView2::Win32::*, ProcessFailedEventHandler};

            let crash_proxy = self.proxy.clone();
            let crash_tab = id.clone();
            unsafe {
                match wv.controller().CoreWebView2() {
                    Ok(core) => {
                        let handler =
                            ProcessFailedEventHandler::create(Box::new(move |_sender, args| {
                                let Some(args) = args else { return Ok(()) };
                                let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED;
                                args.ProcessFailedKind(&mut kind)?;
                                let browser =
                                    kind == COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED;
                                let render =
                                    kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED;
                                if browser || render {
                                    let _ = crash_proxy.send_event(UserEvent::TabCrashed {
                                        tab: crash_tab.clone(),
                                        browser,
                                    });
                                }
                                Ok(())
                            }));
                        let mut token = 0;
                        if let Err(error) = core.add_ProcessFailed(&handler, &mut token) {
                            log::warn!("registering WebView2 crash handler failed: {error}");
                        }
                    }
                    Err(error) => {
                        log::warn!("getting WebView2 core for crash handler failed: {error}")
                    }
                }
            }
        }

        if self.ext_done.insert(profile_id) {
            extensions::load_all(&wv, &self.root.join("extensions"));
        }
        self.tabs[idx].webview = Some(wv);
        Ok(())
    }

    /// Recreates the WebView of a tab whose renderer died, or of every tab of a profile whose
    /// browser process died. Guards against crash loops.
    fn recover_tab(&mut self, tab: &str, browser_died: bool) {
        let Some(profile) = self
            .tabs
            .iter()
            .find(|t| t.id == tab)
            .map(|t| t.profile_id.clone())
        else {
            return;
        };

        let now = Instant::now();
        let entry = self.crashes.entry(tab.to_string()).or_insert((now, 0));
        if now.duration_since(entry.0) > Duration::from_secs(30) {
            *entry = (now, 0);
        }
        entry.1 += 1;
        let count = entry.1;
        if count > 3 {
            self.toast("This page keeps crashing, so it was not reloaded again.");
            return;
        }

        let victims: Vec<usize> = self
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                t.webview.is_some()
                    && if browser_died {
                        t.profile_id == profile
                    } else {
                        t.id == tab
                    }
            })
            .map(|(i, _)| i)
            .collect();
        if victims.is_empty() {
            return;
        }

        let visible_hit = self
            .visible
            .as_deref()
            .map(|v| victims.iter().any(|&i| self.tabs[i].id == v))
            .unwrap_or(false);
        if visible_hit {
            if self.devtools_open {
                self.close_devtools();
            }
            self.leave_fullscreen();
        }
        let revive = if visible_hit { self.visible.take() } else { None };
        for i in victims {
            self.tabs[i].webview = None;
        }
        if browser_died {
            // The profile's environment is gone, so extensions must be loaded again.
            self.ext_done.remove(&profile);
        }
        if let Some(id) = revive {
            if let Err(e) = self.activate(&id) {
                log::error!("could not recover crashed tab: {e:#}");
            }
        }
        self.toast("A page crashed and was reloaded.");
    }

    fn leave_fullscreen(&mut self) {
        if self.fullscreen {
            if let Some(wv) = self.visible_webview() {
                let _ = wv.evaluate_script(
                    "if (document.fullscreenElement) document.exitFullscreen().catch(()=>{});",
                );
            }
            self.fullscreen = false;
            self.window.set_fullscreen(None);
            let _ = self.sidebar.set_visible(true);
        }
    }

    fn hide_visible(&mut self) {
        self.leave_fullscreen();
        let Some(id) = self.visible.take() else {
            return;
        };
        self.responsive_mode = false;
        self.responsive_bounds = None;
        let auto_pip = self.p.auto_pip;
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            // The idle clock (for sleeping the tab) starts when the user leaves it.
            t.last_active = SystemTime::now();
            if let Some(wv) = t.webview.as_ref() {
                if auto_pip {
                    pip::on_leave(wv);
                }
                let _ = wv.set_visible(false);
                if !is_playing_audio(wv) {
                    set_memory_level(wv, true);
                }
            }
        }
        if let Some(devtools) = self.devtools.as_ref() {
            let _ = devtools.set_visible(false);
        }
    }

    fn activate(&mut self, id: &str) -> Result<()> {
        let Some(idx) = self.tabs.iter().position(|t| t.id == id) else {
            return Ok(());
        };
        self.tabs[idx].last_active = SystemTime::now();
        if self.visible.as_deref() == Some(id) && !self.overlay {
            return Ok(());
        }
        self.hide_visible();
        self.ensure_webview(idx)?;
        self.overlay = false;
        self.visible = Some(id.to_string());
        self.active
            .insert(self.p.current_space.clone(), id.to_string());
        if let Some(wv) = &self.tabs[idx].webview {
            set_memory_level(wv, false);
        }
        self.relayout();
        if let Some(wv) = &self.tabs[idx].webview {
            wv.focus()?;
            if self.p.auto_pip {
                pip::on_enter(wv);
            }
        }
        if self.p.restore_tabs {
            self.save();
        }
        if self.devtools_open {
            self.attach_devtools()?;
        }
        Ok(())
    }

    fn close_tab(&mut self, id: &str) -> Result<()> {
        let Some(idx) = self.tabs.iter().position(|t| t.id == id) else {
            return Ok(());
        };
        let was_visible = self.visible.as_deref() == Some(id);
        if was_visible {
            self.leave_fullscreen();
            self.visible = None;
            self.responsive_mode = false;
            self.responsive_bounds = None;
            if let Some(wv) = &self.tabs[idx].webview {
                let _ = wv.set_visible(false);
            }
            if let Some(devtools) = self.devtools.as_ref() {
                let _ = devtools.set_visible(false);
            }
        }
        if self.tabs[idx].kind == Kind::Normal {
            self.closed_tabs.insert(0, self.tabs[idx].url.clone());
            self.closed_tabs.truncate(25);
            self.tabs.remove(idx);
        } else {
            // Pinned / favorite: unload and reset to its home URL, keep the entry.
            let t = &mut self.tabs[idx];
            t.webview = None;
            t.url = t.home.clone();
        }
        self.active.retain(|_, v| v != id);
        if was_visible {
            self.restore_space_tab()?;
        }
        self.prune_ext_done();
        self.save();
        Ok(())
    }

    fn reopen_closed_tab(&mut self) -> Result<()> {
        if self.closed_tabs.is_empty() {
            return Ok(());
        }
        let url = self.closed_tabs.remove(0);
        self.new_tab(&url)
    }

    /// Tabs reachable from the current space, in sidebar order.
    fn space_tab_ids(&self) -> Vec<String> {
        self.tabs
            .iter()
            .filter(|t| {
                t.space_id.as_deref() == Some(self.p.current_space.as_str())
                    || t.kind == Kind::Favorite
            })
            .map(|t| t.id.clone())
            .collect()
    }

    fn cycle_tab(&mut self, direction: i32) -> Result<()> {
        let ids = self.space_tab_ids();
        if ids.len() < 2 {
            return Ok(());
        }
        let current = self
            .visible
            .as_ref()
            .and_then(|id| ids.iter().position(|x| x == id))
            .unwrap_or(0) as i32;
        let next = (current + direction).rem_euclid(ids.len() as i32) as usize;
        self.activate(&ids[next])
    }

    fn activate_tab_index(&mut self, index: usize) -> Result<()> {
        let ids = self.space_tab_ids();
        if ids.is_empty() {
            return Ok(());
        }
        self.activate(&ids[index.min(ids.len() - 1)])
    }

    fn restore_space_tab(&mut self) -> Result<()> {
        let sp = self.p.current_space.clone();
        let remembered = self
            .active
            .get(&sp)
            .cloned()
            .filter(|id| self.tabs.iter().any(|t| &t.id == id));
        let pick = remembered.or_else(|| {
            self.tabs
                .iter()
                .filter(|t| t.space_id.as_deref() == Some(sp.as_str()))
                .min_by_key(|t| (t.kind != Kind::Pinned) as u8)
                .map(|t| t.id.clone())
        });
        if let Some(id) = pick {
            self.activate(&id)?;
        } else if self.devtools_open {
            self.close_devtools();
        }
        Ok(())
    }

    fn switch_space(&mut self, id: &str) -> Result<()> {
        if !self.p.spaces.iter().any(|s| s.id == id) || self.p.current_space == id {
            return Ok(());
        }
        self.hide_visible(); // triggers auto-PiP for the outgoing tab
        self.p.current_space = id.to_string();
        self.restore_space_tab()?;
        self.save();
        Ok(())
    }

    fn cycle_space(&mut self, dir: i32) -> Result<()> {
        let n = self.p.spaces.len() as i32;
        let cur = self
            .p
            .spaces
            .iter()
            .position(|s| s.id == self.p.current_space)
            .unwrap_or(0) as i32;
        let next = (cur + dir).rem_euclid(n) as usize;
        let id = self.p.spaces[next].id.clone();
        self.switch_space(&id)
    }

    fn delete_space(&mut self, id: &str) -> Result<()> {
        if self.p.spaces.len() <= 1 {
            return Ok(());
        }
        if self.p.current_space == id {
            self.hide_visible();
        }
        self.tabs.retain(|t| t.space_id.as_deref() != Some(id));
        self.p.spaces.retain(|s| s.id != id);
        // Folders of a deleted space would otherwise live in state.json forever.
        self.p.folders.retain(|f| f.space_id != id);
        self.active.remove(id);
        if self.p.current_space == id {
            self.p.current_space = self.p.spaces[0].id.clone();
            self.restore_space_tab()?;
        }
        self.prune_ext_done();
        self.save();
        Ok(())
    }

    /// A profile whose last WebView is gone loses its WebView2 environment, so its extensions
    /// have to be loaded again the next time one is created.
    fn prune_ext_done(&mut self) {
        let live: HashSet<String> = self
            .tabs
            .iter()
            .filter(|t| t.webview.is_some())
            .map(|t| t.profile_id.clone())
            .collect();
        self.ext_done.retain(|p| live.contains(p));
    }

    /// Releases the WebView (and with it the renderer process) of background tabs that have been
    /// idle for `tab_sleep_minutes`. The tab stays in the sidebar at its current URL and reloads
    /// when selected. The visible tab, the DevTools target and tabs playing audio are kept.
    fn sleep_idle_tabs(&mut self) {
        let minutes = self.p.tab_sleep_minutes;
        if minutes == 0 {
            return;
        }
        let limit = Duration::from_secs(minutes.saturating_mul(60));
        let now = SystemTime::now();
        let visible = self.visible.clone();
        let devtools_tab = self.devtools_tab.clone();
        let mut slept = false;
        for t in self.tabs.iter_mut() {
            let Some(wv) = t.webview.as_ref() else {
                continue;
            };
            if visible.as_deref() == Some(t.id.as_str())
                || devtools_tab.as_deref() == Some(t.id.as_str())
            {
                continue;
            }
            if now.duration_since(t.last_active).unwrap_or(Duration::ZERO) < limit {
                continue;
            }
            if is_playing_audio(wv) {
                t.last_active = now;
                continue;
            }
            t.webview = None;
            slept = true;
        }
        if slept {
            self.prune_ext_done();
        }
    }

    /// Expires inactive normal tabs, puts idle ones to sleep and applies profile-level
    /// browsing-data policies.
    pub fn cleanup_expired_tabs(&mut self) {
        let now = SystemTime::now();
        let expired: Vec<String> = self
            .tabs
            .iter()
            .filter(|t| t.kind == Kind::Normal && self.visible.as_deref() != Some(t.id.as_str()))
            .filter(|t| {
                let hours = self
                    .p
                    .profiles
                    .iter()
                    .find(|p| p.id == t.profile_id)
                    .map(|p| p.tab_retention_hours)
                    .unwrap_or(48);
                now.duration_since(t.last_active)
                    .unwrap_or(Duration::ZERO)
                    .as_secs()
                    >= hours.saturating_mul(3600)
            })
            .map(|t| t.id.clone())
            .collect();
        for id in expired {
            if let Err(e) = self.close_tab(&id) {
                log::warn!("auto-tidy could not close tab: {e:#}");
            }
        }
        self.sleep_idle_tabs();
        self.cleanup_profile_data();
        self.push_state();
    }

    #[cfg(windows)]
    fn cleanup_profile_data(&mut self) {
        use webview2_com::{
            ClearBrowsingDataCompletedHandler,
            Microsoft::Web::WebView2::Win32::{
                ICoreWebView2Profile2, ICoreWebView2_13,
                COREWEBVIEW2_BROWSING_DATA_KINDS_BROWSING_HISTORY,
                COREWEBVIEW2_BROWSING_DATA_KINDS_COOKIES,
                COREWEBVIEW2_BROWSING_DATA_KINDS_DISK_CACHE,
            },
        };
        use windows::core::Interface;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();
        let due_profiles: Vec<String> = self
            .p
            .profiles
            .iter()
            .filter(|p| {
                p.cookie_retention_hours > 0
                    || p.cache_retention_hours > 0
                    || p.history_retention_hours > 0
            })
            .filter(|p| now.saturating_sub(p.last_cleanup_at) >= 3600)
            .map(|p| p.id.clone())
            .collect();

        for profile_id in due_profiles {
            let Some(webview) = self
                .tabs
                .iter()
                .find(|t| t.profile_id == profile_id && t.webview.is_some())
                .and_then(|t| t.webview.as_ref())
            else {
                continue;
            };

            let settings = self
                .p
                .profiles
                .iter()
                .find(|p| p.id == profile_id)
                .map(|p| {
                    (
                        p.cookie_retention_hours,
                        p.cache_retention_hours,
                        p.history_retention_hours,
                    )
                })
                .unwrap_or((0, 0, 0));

            let result = (|| -> anyhow::Result<()> {
                unsafe {
                    let core = webview.controller().CoreWebView2()?;
                    let core13: ICoreWebView2_13 = core.cast()?;
                    let profile = core13.Profile()?;
                    let profile2: ICoreWebView2Profile2 = profile.cast()?;
                    for (hours, kind) in [
                        (settings.0, COREWEBVIEW2_BROWSING_DATA_KINDS_COOKIES),
                        (settings.1, COREWEBVIEW2_BROWSING_DATA_KINDS_DISK_CACHE),
                        (
                            settings.2,
                            COREWEBVIEW2_BROWSING_DATA_KINDS_BROWSING_HISTORY,
                        ),
                    ] {
                        if hours == 0 {
                            continue;
                        }
                        let cutoff = now.saturating_sub(hours.saturating_mul(3600)) as f64;
                        let handler =
                            ClearBrowsingDataCompletedHandler::create(Box::new(|_| Ok(())));
                        profile2.ClearBrowsingDataInTimeRange(kind, 0.0, cutoff, &handler)?;
                    }
                }
                Ok(())
            })();

            match result {
                Ok(()) => {
                    if let Some(p) = self.p.profiles.iter_mut().find(|p| p.id == profile_id) {
                        p.last_cleanup_at = now;
                    }
                    self.save();
                }
                Err(e) => log::warn!("profile browsing-data cleanup failed: {e:#}"),
            }
        }
    }

    /// Age-scoped profile cleanup is only available through WebView2.
    #[cfg(not(windows))]
    fn cleanup_profile_data(&mut self) {}

    fn sidebar_left(&self) -> f64 {
        if self.p.sidebar_collapsed {
            0.0
        } else if self.p.sidebar_compact {
            SIDEBAR_COMPACT_W
        } else {
            self.p.sidebar_width
        }
    }

    /// [x, y, w, h] of the area normally occupied by web content.
    fn content_box(&self) -> [f64; 4] {
        let (w, h) = logical(&self.window);
        let left = self.sidebar_left();
        [
            left + INSET,
            TOPBAR_H,
            (w - left - 2.0 * INSET).max(1.0),
            (h - TOPBAR_H - INSET).max(1.0),
        ]
    }

    fn content_rect(&self) -> Rect {
        if self.fullscreen {
            full_rect(&self.window)
        } else {
            rect_of(self.content_box())
        }
    }

    /// [x, y, w, h] the DevTools frontend occupies in device mode: the content area minus the header.
    fn responsive_tools_box(&self) -> [f64; 4] {
        let [x, y, w, h] = self.content_box();
        [x, y + HEADER_H, w, (h - HEADER_H).max(1.0)]
    }

    /// Largest panel size that still leaves a usable page.
    fn devtools_limit(&self) -> f64 {
        let [_, _, w, h] = self.content_box();
        let avail = if matches!(self.p.devtools_position.as_str(), "left" | "right") {
            w
        } else {
            h
        };
        (avail - PANEL_GAP - PAGE_MIN).clamp(PANEL_MIN, PANEL_MAX)
    }

    fn devtools_size(&self) -> f64 {
        self.p.devtools_size.clamp(PANEL_MIN, self.devtools_limit())
    }

    fn layout(&self) -> Layout {
        let mut out = Layout {
            page: self.content_rect(),
            page_visible: true,
            tools: None,
            header: None,
            handle: None,
        };
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
        let _ = self.sidebar.set_visible(!self.fullscreen);
        let layout = self.layout();

        let page_shown = !self.overlay && layout.page_visible;
        if let Some(wv) = self.visible_webview() {
            if let Err(error) = apply_webview_bounds(wv, layout.page, scale) {
                log::warn!("resizing content WebView failed: {error:#}");
            }
            let _ = wv.set_visible(page_shown);
            #[cfg(windows)]
            {
                let [_, _, cw, ch] = self.content_box();
                apply_webview_radius(wv, cw, ch, scale, !self.fullscreen && !self.devtools_open);
                if page_shown && self.devtools_open && self.responsive_mode {
                    raise_webview(wv);
                }
            }
        }

        if let Some(devtools) = self.devtools.as_ref() {
            match layout.tools {
                Some(bounds) => {
                    if let Err(error) = apply_webview_bounds(devtools, bounds, scale) {
                        log::warn!("resizing DevTools WebView failed: {error:#}");
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
    }

    /// Shows, moves or hides the resize handle and header strip drawn by the sidebar WebView.
    fn sync_devtools_chrome(&self, layout: &Layout) {
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

    /// Marks the state dirty and writes it unless a write just happened; bursts of changes
    /// (navigation, favicons, drags) are coalesced and flushed by `flush_save` from the event
    /// loop timer, so the UI thread is not blocked by disk I/O on every event.
    pub fn save(&mut self) {
        self.save_dirty = true;
        if self.last_save.elapsed() >= SAVE_MIN_INTERVAL {
            self.flush_save();
        }
    }

    pub fn save_pending(&self) -> bool {
        self.save_dirty
    }

    /// Final, unconditional write on exit.
    pub fn shutdown(&mut self) {
        self.save_dirty = true;
        self.flush_save();
        if let Some(devtools) = self.devtools.take() {
            drop(devtools);
        }
    }

    pub fn flush_save(&mut self) {
        if !self.save_dirty {
            return;
        }
        let restore = self.p.restore_tabs;
        self.p.saved = self
            .tabs
            .iter()
            .filter(|t| t.kind != Kind::Normal || (restore && t.url != "about:blank"))
            .map(|t| Saved {
                id: t.id.clone(),
                profile_id: t.profile_id.clone(),
                space_id: t.space_id.clone(),
                kind: t.kind,
                folder_id: t.folder_id.clone(),
                // Pinned/favorite tabs reset to `home`; normal tabs resume where they were.
                url: if t.kind == Kind::Normal {
                    t.url.clone()
                } else {
                    t.home.clone()
                },
                title: t.title.clone(),
                // Large inline icons are kept in memory only, not written to disk.
                favicon: t
                    .favicon
                    .clone()
                    .filter(|f| f.len() <= FAVICON_PERSIST_MAX),
            })
            .collect();
        self.p.active_tabs = if restore {
            self.active.clone()
        } else {
            HashMap::new()
        };
        let ok = self.p.save(&self.root.join("state.json"));
        self.last_save = Instant::now();
        if ok {
            self.save_dirty = false;
        } else {
            log::warn!("saving state.json failed; will retry");
        }
    }

    /// Visited sites by use first, then popular defaults the user hasn't visited yet.
    fn suggestions(&self) -> Vec<Value> {
        let mut user: Vec<_> = self.p.sites.iter().collect();
        user.sort_by(|a, b| b.1.count.cmp(&a.1.count).then(a.0.cmp(b.0)));
        let mut seen: HashSet<String> = HashSet::new();
        let mut out: Vec<Value> = Vec::new();
        for (host, s) in user.into_iter().take(40) {
            seen.insert(host.clone());
            out.push(json!({"label": host, "url": s.url, "favicon": s.favicon, "visits": s.count, "popular": false}));
        }
        for (name, u) in DEFAULT_SITES {
            let host = url::Url::parse(u)
                .ok()
                .and_then(|x| {
                    x.host_str()
                        .map(|h| h.trim_start_matches("www.").to_string())
                })
                .unwrap_or_default();
            if seen.insert(host) {
                out.push(
                    json!({"label": name, "url": u, "favicon": null, "visits": 0, "popular": true}),
                );
            }
        }
        out
    }

    /// Suggestions are rebuilt only after visit stats changed, not on every state push.
    fn suggestions_cached(&self) -> std::cell::Ref<'_, [Value]> {
        if self.sugg_cache.borrow().is_none() {
            let fresh = self.suggestions();
            *self.sugg_cache.borrow_mut() = Some(fresh);
        }
        std::cell::Ref::map(self.sugg_cache.borrow(), |cache| {
            cache.as_deref().unwrap_or(&[])
        })
    }

    /// If the visible tab is a Chrome Web Store detail page, the extension's ID.
    fn store_ext(&self) -> Option<String> {
        let t = self
            .visible
            .as_ref()
            .and_then(|id| self.tabs.iter().find(|t| &t.id == id))?;
        let u = url::Url::parse(&t.url).ok()?;
        match u.host_str()? {
            "chromewebstore.google.com" | "chrome.google.com" if u.path().contains("/detail/") => {
                store::parse_id(u.path())
            }
            _ => None,
        }
    }

    fn push_state(&self) {
        let sp = self.cur_space();
        let row = |t: &Tab| {
            json!({
                "id": t.id, "title": t.title, "url": t.url,
                "favicon": t.favicon, "loaded": t.webview.is_some(), "folder_id": t.folder_id
            })
        };
        let favorites: Vec<_> = self
            .tabs
            .iter()
            .filter(|t| t.kind == Kind::Favorite && t.profile_id == sp.profile_id)
            .map(row)
            .collect();
        let in_space = |k: Kind| -> Vec<_> {
            self.tabs
                .iter()
                .filter(|t| t.kind == k && t.space_id.as_deref() == Some(sp.id.as_str()))
                .map(row)
                .collect()
        };
        let url = self
            .visible
            .as_ref()
            .and_then(|id| self.tabs.iter().find(|t| &t.id == id))
            .map(|t| t.url.clone())
            .unwrap_or_default();

        let sugg_ref = self.suggestions_cached();
        let sugg: &[Value] = &sugg_ref;
        let state = json!({
            "profiles": self.p.profiles,
            "spaces": self.p.spaces,
            "current": sp.id,
            "active": self.visible,
            "url": url,
            "favorites": favorites,
            "pinned": in_space(Kind::Pinned),
            "tabs": in_space(Kind::Normal),
            "folders": self.p.folders.iter().filter(|f| f.space_id == sp.id).collect::<Vec<_>>(),
            "suggestions": sugg,
            "store_ext": self.store_ext(),
            "theme": self.p.theme,
            "auto_pip": self.p.auto_pip,
            "restore_tabs": self.p.restore_tabs,
            "sidebar_collapsed": self.p.sidebar_collapsed,
            "sidebar_width": self.p.sidebar_width,
            "sidebar_compact": self.p.sidebar_compact,
            "devtools_open": self.devtools_open,
            "devtools_position": self.p.devtools_position,
            "devtools_size": self.p.devtools_size,
            "profile_cleanup": self.p.profiles.iter()
                .find(|p| p.id == sp.profile_id)
                .map(|p| json!({
                    "tab_retention_hours": p.tab_retention_hours,
                    "cookie_retention_hours": p.cookie_retention_hours,
                    "cache_retention_hours": p.cache_retention_hours,
                    "history_retention_hours": p.history_retention_hours
                }))
                .unwrap_or_else(|| json!({})),
        });
        drop(sugg_ref);

        // Skip the IPC round-trip and the sidebar re-render when nothing visible changed.
        let payload = state.to_string();
        let mut hasher = DefaultHasher::new();
        payload.hash(&mut hasher);
        let digest = hasher.finish().max(1);
        if self.last_state_hash.get() == digest {
            return;
        }
        if self
            .sidebar
            .evaluate_script(&format!("window.__setState({payload})"))
            .is_ok()
        {
            self.last_state_hash.set(digest);
        }
    }
}