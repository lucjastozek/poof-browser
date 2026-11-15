use crate::{
    extensions, pip,
    state::{new_id, valid_icon, Kind, Persisted, Profile, Saved, Space, TabFolder},
    store, UserEvent,
};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
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
use std::{path::Path, thread};
#[cfg(windows)]
use wry::WebViewExtWindows;

#[cfg(windows)]
#[cfg(windows)]
fn apply_webview_bounds(wv: &WebView, bounds: Rect, scale: f64) -> Result<()> {
    use windows::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER},
    };

    // Keep WebView2's viewport in sync, then position its child HWND
    // synchronously. Wry's normal set_bounds uses ASYNCWINDOWPOS on Windows,
    // which can visibly lag behind rapid sidebar pointer-move events.
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

fn apply_webview_radius(wv: &WebView, width: f64, height: f64, scale: f64, rounded: bool) {
    use windows::Win32::{
        Foundation::HWND,
        Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, HGDIOBJ, SetWindowRgn},
    };
    use wry::WebViewExtWindows;

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
        // SetWindowRgn takes ownership only when it succeeds.
        if SetWindowRgn(hwnd, Some(region), true) == 0 {
            let _ = DeleteObject(HGDIOBJ(region.0));
        }
    }
}

const SIDEBAR_MIN_W: f64 = 180.0;
const SIDEBAR_MAX_W: f64 = 480.0;
const SIDEBAR_COMPACT_W: f64 = 56.0;
const INSET: f64 = 8.0;
const TOPBAR_H: f64 = 40.0;

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

/// The hosted DevTools frontend normally leaves this embedder hook empty.
/// Forward its inspected-page placeholder bounds to the native WebView host.
const DEVTOOLS_BOUNDS_INIT_JS: &str = r#"
(() => {
  let host = window.InspectorFrontendHost;
  const report = bounds => {
    if (!bounds || ![bounds.x, bounds.y, bounds.width, bounds.height].every(Number.isFinite)) return;
    window.ipc?.postMessage(JSON.stringify({t: "inspected_page_bounds", ...bounds}));
  };
  const patch = value => {
    if (!value || value.__poofBoundsPatched || typeof value.setInspectedPageBounds !== "function") return;
    const original = value.setInspectedPageBounds;
    value.setInspectedPageBounds = function(bounds) {
      report(bounds);
      return original.call(this, bounds);
    };
    Object.defineProperty(value, "__poofBoundsPatched", {value: true});
  };
  patch(host);
  try {
    Object.defineProperty(window, "InspectorFrontendHost", {
      configurable: true,
      enumerable: true,
      get: () => host,
      set: value => { host = value; patch(value); }
    });
  } catch (error) {
    console.error("Poof could not install the DevTools page-bounds bridge", error);
  }
})();
"#;

#[derive(Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
enum DevToolsMsg {
    InspectedPageBounds { x: f64, y: f64, width: f64, height: f64 },
}

fn devtools_init_script(position: &str, size: f64) -> String {
    let dock_side = match position {
        "left" => "left",
        "bottom" | "top" => "bottom",
        _ => "right",
    };
    let vertical = matches!(position, "left" | "right");
    let second_is_sidebar = matches!(position, "right" | "bottom");
    format!(
        r#"
(() => {{
  try {{
    localStorage.setItem("currentDockState", JSON.stringify("{dock_side}"));
    localStorage.setItem("inspector-view.split-view-state", JSON.stringify({{
      vertical: {{size: {size}}},
      horizontal: {{size: {size}}}
    }}));
  }} catch (_) {{}}
  {DEVTOOLS_BOUNDS_INIT_JS}
  const applyPanelLayout = () => {{
    const findSplit = root => {{
      const direct = root.querySelector?.(".split-widget");
      if (direct) return direct;
      for (const element of root.querySelectorAll?.("*") || []) {{
        if (element.shadowRoot) {{
          const nested = findSplit(element.shadowRoot);
          if (nested) return nested;
        }}
      }}
      return null;
    }};
    const split = findSplit(document);
    const sidebar = split?.shadowRoot?.querySelector(".shadow-split-widget-sidebar");
    if (sidebar) {{
      sidebar.style.setProperty("flex-basis", "{size}px", "important");
      sidebar.style.setProperty("{dimension}", "{size}px", "important");
    }}
    const rootSplit = window.Emulation?.AdvancedApp?.instance?.().rootSplitWidget;
    if (rootSplit) {{
      rootSplit.setVertical({vertical});
      rootSplit.setSecondIsSidebar({second_is_sidebar});
      rootSplit.showBoth();
    }}
  }};
  window.addEventListener("load", applyPanelLayout, {{once: true}});
}})();
"#,
        dock_side = dock_side,
        size = size.clamp(240.0, 800.0),
        dimension = if matches!(position, "top" | "bottom") { "height" } else { "width" },
        vertical = vertical,
        second_is_sidebar = second_is_sidebar,
        DEVTOOLS_BOUNDS_INIT_JS = DEVTOOLS_BOUNDS_INIT_JS,
    )
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
    pub webview: Option<WebView>, // None = not loaded yet (lazy)
    pub last_active: SystemTime,
}

