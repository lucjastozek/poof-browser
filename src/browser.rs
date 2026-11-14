use crate::{
    extensions, pip,
    state::{new_id, valid_icon, Kind, Persisted, Profile, Saved, Space},
    store, UserEvent,
};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
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
fn apply_webview_radius(wv: &WebView, width: f64, height: f64, scale: f64, rounded: bool) {
    use windows::Win32::{
        Foundation::HWND,
        Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, HGDIOBJ, HRGN, SetWindowRgn},
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

const SIDEBAR_W: f64 = 280.0;
const SIDEBAR_COMPACT_W: f64 = 56.0;
const INSET: f64 = 8.0;
const TOPBAR_H: f64 = 44.0;

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
const INIT_JS: &str = r#"(function(){
  function post(o){ try { window.ipc.postMessage(JSON.stringify(o)); } catch(e){} }
  // Only the top document controls the native window. Fullscreen changes from
  // embedded frames propagate to their iframe element in the parent document.
  // Keep the video player's F shortcut available even before its controls are clicked.
  // This runs in every frame because embedded players commonly own their video element.
  function editableTarget(el){
    return !!(el && (el.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName)));
  }
  function requestPlayerFullscreen(){
    if (document.fullscreenElement) {
      document.exitFullscreen().catch(function(){});
      return true;
    }
    var videos = Array.prototype.slice.call(document.querySelectorAll('video'))
      .filter(function(v){ return v.readyState > 0 && v.videoWidth > 0; });
    if (videos.length) {
      videos.sort(function(a,b){ return (b.videoWidth*b.videoHeight)-(a.videoWidth*a.videoHeight); });
      var v = videos[0];
      var target = v.closest('.html5-video-player, .vp-player, .jwplayer, [data-testid*="player"]') || v.parentElement || v;
      if (target.requestFullscreen) {
        target.requestFullscreen().catch(function(){});
        return true;
      }
    }
    return false;
  }
  addEventListener('keydown', function(e){
    if (!e.isTrusted || e.defaultPrevented || e.repeat || e.ctrlKey || e.altKey || e.metaKey ||
        e.shiftKey || e.key.toLowerCase() !== 'f' || editableTarget(e.target)) return;
    if (requestPlayerFullscreen()) {
      e.preventDefault();
      e.stopPropagation();
      return;
    }
    // If focus is in the top document rather than a cross-origin player frame,
    // fullscreen the embedded player frame from this trusted key event.
    if (window.top === window) {
      var frames = Array.prototype.slice.call(document.querySelectorAll('iframe'))
        .filter(function(frame){
          var src = (frame.getAttribute('src') || '').toLowerCase();
          return /youtube|youtu\.be|vimeo|dailymotion|twitch/.test(src) &&
            frame.allowFullscreen && frame.requestFullscreen;
        });
      if (frames.length) {
        frames[0].requestFullscreen().catch(function(){});
        e.preventDefault();
        e.stopPropagation();
      }
    }
  }, true);
  // Make the fullscreen element occupy the whole display. For a player
  // container, center its video inside the black viewport; for a video that
  // enters fullscreen directly, use the viewport as its contain box.
  // Inject this in every frame because third-party players are often cross-origin.
  var fullscreenStyle = document.createElement('style');
  fullscreenStyle.textContent =
    ':fullscreen{background:#000!important}' +
    ':fullscreen.html5-video-player,:fullscreen .vp-player,:fullscreen .jwplayer,:fullscreen [data-testid*="player"]{' +
      'width:100vw!important;height:100vh!important;max-width:none!important;max-height:none!important;' +
      'display:flex!important;align-items:center!important;justify-content:center!important;overflow:hidden!important}' +
    ':fullscreen video{position:relative!important;inset:auto!important;margin:auto!important;width:auto!important;height:auto!important;max-width:100vw!important;max-height:100vh!important;object-fit:contain!important}' +
    'video:fullscreen{width:100vw!important;height:100vh!important;max-width:none!important;max-height:none!important;object-fit:contain!important;background:#000!important}';
  (document.head || document.documentElement).appendChild(fullscreenStyle);
  // Fullscreen in a cross-origin iframe is isolated from the top document.
  // Relay its state up the frame tree with postMessage; only the top document
  // talks to native code. This is the signal that makes the APP window itself
  // enter OS-level borderless fullscreen, rather than merely filling the WebView.
  var fullscreenRelay = '__poof_native_fullscreen__';
  document.addEventListener('fullscreenchange', function(){
    var active = !!document.fullscreenElement;
    if (window.top === window) {
      post({t:'fullscreen', active:active});
    } else {
      try { window.parent.postMessage({type:fullscreenRelay, active:active}, '*'); } catch(e){}
    }
  }, true);
  if (window.top !== window) {
    addEventListener('message', function(e){
      if (!e.data || e.data.type !== fullscreenRelay) return;
      if (window.parent !== window) {
        try { window.parent.postMessage(e.data, '*'); } catch(err){}
      } else {
        post({t:'fullscreen', active:!!e.data.active});
      }
    }, false);
    return;
  }
  // A player may forward a shortcut/fullscreen request from a nested frame.
  addEventListener('message', function(e){
    if (e.data && e.data.type === fullscreenRelay) {
      post({t:'fullscreen', active:!!e.data.active});
    }
  }, false);
  var scrollbarStyle = document.createElement('style');
  scrollbarStyle.textContent = 'html{scrollbar-width:thin!important;scrollbar-color:color-mix(in srgb,currentColor 24%,transparent) transparent!important}::-webkit-scrollbar{width:6px!important;height:6px!important}::-webkit-scrollbar-track{background:transparent!important}::-webkit-scrollbar-thumb{background:color-mix(in srgb,currentColor 28%,transparent)!important;border-radius:99px!important;border:2px solid transparent!important;background-clip:padding-box!important}::-webkit-scrollbar-thumb:hover{background:color-mix(in srgb,currentColor 42%,transparent)!important;background-clip:padding-box!important}::-webkit-scrollbar-button{display:none!important;width:0!important;height:0!important}';
  (document.head || document.documentElement).appendChild(scrollbarStyle);
  var last = '';
  function icon(){
    try {
      var l = document.querySelector('link[rel~="icon" i]');
      return new URL(l ? l.getAttribute('href') : '/favicon.ico', location.href).href;
    } catch(e){ return null; }
  }
  function meta(force){
    var u = location.href, f = icon(), k = u + '|' + f;
    if (!force && k === last) return;
    last = k; post({t:'meta', u:u, f:f});
  }
  addEventListener('DOMContentLoaded', function(){ meta(true); });
  addEventListener('load', function(){ meta(true); });
  setInterval(function(){ meta(false); }, 1500);
  addEventListener('keydown', function(e){
    if (!e.isTrusted || !e.ctrlKey) return;
    var key = e.key.toLowerCase(), k = null;
    if (e.altKey && e.key === 'ArrowRight') k = 'space_next';
    else if (e.altKey && e.key === 'ArrowLeft') k = 'space_prev';
    else if (!e.altKey && !e.shiftKey) {
      if (key === 't') k = 'new_tab'; else if (key === 'w') k = 'close'; else if (key === 'l') k = 'focus_url';
    }
    if (k) { e.preventDefault(); post({t:'key', k:k}); }
  }, true);
})();"#;