#[derive(Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum Cmd {
    Ready,
    SidebarCollapsed { collapsed: bool },
    SidebarCompact { compact: bool },
    SidebarWidth { width: f64, #[serde(default)] persist: bool },
    WindowMinimize,
    WindowToggleMaximize,
    WindowDrag,
    WindowClose,
    /// The sidebar is showing a full-area view (new tab bar, dialogs): hide/show the page.
    Overlay { open: bool },
    NewTab { input: String },
    Navigate { input: String },
    SelectTab { id: String },
    CloseTab { id: String },
    PinTab { id: String },
    UnpinTab { id: String },
    FavTab { id: String },
    UnfavTab { id: String },
    SwitchSpace { id: String },
    CycleSpace { dir: i32 },
    AddSpace {
        name: String,
        icon: String,
        color: String,
        #[serde(default)]
        profile_id: Option<String>,
        #[serde(default)]
        new_profile: Option<String>,
    },
    EditSpace { id: String, name: String, icon: String, color: String },
    /// Drag & drop: put a tab into the pinned list, the tab list or the favorites at `index`.
    MoveTab { id: String, to: String, index: usize },
    CreateFolder { name: String },
    RenameFolder { id: String, name: String },
    DeleteFolder { id: String },
    ToggleFolderCollapsed { id: String },
    MoveFolder { id: String, to: String, index: usize },
    MoveTabToFolder { id: String, folder_id: Option<String> },
    SetTheme { theme: String },
    SetAutoPip { on: bool },
    UpdateProfileCleanup {
        tab_retention_hours: u64,
        cookie_retention_hours: u64,
        cache_retention_hours: u64,
        history_retention_hours: u64,
    },
    DeleteSpace { id: String },
    ListExt,
    InstallExt { input: String },
    RemoveExt { dir: String },
    Back,
    Forward,
    Reload,
    DevToolsToggle,
    DevToolsPosition { position: String },
    DevToolsSize { size: f64, #[serde(default)] persist: bool },
}

#[derive(Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
enum PageMsg {
    Meta { u: Option<String> },
    Favicon { f: Option<String> },
    Key { k: String },
    Fullscreen { active: bool },
}

pub struct Browser {
    window: Window,
    proxy: EventLoopProxy<UserEvent>,
    root: PathBuf,
    sidebar: WebView,
    _ui_ctx: WebContext,
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
    closed_tabs: Vec<(String, String)>,
    devtools_open: bool,
    devtools: Option<WebView>,
    devtools_tab: Option<String>,
    /// Preview bounds relative to the full DevTools frontend surface.
    devtools_page_bounds: Option<(f64, f64, f64, f64)>,
}

// ---------- geometry ----------
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
fn full_rect(w: &Window) -> Rect {
    let (ww, hh) = logical(w);
    rect(0.0, 0.0, ww, hh)
}
fn content_rect(w: &Window, collapsed: bool, compact: bool, sidebar_width: f64, fullscreen: bool) -> Rect {
    let (ww, hh) = logical(w);
    if fullscreen {
        return full_rect(w);
    }
    let left = if collapsed { 0.0 } else if compact { SIDEBAR_COMPACT_W } else { sidebar_width };
    rect(left + INSET, TOPBAR_H, ww - left - 2.0 * INSET, hh - TOPBAR_H - INSET)
}

// ---------- input helpers ----------
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

/// Keep the last usable icon when WebView2 reports a transient empty result.
fn update_favicon(current: &mut Option<String>, incoming: Option<String>) -> bool {
    let Some(favicon) = incoming.filter(|value| ok_url(value, 512_000, true)) else {
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

/// Map WebView2 accelerator keys to browser commands. Virtual-key values are
/// the Win32 VK_* constants so this logic can be tested without a WebView.
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
            0x54 => Some("reopen"), // T
            0x52 => Some("reload"), // R
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
        0x31..=0x38 => Some(match key {
            0x31 => "tab_1", 0x32 => "tab_2", 0x33 => "tab_3", 0x34 => "tab_4",
            0x35 => "tab_5", 0x36 => "tab_6", 0x37 => "tab_7", _ => "tab_8",
        }),
        0x39 => Some("tab_last"),
        VK_TAB => Some("tab_next"),
        _ => None,
    }
}

fn clean_icon(s: &str) -> String {
    if valid_icon(s) { s.to_string() } else { "house".into() }
}

fn clip(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

#[cfg(windows)]
fn devtools_target_id(webview: &WebView) -> Result<String> {
    use std::sync::{Arc, Mutex};
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

#[cfg(any(windows, test))]
const WEBVIEW2_BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --remote-debugging-address=127.0.0.1 --remote-debugging-port=0 --remote-allow-origins=*";

#[cfg(any(windows, test))]
fn devtools_frontend_url(port: u16, target_id: &str) -> Result<String> {
    if port == 0
        || target_id.is_empty()
        || !target_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        anyhow::bail!("invalid WebView2 DevTools endpoint");
    }

    // WebView2 ships the real frontend behind its privileged devtools:// scheme.
    // The HTTP /devtools/inspector.html route is not a reliable frontend host.
    Ok(format!(
        "devtools://devtools/bundled/inspector.html?ws=127.0.0.1:{port}/devtools/page/{target_id}"
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
            let Ok(contents) = fs::read_to_string(path) else { continue };
            let Some(port) = contents.lines().next().and_then(|line| line.trim().parse::<u16>().ok()) else {
                continue;
            };
            if let Ok(url) = devtools_frontend_url(port, target_id) {
                return Ok(url);
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
    anyhow::bail!("WebView2 DevTools endpoint did not become ready for the active tab")
}

#[cfg(windows)]
fn register_browser_shortcuts(
    webview: &WebView,
    proxy: EventLoopProxy<UserEvent>,
    tab: String,
) {
    use webview2_com::{
        AcceleratorKeyPressedEventHandler, Microsoft::Web::WebView2::Win32::*,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;

    unsafe {
        let handler =
            AcceleratorKeyPressedEventHandler::create(Box::new(move |_sender, args| {
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
        let p = Persisted::load(&root.join("state.json"));

        // The sidebar is our own trusted UI (HTML), living in its own data folder.
        // theme.css holds every design token; it is spliced into the page here.
        let html = include_str!("../ui/sidebar.html")
            .replace(
                "/*STYLES*/",
                &format!("<style>{}</style>", [
                    include_str!("../ui/theme.css"),
                    include_str!("../ui/styles/base.css"),
                    include_str!("../ui/styles/components.css"),
                    include_str!("../ui/styles/topbar.css"),
                    include_str!("../ui/styles/sidebar.css"),
                    include_str!("../ui/styles/stage.css"),
                    include_str!("../ui/styles/start-page.css")
                ].concat()),
            )
            .replace("/*ICONS*/", include_str!("../ui/icons.js"))
            .replace("/*CORE*/", include_str!("../ui/scripts/core.js"))
            .replace("/*SIDEBAR*/", include_str!("../ui/scripts/sidebar.js"))
            .replace("/*START_PAGE*/", include_str!("../ui/scripts/start-page.js"))
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

        let mut b = Self {
            window,
            proxy,
            root,
            sidebar,
            _ui_ctx: ui_ctx,
            contexts: HashMap::new(),
            ext_done: HashSet::new(),
            p,
            tabs,
            active: HashMap::new(),
            visible: None,
            overlay: false,
            fullscreen: false,
            closed_tabs: Vec::new(),
            devtools_open: false,
            devtools: None,
            devtools_tab: None,
            devtools_page_bounds: None,
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

    // ---------- extensions ----------
    fn preinstall(&mut self) {
        for id in store::PREINSTALL {
            let id = id.to_string();
            if self.p.preinstalled.contains(&id) {
                continue;
            }
            if self.root.join("extensions").join(&id).join("manifest.json").is_file() {
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
                Err(e) => UserEvent::ExtResult { ok: false, msg: format!("Couldn’t install: {e}"), dir: None, id: Some(id) },
            };
            let _ = proxy.send_event(ev);
        });
    }

    fn toast(&self, msg: &str) {
        let m = serde_json::to_string(msg).unwrap_or_default();
        let _ = self.sidebar.evaluate_script(&format!("window.__toast&&window.__toast({m})"));
    }

    fn push_exts(&self) {
        let list = serde_json::to_string(&store::list(&self.root)).unwrap_or_else(|_| "[]".into());
        let _ = self.sidebar.evaluate_script(&format!("window.__setExts&&window.__setExts({list})"));
    }

    // ---------- events ----------
    pub fn on_user_event(&mut self, ev: UserEvent) {
        match ev {
            UserEvent::CloseWindow => {},
            UserEvent::DevToolsReady { tab, url } => self.show_devtools(&tab, &url),
            UserEvent::DevToolsPageBounds { x, y, width, height } => {
                if self.devtools_open && self.visible.is_some()
                    && [x, y, width, height].iter().all(|value| value.is_finite())
                    && width > 0.0 && height > 0.0
                {
                    self.devtools_page_bounds = Some((x, y, width, height));
                    self.relayout();
                }
            }
            UserEvent::DevToolsFailed { tab, message } => {
                if self.devtools_open && self.visible.as_deref() == Some(tab.as_str()) {
                    self.devtools_open = false;
                    self.devtools_tab = None;
                    self.relayout();
                    self.toast(&message);
                }
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
                            .tabs.iter().filter(|t| t.webview.is_some()).map(|t| t.profile_id.clone()).collect();
                        for pid in &loaded {
                            if let Some(wv) = self
                                .tabs.iter()
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

    /// Switch the entire native application window, not just the embedded WebView, into fullscreen.
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
        let Ok(msg) = serde_json::from_str::<PageMsg>(body) else { return };
        match msg {
            PageMsg::Meta { u } => {
                let mut persist = false;
                if let Some(t) = self.tabs.iter_mut().find(|t| t.id == tab) {
                    if let Some(u) = u.filter(|u| ok_url(u, 4096, false)) {
                        t.url = u.clone();
                        if let Ok(parsed) = url::Url::parse(&u) {
                            if let Some(h) = parsed.host_str() {
                                let host = h.trim_start_matches("www.").to_string();
                                if t.host != host {
                                    // Do not carry the previous site's favicon across navigation.
                                    t.favicon = None;
                                    // A new site was visited in this tab: count it for the new-tab bar.
                                    t.host = host.clone();
                                    let s = self.p.sites.entry(host).or_default();
                                    s.count += 1;
                                    s.url = format!("{}://{}/", parsed.scheme(), h);
                                }
                            }
                        }
                    }
                    if let Some(fv) = &t.favicon {
                        if let Some(s) = self.p.sites.get_mut(&t.host) {
                            s.favicon = Some(fv.clone());
                        }
                    }
                    persist = t.kind != Kind::Normal;
                }
                self.p.prune_sites();
                if persist {
                    self.save();
                }
                // The sidebar is a separate WebView and only sees tab changes
                // when its serialized state is pushed.
                self.push_state();
            }
            PageMsg::Favicon { f } => {
                let mut persist = false;
                if let Some(t) = self.tabs.iter_mut().find(|t| t.id == tab) {
                    let changed = update_favicon(&mut t.favicon, f);
                    if changed {
                        if let Some(s) = self.p.sites.get_mut(&t.host) {
                            s.favicon = t.favicon.clone();
                        }
                        persist = t.kind != Kind::Normal;
                    }
                }
                if persist { self.save(); }
                self.push_state();
            }
            PageMsg::Fullscreen { active } => self.handle_fullscreen(tab, active),
            PageMsg::Key { k } => {
                // An empty tab ID means the shortcut originated in the sidebar;
                // resolve the active tab now, not when the handler was registered.
                let active_tab = self.visible.clone().unwrap_or_default();
                let tab = if tab.is_empty() { active_tab.as_str() } else { tab };
                if self.visible.as_deref() != Some(tab) {
                    return;
                }
                let r = match k.as_str() {
                    "new_tab" => self.new_tab("about:blank"),
                    "focus_url" => {
                        let _ = self.sidebar.focus();
                        let _ = self.sidebar.evaluate_script("window.__focusUrl && window.__focusUrl()");
                        Ok(())
                    }
                    "close" => self.close_tab(tab),
                    "reopen" => self.reopen_closed_tab(),
                    "back" => { self.eval_visible("history.back()"); Ok(()) },
                    "forward" => { self.eval_visible("history.forward()"); Ok(()) },
                    "reload" => { self.eval_visible("location.reload()"); Ok(()) },
                    "space_next" => self.cycle_space(1),
                    "space_prev" => self.cycle_space(-1),
                    "tab_next" => self.cycle_tab(1),
                    "tab_prev" => self.cycle_tab(-1),
                    "tab_1" | "tab_2" | "tab_3" | "tab_4" | "tab_5" | "tab_6" | "tab_7" | "tab_8" | "tab_last" => {
                        let index = if k == "tab_last" { usize::MAX } else { k[4..].parse::<usize>().unwrap_or(1).saturating_sub(1) };
                        self.activate_tab_index(index)
                    }
                    "print" => { self.eval_visible("window.print()"); Ok(()) },
                    "favorite" => { self.set_kind(tab, Kind::Favorite); Ok(()) },
                    "devtools_toggle" => self.toggle_devtools(),
                    _ => Ok(()),
                };
                if let Err(e) = r {
                    log::error!("{e:#}");
                }
            }
        }
        self.push_state();
    }

    fn run(&mut self, cmd: Cmd) -> Result<()> {
        match cmd {
            Cmd::Ready => self.restore_space_tab()?,
            Cmd::SidebarCollapsed { collapsed } => {
                self.p.sidebar_collapsed = collapsed;
                self.save();
                self.relayout();
            }
            Cmd::SidebarCompact { compact } => {
                self.p.sidebar_compact = compact;
                if compact { self.p.sidebar_collapsed = false; }
                self.save();
                self.relayout();
            }
            Cmd::SidebarWidth { width, persist } => {
                if width.is_finite() {
                    self.p.sidebar_width = width.clamp(SIDEBAR_MIN_W, SIDEBAR_MAX_W);
                    if persist { self.save(); }
                    self.relayout();
                }
            }
            Cmd::WindowMinimize => self.window.set_minimized(true),
            Cmd::WindowToggleMaximize => self.window.set_maximized(!self.window.is_maximized()),
            Cmd::WindowDrag => { let _ = self.window.drag_window(); }
            Cmd::WindowClose => { let _ = self.proxy.send_event(UserEvent::CloseWindow); }
            Cmd::Overlay { open } => {
                self.overlay = open;
                if open {
                    let _ = self.sidebar.focus();
                    if let Some(wv) = self.visible_webview() {
                        let _ = wv.set_visible(false);
                    }
                    if let Some(devtools) = self.devtools.as_ref() {
                        let _ = devtools.set_visible(false);
                    }
                } else if let Some(wv) = self.visible_webview() {
                    let _ = wv.set_visible(true);
                    let _ = wv.focus();
                    if self.devtools_open {
                        if let Some(devtools) = self.devtools.as_ref() {
                            let _ = devtools.set_visible(true);
                        }
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
            Cmd::AddSpace { name, icon, color, profile_id, new_profile } => {
                let pid = match new_profile.map(|n| clip(n.trim(), 40)).filter(|n| !n.is_empty()) {
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
                    color: if ok_color(&color) { color } else { "#6dc5e2".into() },
                });
                self.switch_space(&id)?;
            }
            Cmd::EditSpace { id, name, icon, color } => {
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
                    self.p.folders.push(TabFolder { id: new_id(), space_id, name, collapsed: false, pinned: false, order: usize::MAX });
                    self.save();
                }
            }
            Cmd::RenameFolder { id, name } => {
                let name = clip(name.trim(), 40);
                if !name.is_empty() {
                    if let Some(f) = self.p.folders.iter_mut().find(|f| f.id == id && f.space_id == self.p.current_space) { f.name = name; self.save(); }
                }
            }
            Cmd::DeleteFolder { id } => {
                if self.p.folders.iter().any(|f| f.id == id && f.space_id == self.p.current_space) {
                    self.p.folders.retain(|f| f.id != id);
                    for t in self.tabs.iter_mut().filter(|t| t.folder_id.as_deref() == Some(id.as_str())) { t.folder_id = None; }
                    self.save();
                }
            }
            Cmd::ToggleFolderCollapsed { id } => {
                if let Some(f) = self.p.folders.iter_mut().find(|f| f.id == id && f.space_id == self.p.current_space) { f.collapsed = !f.collapsed; self.save(); }
            }
            Cmd::MoveFolder { id, to, index } => {
                let space = self.p.current_space.clone();
                let pinned = to == "pinned";
                if to == "pinned" || to == "tab" {
                    if let Some(from) = self.p.folders.iter().position(|f| f.id == id && f.space_id == space) {
                        let mut folder = self.p.folders.remove(from);
                        let old_order = folder.order;
                        let old_pinned = folder.pinned;
                        // Close the gap in the source section, then make room at the exact
                        // mixed tab/folder position selected by the pointer.
                        for peer in self.p.folders.iter_mut().filter(|f| f.space_id == space && f.pinned == old_pinned && f.order > old_order) {
                            peer.order = peer.order.saturating_sub(1);
                        }
                        for peer in self.p.folders.iter_mut().filter(|f| f.space_id == space && f.pinned == pinned && f.order >= index) {
                            peer.order = peer.order.saturating_add(1);
                        }
                        folder.pinned = pinned;
                        folder.order = index;
                        self.p.folders.push(folder);
                        for tab in self.tabs.iter_mut().filter(|t| t.folder_id.as_deref() == Some(id.as_str())
                            && t.space_id.as_deref() == Some(space.as_str())) {
                            let kind = if pinned { Kind::Pinned } else { Kind::Normal };
                            if tab.kind != kind { tab.home = tab.url.clone(); tab.kind = kind; }
                        }
                        self.save();
                    }
                }
            }
            Cmd::MoveTabToFolder { id, folder_id } => {
                let target_pinned = folder_id.as_ref().and_then(|fid| self.p.folders.iter()
                    .find(|f| f.id == *fid && f.space_id == self.p.current_space))
                    .map(|f| f.pinned);
                let valid = folder_id.is_none() || target_pinned.is_some();
                if valid {
                    if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id
                        && t.space_id.as_deref() == Some(self.p.current_space.as_str())) {
                        if let Some(pinned) = target_pinned {
                            let kind = if pinned { Kind::Pinned } else { Kind::Normal };
                            if t.kind != kind { t.home = t.url.clone(); t.kind = kind; }
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
                    if valid(tab_retention_hours, false) { profile.tab_retention_hours = tab_retention_hours; }
                    if valid(cookie_retention_hours, true) { profile.cookie_retention_hours = cookie_retention_hours; }
                    if valid(cache_retention_hours, true) { profile.cache_retention_hours = cache_retention_hours; }
                    if valid(history_retention_hours, true) { profile.history_retention_hours = history_retention_hours; }
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
                    self.save();
                    self.sync_devtools_panel_size();
                    self.relayout();
                }
            }
            Cmd::DevToolsSize { size, persist } => {
                if size.is_finite() {
                    self.p.devtools_size = size.clamp(240.0, 800.0);
                    if persist { self.save(); }
                    self.sync_devtools_panel_size();
                    self.relayout();
                }
            }
        }
        Ok(())
    }

    fn toggle_devtools(&mut self) -> Result<()> {
        if self.devtools_open {
            self.devtools_open = false;
            self.devtools_tab = None;
            self.devtools_page_bounds = None;
            if let Some(devtools) = self.devtools.as_ref() {
                let _ = devtools.set_visible(false);
            }
            self.relayout();
            return Ok(());
        }
        self.devtools_open = true;
        self.relayout();
        if let Err(error) = self.attach_devtools() {
            self.devtools_open = false;
            self.relayout();
            return Err(error);
        }
        Ok(())
    }

    fn attach_devtools(&mut self) -> Result<()> {
        let Some(tab_id) = self.visible.clone() else {
            self.devtools_open = false;
            self.relayout();
            self.toast("Open a page before opening Developer Tools.");
            return Ok(());
        };
        let Some(tab) = self.tabs.iter().find(|tab| tab.id == tab_id) else {
            self.devtools_open = false;
            self.relayout();
            return Ok(());
        };
        let Some(webview) = tab.webview.as_ref() else {
            self.devtools_open = false;
            self.relayout();
            return Ok(());
        };
        #[cfg(windows)]
        {
            let target_id = match devtools_target_id(webview) {
                Ok(target_id) => target_id,
                Err(error) => {
                    self.devtools_open = false;
                    self.relayout();
                    self.toast(&format!("Could not identify the active DevTools target: {error}"));
                    return Ok(());
                }
            };
            let profile_dir = self.root.join("profiles").join(&tab.profile_id);
            let proxy = self.proxy.clone();
            thread::spawn(move || {
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
            self.devtools_open = false;
            self.relayout();
            self.toast("Embedded Developer Tools are available on Windows only.");
        }
        Ok(())
    }

    fn show_devtools(&mut self, tab: &str, url: &str) {
        if !self.devtools_open || self.visible.as_deref() != Some(tab) {
            return;
        }
        if self.devtools_tab.as_deref() == Some(tab) && self.devtools.is_some() {
            self.relayout();
            if let Some(devtools) = self.devtools.as_ref() {
                let visible = !self.fullscreen && !self.overlay;
                let _ = devtools.set_visible(visible);
                if visible { let _ = devtools.focus(); }
            }
            self.relayout();
            return;
        }
        self.devtools_page_bounds = None;
        let init_script = devtools_init_script(
            &self.p.devtools_position,
            self.p.devtools_size,
        );
        let bounds_proxy = self.proxy.clone();
        let result = if let Some(devtools) = self.devtools.as_ref() {
            devtools.load_url(url)
        } else {
            let (page_bounds, tools_bounds) = self.layout_rects();
            let bounds = tools_bounds.unwrap_or(page_bounds);
            match WebViewBuilder::new_with_web_context(&mut self._ui_ctx)
                .with_bounds(bounds)
                .with_visible(false)
                .with_devtools(false)
                .with_initialization_script_for_main_only(init_script.clone(), false)
                .with_ipc_handler(move |request| {
                    let Ok(message) = serde_json::from_str::<DevToolsMsg>(request.into_body().as_str()) else {
                        return;
                    };
                    match message {
                        DevToolsMsg::InspectedPageBounds { x, y, width, height } => {
                            let _ = bounds_proxy.send_event(UserEvent::DevToolsPageBounds {
                                x, y, width, height,
                            });
                        }
                    }
                })
                .with_url(url)
                .build_as_child(&self.window)
            {
                Ok(devtools) => {
                    #[cfg(windows)]
                    register_browser_shortcuts(&devtools, self.proxy.clone(), tab.to_string());
                    self.devtools = Some(devtools);
                    Ok(())
                }
                Err(error) => Err(error),
            }
        };
        match result {
            Ok(()) => {
                self.devtools_tab = Some(tab.to_string());
                self.relayout();
                self.sync_devtools_panel_size();
                if let Some(devtools) = self.devtools.as_ref() {
                    let visible = !self.fullscreen && !self.overlay;
                    let _ = devtools.set_visible(visible);
                    if visible { let _ = devtools.focus(); }
                }
                self.relayout();
            }
            Err(error) => {
                self.devtools_open = false;
                self.devtools_tab = None;
                if let Some(devtools) = self.devtools.as_ref() {
                    let _ = devtools.set_visible(false);
                }
                self.relayout();
                self.toast(&format!("Could not open Developer Tools: {error}"));
            }
        }
    }

    fn sync_devtools_panel_size(&self) {
        let Some(devtools) = self.devtools.as_ref() else { return };
        let size = self.p.devtools_size.clamp(240.0, 800.0);
        let horizontal = matches!(self.p.devtools_position.as_str(), "top" | "bottom");
        let dock_side = match self.p.devtools_position.as_str() {
            "left" => "left",
            "bottom" | "top" => "bottom",
            _ => "right",
        };
        let vertical = matches!(self.p.devtools_position.as_str(), "left" | "right");
        let second_is_sidebar = matches!(self.p.devtools_position.as_str(), "right" | "bottom");
        let script = format!(
            r#"
(() => {{
  try {{
    localStorage.setItem("currentDockState", JSON.stringify("{dock_side}"));
    localStorage.setItem("inspector-view.split-view-state", JSON.stringify({{
      vertical: {{size: {size}}},
      horizontal: {{size: {size}}}
    }}));
  }} catch (_) {{}}
  const dockController = window.UI?.DockController?.DockController?.instance?.();
  dockController?.setDockSide("{dock_side}");
  const rootSplit = window.Emulation?.AdvancedApp?.instance?.().rootSplitWidget;
  if (rootSplit) {{
    rootSplit.setVertical({vertical});
    rootSplit.setSecondIsSidebar({second_is_sidebar});
    rootSplit.showBoth();
  }}
  const findSplit = root => {{
    const direct = root.querySelector?.(".split-widget");
    if (direct) return direct;
    for (const element of root.querySelectorAll?.("*") || []) {{
      if (element.shadowRoot) {{
        const nested = findSplit(element.shadowRoot);
        if (nested) return nested;
      }}
    }}
    return null;
  }};
  const split = findSplit(document);
  const sidebar = split?.shadowRoot?.querySelector(".shadow-split-widget-sidebar");
  if (sidebar) {{
    sidebar.style.setProperty("flex-basis", "{size}px", "important");
    sidebar.style.setProperty("{dimension}", "{size}px", "important");
  }}
}})();
"#,
            size = size,
            dimension = if horizontal { "height" } else { "width" },
            dock_side = dock_side,
            vertical = vertical,
            second_is_sidebar = second_is_sidebar,
        );
        let _ = devtools.evaluate_script(&script);
    }

    // ---------- model helpers ----------
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

    /// Drop a tab into a list. `to` is "pinned", "tab" or "fav"; `index` is its position in that list.
    fn move_tab(&mut self, id: &str, to: &str, index: usize) {
        let sp = self.cur_space().clone();
        let Some(from) = self.tabs.iter().position(|t| t.id == id) else { return };
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
        if kind != Kind::Normal { t.folder_id = None; }
        t.space_id = if kind == Kind::Favorite { None } else { Some(sp.id.clone()) };
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

    // ---------- tabs ----------
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
        let rect = content_rect(&self.window, self.p.sidebar_collapsed, self.p.sidebar_compact, self.p.sidebar_width, self.fullscreen);
        let dir = self.root.join("profiles").join(&profile_id);
        let ctx = self
            .contexts
            .entry(profile_id.clone())
            .or_insert_with(|| WebContext::new(Some(dir)));

        let (p1, p2, p3) = (self.proxy.clone(), self.proxy.clone(), self.proxy.clone());
        let (id1, id2) = (id.clone(), id.clone());

        let builder = WebViewBuilder::new_with_web_context(ctx)
            .with_bounds(rect)
            .with_visible(false)
            .with_devtools(false)
            .with_url(&url)
            .with_initialization_script_for_main_only(INIT_JS, false)
            .with_ipc_handler(move |req| {
                let _ = p1.send_event(UserEvent::Page { tab: id1.clone(), body: req.into_body() });
            })
            .with_document_title_changed_handler(move |title| {
                let _ = p2.send_event(UserEvent::Title { tab: id2.clone(), title });
            })
            // target=_blank / window.open -> open as a new tab (OAuth popups need more work).
            .with_new_window_req_handler(move |url, _features| {
                let _ = p3.send_event(UserEvent::OpenTab { url });
                NewWindowResponse::Deny
            });

        #[cfg(windows)]
        let builder = {
            use wry::WebViewBuilderExtWindows;
            builder
                .with_browser_extensions_enabled(true)
                // Keep Wry's defaults while enabling a random loopback CDP port
                // for this isolated profile. The port is read from DevToolsActivePort.
                .with_additional_browser_args(
                    WEBVIEW2_BROWSER_ARGS,
                )
        };

        let wv = builder.build_as_child(&self.window)?;

        // Register on each content WebView as well as the sidebar WebView.
        #[cfg(windows)]
        register_browser_shortcuts(&wv, self.proxy.clone(), id.clone());

        // Use WebView2's original favicon URI so the sidebar and tab rows can
        // render SVGs and multi-resolution ICO/PNG sources without rasterizing
        // every icon to WebView2's small PNG snapshot first.
        #[cfg(windows)]
        {
            use webview2_com::{FaviconChangedEventHandler, Microsoft::Web::WebView2::Win32::*};
            use windows::Win32::System::Com::CoTaskMemFree;
            use windows::core::{Interface, PWSTR};
            use wry::WebViewExtWindows;

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
                        let handler = FaviconChangedEventHandler::create(Box::new(move |sender, _| {
                            let Some(sender) = sender else { return Ok(()) };
                            let Ok(core15) = sender.cast::<ICoreWebView2_15>() else { return Ok(()) };
                            let favicon = current_favicon_uri(&core15);
                            let body = json!({"t": "favicon", "f": favicon}).to_string();
                            let _ = favicon_proxy.send_event(UserEvent::Page { tab: favicon_tab.clone(), body });
                            Ok(())
                        }));
                        let mut token = 0;
                        if let Err(error) = core15.add_FaviconChanged(&handler, &mut token) {
                            log::warn!("registering WebView2 favicon handler failed: {error}");
                        }
                        // The initial navigation may have started before this handler
                        // was attached, so publish the current URI as well.
                        let favicon = current_favicon_uri(&core15);
                        let body = json!({"t": "favicon", "f": favicon}).to_string();
                        let _ = initial_proxy.send_event(UserEvent::Page { tab: initial_tab, body });
                    } else {
                        log::warn!("WebView2 runtime lacks native favicon support (ICoreWebView2_15)");
                    }
                }
            }
        }

        // WebView2 reports HTML fullscreen transitions directly from its native
        // host. This covers fullscreen requests inside cross-origin/nested frames
        // without depending on JavaScript IPC or postMessage frame relays.
        #[cfg(windows)]
        {
            use webview2_com::ContainsFullScreenElementChangedEventHandler;
            use windows::core::BOOL;
            use wry::WebViewExtWindows;

            let fullscreen_proxy = self.proxy.clone();
            let fullscreen_tab = id.clone();
            unsafe {
                match wv.controller().CoreWebView2() {
                    Ok(core) => {
                        let handler = ContainsFullScreenElementChangedEventHandler::create(Box::new(move |sender, _| {
                            let Some(sender) = sender else { return Ok(()) };
                            let mut contains = BOOL::default();
                            if let Err(error) = sender.ContainsFullScreenElement(&mut contains) {
                                log::warn!("reading WebView2 fullscreen state failed: {error}");
                                return Ok(());
                            }
                            let _ = fullscreen_proxy.send_event(UserEvent::Fullscreen {
                                tab: fullscreen_tab.clone(),
                                active: contains.as_bool(),
                            });
                            Ok(())
                        }));
                        let mut token = 0;
                        if let Err(error) = core.add_ContainsFullScreenElementChanged(&handler, &mut token) {
                            log::warn!("registering WebView2 fullscreen handler failed: {error}");
                        }
                    }
                    Err(error) => log::warn!("getting WebView2 core for fullscreen handler failed: {error}"),
                }
            }
        }

        if self.ext_done.insert(profile_id) {
            extensions::load_all(&wv, &self.root.join("extensions"));
        }
        self.tabs[idx].webview = Some(wv);
        Ok(())
    }

    fn leave_fullscreen(&mut self) {
        if self.fullscreen {
            if let Some(wv) = self.visible_webview() {
                let _ = wv.evaluate_script("if (document.fullscreenElement) document.exitFullscreen().catch(()=>{});");
            }
            self.fullscreen = false;
            self.window.set_fullscreen(None);
            let _ = self.sidebar.set_visible(true);
        }
    }

    fn hide_visible(&mut self) {
        self.leave_fullscreen();
        let Some(id) = self.visible.take() else { return };
        if let Some(wv) = self.tabs.iter().find(|t| t.id == id).and_then(|t| t.webview.as_ref()) {
            if self.p.auto_pip {
                pip::on_leave(wv);
            }
            let _ = wv.set_visible(false);
        }
        if let Some(devtools) = self.devtools.as_ref() {
            let _ = devtools.set_visible(false);
        }
    }

    fn activate(&mut self, id: &str) -> Result<()> {
        let Some(idx) = self.tabs.iter().position(|t| t.id == id) else { return Ok(()) };
        if self.tabs[idx].kind == Kind::Normal {
            self.tabs[idx].last_active = SystemTime::now();
        }
        if self.visible.as_deref() == Some(id) && !self.overlay {
            return Ok(());
        }
        self.hide_visible();
        self.ensure_webview(idx)?;
        let (rect, _) = self.layout_rects();
        self.overlay = false;
        if let Some(wv) = &self.tabs[idx].webview {
            apply_webview_bounds(wv, rect, self.window.scale_factor())?;
            #[cfg(windows)]
            {
                let (window_w, window_h) = logical(&self.window);
                let left = if self.p.sidebar_collapsed { 0.0 } else if self.p.sidebar_compact { SIDEBAR_COMPACT_W } else { self.p.sidebar_width };
                apply_webview_radius(
                    wv,
                    window_w - left - 2.0 * INSET,
                    window_h - TOPBAR_H - INSET,
                    self.window.scale_factor(),
                    !self.fullscreen && !self.devtools_open,
                );
            }
            wv.set_visible(true)?;
            wv.focus()?;
            if self.p.auto_pip {
                pip::on_enter(wv);
            }
        }
        self.visible = Some(id.to_string());
        self.active.insert(self.p.current_space.clone(), id.to_string());
        if self.devtools_open {
            self.attach_devtools()?;
        }
        Ok(())
    }

    fn close_tab(&mut self, id: &str) -> Result<()> {
        let Some(idx) = self.tabs.iter().position(|t| t.id == id) else { return Ok(()) };
        let was_visible = self.visible.as_deref() == Some(id);
        if was_visible {
            self.leave_fullscreen();
            self.visible = None;
            if let Some(wv) = &self.tabs[idx].webview {
                let _ = wv.set_visible(false);
            }
            if let Some(devtools) = self.devtools.as_ref() {
                let _ = devtools.set_visible(false);
            }
        }
        if self.tabs[idx].kind == Kind::Normal {
            self.closed_tabs.insert(0, (self.tabs[idx].url.clone(), self.tabs[idx].space_id.clone().unwrap_or_else(|| self.p.current_space.clone())));
            self.closed_tabs.truncate(25);
            self.tabs.remove(idx); // drops the WebView
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
        self.save();
        Ok(())
    }

    fn reopen_closed_tab(&mut self) -> Result<()> {
        let Some((url, _space)) = self.closed_tabs.first().cloned() else { return Ok(()) };
        self.closed_tabs.remove(0);
        self.new_tab(&url)
    }

    fn cycle_tab(&mut self, direction: i32) -> Result<()> {
        let ids: Vec<String> = self.tabs.iter()
            .filter(|t| t.space_id.as_deref() == Some(self.p.current_space.as_str()) || t.kind == Kind::Favorite)
            .map(|t| t.id.clone()).collect();
        if ids.len() < 2 { return Ok(()); }
        let current = self.visible.as_ref().and_then(|id| ids.iter().position(|x| x == id)).unwrap_or(0) as i32;
        let next = (current + direction).rem_euclid(ids.len() as i32) as usize;
        self.activate(&ids[next])
    }

    fn activate_tab_index(&mut self, index: usize) -> Result<()> {
        let ids: Vec<String> = self.tabs.iter()
            .filter(|t| t.space_id.as_deref() == Some(self.p.current_space.as_str()) || t.kind == Kind::Favorite)
            .map(|t| t.id.clone()).collect();
        if ids.is_empty() { return Ok(()); }
        let target = if index == usize::MAX { ids.len() - 1 } else { index.min(ids.len() - 1) };
        self.activate(&ids[target])
    }

    // ---------- spaces ----------
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
            self.devtools_open = false;
            self.devtools_tab = None;
            self.relayout();
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
        let cur = self.p.spaces.iter().position(|s| s.id == self.p.current_space).unwrap_or(0) as i32;
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
        self.active.remove(id);
        if self.p.current_space == id {
            self.p.current_space = self.p.spaces[0].id.clone();
            self.restore_space_tab()?;
        }
        self.save();
        Ok(())
    }


    /// Expire inactive normal tabs and apply profile-level browsing-data policies.
    pub fn cleanup_expired_tabs(&mut self) {
        let now = SystemTime::now();
        let expired: Vec<String> = self.tabs.iter()
            .filter(|t| t.kind == Kind::Normal && self.visible.as_deref() != Some(t.id.as_str()))
            .filter(|t| {
                let hours = self.p.profiles.iter()
                    .find(|p| p.id == t.profile_id)
                    .map(|p| p.tab_retention_hours)
                    .unwrap_or(48);
                now.duration_since(t.last_active).unwrap_or(Duration::ZERO).as_secs()
                    >= hours.saturating_mul(3600)
            })
            .map(|t| t.id.clone())
            .collect();
        for id in expired {
            if let Err(e) = self.close_tab(&id) {
                log::warn!("auto-tidy could not close tab: {e:#}");
            }
        }
        self.cleanup_profile_data();
        self.push_state();
    }

    #[cfg(windows)]
    fn cleanup_profile_data(&mut self) {
        use webview2_com::{
            ClearBrowsingDataCompletedHandler,
            Microsoft::Web::WebView2::Win32::{
                ICoreWebView2_13, ICoreWebView2Profile2,
                COREWEBVIEW2_BROWSING_DATA_KINDS_COOKIES,
                COREWEBVIEW2_BROWSING_DATA_KINDS_DISK_CACHE,
                COREWEBVIEW2_BROWSING_DATA_KINDS_BROWSING_HISTORY,
            },
        };
        use windows::core::Interface;

        let now = SystemTime::now().duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO).as_secs();
        let due_profiles: Vec<String> = self.p.profiles.iter()
            .filter(|p| p.cookie_retention_hours > 0 || p.cache_retention_hours > 0 || p.history_retention_hours > 0)
            .filter(|p| now.saturating_sub(p.last_cleanup_at) >= 3600)
            .map(|p| p.id.clone())
            .collect();

        for profile_id in due_profiles {
            let Some(webview) = self.tabs.iter()
                .find(|t| t.profile_id == profile_id && t.webview.is_some())
                .and_then(|t| t.webview.as_ref())
            else { continue };

            let settings = self.p.profiles.iter()
                .find(|p| p.id == profile_id)
                .map(|p| (p.cookie_retention_hours, p.cache_retention_hours, p.history_retention_hours))
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
                        (settings.2, COREWEBVIEW2_BROWSING_DATA_KINDS_BROWSING_HISTORY),
                    ] {
                        if hours == 0 { continue; }
                        let cutoff = now.saturating_sub(hours.saturating_mul(3600)) as f64;
                        let handler = ClearBrowsingDataCompletedHandler::create(Box::new(|_| Ok(())));
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

    #[cfg(not(windows))]
    fn cleanup_profile_data(&mut self) {
        // Age-scoped profile cleanup is exposed through WebView2 on Windows.
    }

    // ---------- output ----------
    fn layout_rects(&self) -> (Rect, Option<Rect>) {
        let base = content_rect(
            &self.window,
            self.p.sidebar_collapsed,
            self.p.sidebar_compact,
            self.p.sidebar_width,
            self.fullscreen,
        );
        if !self.devtools_open || self.fullscreen {
            return (base, None);
        }
        let (window_w, window_h) = logical(&self.window);
        let sidebar = if self.p.sidebar_collapsed {
            0.0
        } else if self.p.sidebar_compact {
            SIDEBAR_COMPACT_W
        } else {
            self.p.sidebar_width
        };
        let x = sidebar + INSET;
        let y = TOPBAR_H;
        let width = (window_w - sidebar - 2.0 * INSET).max(1.0);
        let height = (window_h - TOPBAR_H - INSET).max(1.0);
        let gap = 8.0;
        let size = self.p.devtools_size.clamp(240.0, 800.0);
        let position = self.p.devtools_position.as_str();
        let (page, _tools) = match position {
            "left" => {
                let tools_w = size.min((width - gap - 1.0).max(1.0));
                (rect(x + tools_w + gap, y, (width - tools_w - gap).max(1.0), height),
                 rect(x, y, tools_w, height))
            }
            "bottom" => {
                let tools_h = size.min((height - gap - 1.0).max(1.0));
                (rect(x, y, width, (height - tools_h - gap).max(1.0)),
                 rect(x, y + height - tools_h, width, tools_h))
            }
            "top" => {
                let tools_h = size.min((height - gap - 1.0).max(1.0));
                (rect(x, y + tools_h + gap, width, (height - tools_h - gap).max(1.0)),
                 rect(x, y, width, tools_h))
            }
            _ => {
                let tools_w = size.min((width - gap - 1.0).max(1.0));
                (rect(x, y, (width - tools_w - gap).max(1.0), height),
                 rect(x + width - tools_w, y, tools_w, height))
            }
        };
        let page = if let Some((preview_x, preview_y, preview_width, preview_height)) = self.devtools_page_bounds {
            let preview_x = preview_x.clamp(0.0, (width - 1.0).max(0.0));
            let preview_y = preview_y.clamp(0.0, (height - 1.0).max(0.0));
            let preview_width = preview_width.min(width - preview_x).max(1.0);
            let preview_height = preview_height.min(height - preview_y).max(1.0);
            rect(x + preview_x, y + preview_y, preview_width, preview_height)
        } else {
            page
        };
        (page, Some(base))
    }

    pub fn relayout(&self) {
        let (w, h) = logical(&self.window);
        if w < 1.0 || h < 1.0 {
            return; // minimized
        }
        let _ = self.sidebar.set_bounds(full_rect(&self.window));
        let _ = self.sidebar.set_visible(!self.fullscreen);
        let (page_bounds, devtools_bounds) = self.layout_rects();
        if let Some(wv) = self.visible_webview() {
            if let Err(error) = apply_webview_bounds(wv, page_bounds, self.window.scale_factor()) {
                log::warn!("resizing content WebView failed: {error:#}");
            }
            #[cfg(windows)]
            {
                let left = if self.p.sidebar_collapsed { 0.0 } else if self.p.sidebar_compact { SIDEBAR_COMPACT_W } else { self.p.sidebar_width };
                apply_webview_radius(
                    wv,
                    w - left - 2.0 * INSET,
                    h - TOPBAR_H - INSET,
                    self.window.scale_factor(),
                    !self.fullscreen && !self.devtools_open,
                );
            }
        }
        if let Some(devtools) = self.devtools.as_ref() {
            if let Some(bounds) = devtools_bounds {
                if let Err(error) = apply_webview_bounds(devtools, bounds, self.window.scale_factor()) {
                    log::warn!("resizing DevTools WebView failed: {error:#}");
                }
                let _ = devtools.set_visible(self.devtools_open && !self.fullscreen && !self.overlay);
            } else {
                let _ = devtools.set_visible(false);
            }
        }
        #[cfg(windows)]
        if self.devtools_open {
            if let Some(wv) = self.visible_webview() {
                raise_webview(wv);
            }
        }
    }

    pub fn save(&mut self) {
        self.p.saved = self
            .tabs
            .iter()
            .filter(|t| t.kind != Kind::Normal)
            .map(|t| Saved {
                id: t.id.clone(),
                profile_id: t.profile_id.clone(),
                space_id: t.space_id.clone(),
                kind: t.kind,
                folder_id: t.folder_id.clone(),
                url: t.home.clone(),
                title: t.title.clone(),
                favicon: t.favicon.clone(),
            })
            .collect();
        self.p.save(&self.root.join("state.json"));
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
                .and_then(|x| x.host_str().map(|h| h.trim_start_matches("www.").to_string()))
                .unwrap_or_default();
            if seen.insert(host) {
                out.push(json!({"label": name, "url": u, "favicon": null, "visits": 0, "popular": true}));
            }
        }
        out
    }

    /// If the visible tab is a Chrome Web Store detail page, the extension's ID.
    fn store_ext(&self) -> Option<String> {
        let t = self.visible.as_ref().and_then(|id| self.tabs.iter().find(|t| &t.id == id))?;
        let u = url::Url::parse(&t.url).ok()?;
        match u.host_str()? {
            "chromewebstore.google.com" | "chrome.google.com" if u.path().contains("/detail/") => store::parse_id(u.path()),
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
            .tabs.iter()
            .filter(|t| t.kind == Kind::Favorite && t.profile_id == sp.profile_id)
            .map(row).collect();
        let in_space = |k: Kind| -> Vec<_> {
            self.tabs.iter()
                .filter(|t| t.kind == k && t.space_id.as_deref() == Some(sp.id.as_str()))
                .map(row).collect()
        };
        let url = self
            .visible
            .as_ref()
            .and_then(|id| self.tabs.iter().find(|t| &t.id == id))
            .map(|t| t.url.clone())
            .unwrap_or_default();

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
            "suggestions": self.suggestions(),
            "store_ext": self.store_ext(),
            "theme": self.p.theme,
            "auto_pip": self.p.auto_pip,
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
        let _ = self.sidebar.evaluate_script(&format!("window.__setState({state})"));
    }
}

#[cfg(test)]
mod shortcut_tests {
    use super::browser_shortcut;

    #[test]
    fn favicon_updates_preserve_existing_icon_on_empty_or_invalid_results() {
        let original = "data:image/png;base64,aGVsbG8=".to_string();
        let mut current = Some(original.clone());

        assert!(!super::update_favicon(&mut current, None));
        assert_eq!(current.as_deref(), Some(original.as_str()));

        assert!(!super::update_favicon(&mut current, Some("javascript:alert(1)".into())));
        assert_eq!(current.as_deref(), Some(original.as_str()));
    }

    #[test]
    fn favicon_updates_replace_icon_only_when_a_new_valid_value_arrives() {
        let original = "data:image/png;base64,aGVsbG8=".to_string();
        let replacement = "https://example.test/favicon.png".to_string();
        let mut current = Some(original);

        assert!(!super::update_favicon(&mut current, Some("data:image/png;base64,aGVsbG8=".into())));
        assert!(super::update_favicon(&mut current, Some(replacement.clone())));
        assert_eq!(current.as_deref(), Some(replacement.as_str()));
    }

    #[test]
    fn maps_common_browser_shortcuts() {
        assert_eq!(browser_shortcut(0x54, true, false, false), Some("new_tab"));
        assert_eq!(browser_shortcut(0x57, true, false, false), Some("close"));
        assert_eq!(browser_shortcut(0x4C, true, false, false), Some("focus_url"));
        assert_eq!(browser_shortcut(0x54, true, false, true), Some("reopen"));
        assert_eq!(browser_shortcut(0x52, true, false, true), Some("reload"));
        assert_eq!(browser_shortcut(0x09, true, false, false), Some("tab_next"));
        assert_eq!(browser_shortcut(0x09, true, false, true), Some("tab_prev"));
        assert_eq!(browser_shortcut(0x31, true, false, false), Some("tab_1"));
        assert_eq!(browser_shortcut(0x39, true, false, false), Some("tab_last"));
        assert_eq!(browser_shortcut(0x7B, false, false, false), Some("devtools_toggle"));
        assert_eq!(browser_shortcut(0x7B, true, false, false), None);
        assert_eq!(browser_shortcut(0x49, true, false, true), Some("devtools_toggle"));
        assert_eq!(browser_shortcut(0x49, true, false, false), None);
        assert_eq!(browser_shortcut(0x49, true, true, true), None);
        assert_eq!(browser_shortcut(0x25, false, true, false), Some("back"));
        assert_eq!(browser_shortcut(0x27, false, true, false), Some("forward"));
        assert_eq!(browser_shortcut(0x25, true, true, false), Some("space_prev"));
        assert_eq!(browser_shortcut(0x27, true, true, false), Some("space_next"));
    }

    #[test]
    fn leaves_unhandled_and_unmodified_keys_alone() {
        assert_eq!(browser_shortcut(0x41, false, false, false), None);
        assert_eq!(browser_shortcut(0x46, true, false, false), None);
        assert_eq!(browser_shortcut(0x25, false, false, false), None);
        assert_eq!(browser_shortcut(0x54, true, true, false), None);
    }
}


#[cfg(test)]
mod devtools_frontend_tests {
    use super::devtools_frontend_url;

    #[test]
    fn enables_devtools_websocket_origins_for_the_embedded_frontend() {
        assert!(super::WEBVIEW2_BROWSER_ARGS.contains("--remote-allow-origins=*"));
        assert!(super::WEBVIEW2_BROWSER_ARGS.contains("--remote-debugging-address=127.0.0.1"));
        assert!(super::WEBVIEW2_BROWSER_ARGS.contains("--remote-debugging-port=0"));
    }

    #[test]
    fn builds_a_runtime_bundled_devtools_url_for_the_exact_page_target() {
        assert_eq!(
            devtools_frontend_url(43123, "A1B2C3").unwrap(),
            "devtools://devtools/bundled/inspector.html?ws=127.0.0.1:43123/devtools/page/A1B2C3"
        );
    }

    #[test]
    fn devtools_startup_script_uses_the_configured_dock_and_panel_size() {
        let script = super::devtools_init_script("left", 560.0);
        assert!(script.contains("JSON.stringify(\"left\")"));
        assert!(script.contains("vertical: {size: 560}"));
        assert!(script.contains("horizontal: {size: 560}"));
        assert!(script.contains("setInspectedPageBounds"));
        assert!(script.contains("shadow-split-widget-sidebar"));
    }

    #[test]
    fn devtools_startup_script_maps_top_docking_to_a_top_sidebar() {
        let script = super::devtools_init_script("top", 420.0);
        assert!(script.contains("JSON.stringify(\"bottom\")"));
        assert!(script.contains("setSecondIsSidebar(false)"));
        assert!(script.contains("shadow-split-widget-sidebar"));
    }

    #[test]
    fn rejects_invalid_devtools_endpoint_values() {
        assert!(devtools_frontend_url(0, "A1B2C3").is_err());
        assert!(devtools_frontend_url(43123, "").is_err());
        assert!(devtools_frontend_url(43123, "abc/other").is_err());
        assert!(devtools_frontend_url(43123, "abc?ws=evil").is_err());
    }
}