pub struct Tab {
    pub id: String,
    pub profile_id: String,
    pub space_id: Option<String>, // None for favorites
    pub kind: Kind,
    pub home: String, // URL a pinned/favorite resets to when closed
    pub url: String,
    pub host: String, // last host seen (for visit stats)
    pub title: String,
    pub favicon: Option<String>,
    pub webview: Option<WebView>, // None = not loaded yet (lazy)
}

#[derive(Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum Cmd {
    Ready,
    SidebarCollapsed { collapsed: bool },
    SidebarCompact { compact: bool },
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
    SetTheme { theme: String },
    SetAutoPip { on: bool },
    DeleteSpace { id: String },
    ListExt,
    InstallExt { input: String },
    RemoveExt { dir: String },
    Back,
    Forward,
    Reload,
}

#[derive(Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
enum PageMsg {
    Meta { u: Option<String>, f: Option<String> },
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
fn content_rect(w: &Window, collapsed: bool, compact: bool, fullscreen: bool) -> Rect {
    let (ww, hh) = logical(w);
    if fullscreen {
        return full_rect(w);
    }
    let left = if collapsed { 0.0 } else if compact { SIDEBAR_COMPACT_W } else { SIDEBAR_W };
    rect(left + INSET, TOPBAR_H + INSET, ww - left - 2.0 * INSET, hh - TOPBAR_H - 2.0 * INSET)
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

fn ok_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn clean_icon(s: &str) -> String {
    if valid_icon(s) { s.to_string() } else { "house".into() }
}

fn clip(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
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
            .replace("/*THEME*/", include_str!("../ui/theme.css"))
            .replace("/*ICONS*/", include_str!("../ui/icons.js"));
        let mut ui_ctx = WebContext::new(Some(root.join("ui")));
        let ptx = proxy.clone();
        let sidebar = WebViewBuilder::new_with_web_context(&mut ui_ctx)
            .with_bounds(full_rect(&window))
            .with_html(html)
            .with_ipc_handler(move |req| {
                let _ = ptx.send_event(UserEvent::Ui(req.into_body()));
            })
            .build_as_child(&window)?;

        let tabs = p
            .saved
            .iter()
            .map(|s| Tab {
                id: s.id.clone(),
                profile_id: s.profile_id.clone(),
                space_id: s.space_id.clone(),
                kind: s.kind,
                home: s.url.clone(),
                url: s.url.clone(),
                host: String::new(),
                title: s.title.clone(),
                favicon: s.favicon.clone(),
                webview: None,
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
            UserEvent::Ui(raw) => self.handle_ui(&raw),
            UserEvent::Page { tab, body } => self.handle_page(&tab, &body),
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

    /// Messages from web pages are untrusted: strict parsing, size limits, own-tab only.
    fn handle_page(&mut self, tab: &str, body: &str) {
        if body.len() > 20_000 {
            return;
        }
        let Ok(msg) = serde_json::from_str::<PageMsg>(body) else { return };
        match msg {
            PageMsg::Meta { u, f } => {
                let mut persist = false;
                if let Some(t) = self.tabs.iter_mut().find(|t| t.id == tab) {
                    t.favicon = f.filter(|f| ok_url(f, 16_000, true));
                    if let Some(u) = u.filter(|u| ok_url(u, 4096, false)) {
                        t.url = u.clone();
                        if let Ok(parsed) = url::Url::parse(&u) {
                            if let Some(h) = parsed.host_str() {
                                let host = h.trim_start_matches("www.").to_string();
                                if t.host != host {
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
            }
            PageMsg::Fullscreen { active } => {
                if self.visible.as_deref() == Some(tab) && self.fullscreen != active {
                    self.fullscreen = active;
                    self.window.set_fullscreen(if active {
                        Some(tao::window::Fullscreen::Borderless(None))
                    } else {
                        None
                    });
                    let _ = self.sidebar.set_visible(!active);
                    self.relayout();
                }
            }
            PageMsg::Key { k } => {
                if self.visible.as_deref() != Some(tab) {
                    return;
                }
                let r = match k.as_str() {
                    "new_tab" => {
                        let _ = self.sidebar.focus();
                        let _ = self.sidebar.evaluate_script("window.__openNew && window.__openNew()");
                        Ok(())
                    }
                    "focus_url" => {
                        let _ = self.sidebar.focus();
                        let _ = self.sidebar.evaluate_script("window.__focusUrl && window.__focusUrl()");
                        Ok(())
                    }
                    "close" => self.close_tab(tab),
                    "space_next" => self.cycle_space(1),
                    "space_prev" => self.cycle_space(-1),
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
            Cmd::Ready => {}
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
                } else if let Some(wv) = self.visible_webview() {
                    let _ = wv.set_visible(true);
                    let _ = wv.focus();
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
                        self.p.profiles.push(Profile { id: id.clone(), name: n });
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
        }
        Ok(())
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
        if t.kind != kind {
            t.home = t.url.clone();
        }
        t.kind = kind;
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
            home: url.to_string(),
            url: url.to_string(),
            host: String::new(),
            title: String::new(),
            favicon: None,
            webview: None,
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
        let rect = content_rect(&self.window, self.p.sidebar_collapsed, self.p.sidebar_compact, self.fullscreen);
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
            .with_url(&url)
            .with_initialization_script(INIT_JS)
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
            builder.with_browser_extensions_enabled(true)
        };

        let wv = builder.build_as_child(&self.window)?;

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
    }

    fn activate(&mut self, id: &str) -> Result<()> {
        let Some(idx) = self.tabs.iter().position(|t| t.id == id) else { return Ok(()) };
        if self.visible.as_deref() == Some(id) && !self.overlay {
            return Ok(());
        }
        self.hide_visible();
        self.ensure_webview(idx)?;
        let rect = content_rect(&self.window, self.p.sidebar_collapsed, self.p.sidebar_compact, self.fullscreen);
        self.overlay = false;
        if let Some(wv) = &self.tabs[idx].webview {
            wv.set_bounds(rect)?;
            #[cfg(windows)]
            {
                let (window_w, window_h) = logical(&self.window);
                let left = if self.p.sidebar_collapsed { 0.0 } else if self.p.sidebar_compact { SIDEBAR_COMPACT_W } else { SIDEBAR_W };
                apply_webview_radius(
                    wv,
                    window_w - left - 2.0 * INSET,
                    window_h - TOPBAR_H - 2.0 * INSET,
                    self.window.scale_factor(),
                    !self.fullscreen,
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
        }
        if self.tabs[idx].kind == Kind::Normal {
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

    // ---------- output ----------
    pub fn relayout(&self) {
        let (w, h) = logical(&self.window);
        if w < 1.0 || h < 1.0 {
            return; // minimized
        }
        let _ = self.sidebar.set_bounds(full_rect(&self.window));
        let _ = self.sidebar.set_visible(!self.fullscreen);
        if let Some(wv) = self.visible_webview() {
            let _ = wv.set_bounds(content_rect(&self.window, self.p.sidebar_collapsed, self.p.sidebar_compact, self.fullscreen));
            #[cfg(windows)]
            {
                let left = if self.p.sidebar_collapsed { 0.0 } else if self.p.sidebar_compact { SIDEBAR_COMPACT_W } else { SIDEBAR_W };
                apply_webview_radius(
                    wv,
                    w - left - 2.0 * INSET,
                    h - TOPBAR_H - 2.0 * INSET,
                    self.window.scale_factor(),
                    !self.fullscreen,
                );
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
                "favicon": t.favicon, "loaded": t.webview.is_some()
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
            "suggestions": self.suggestions(),
            "store_ext": self.store_ext(),
            "theme": self.p.theme,
            "auto_pip": self.p.auto_pip,
            "sidebar_collapsed": self.p.sidebar_collapsed,
            "sidebar_compact": self.p.sidebar_compact,
        });
        let _ = self.sidebar.evaluate_script(&format!("window.__setState({state})"));
    }
}
