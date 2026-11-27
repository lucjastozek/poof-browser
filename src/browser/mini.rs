//! Little Arc (a small chrome-less window for links from other apps and quick searches) and
//! Peek (a link preview over the current page). Both are off until turned on in settings.

use super::popup::{popup_bar_html, Popup, BAR_HEIGHT};
use super::*;
use tao::{dpi::LogicalSize, event_loop::EventLoopWindowTarget};

const MINI_SIZE: (f64, f64) = (420.0, 640.0);
const DIM_JS: &str = r#"function (r) {
  var d = document.getElementById("__poofPeekDim");
  if (!r) { if (d) d.remove(); return; }
  if (!d) {
    d = document.createElement("div");
    d.id = "__poofPeekDim";
    d.style.cssText = "position:fixed;inset:0;z-index:2147483647;background:rgba(0,0,0,.45);backdrop-filter:blur(2px)";
    var c = document.createElement("div");
    c.style.cssText = "position:absolute;border-radius:12px;box-shadow:0 24px 70px rgba(0,0,0,.6),0 0 0 1px rgba(255,255,255,.1)";
    d.appendChild(c);
    d.addEventListener("pointerdown", function (e) {
      e.preventDefault(); e.stopPropagation();
      try { window.ipc.postMessage(JSON.stringify({ t: "peek_close" })); } catch (x) {}
    }, true);
    (document.body || document.documentElement).appendChild(d);
  }
  var c = d.firstChild;
  c.style.left = r[0] + "px"; c.style.top = r[1] + "px";
  c.style.width = r[2] + "px"; c.style.height = r[3] + "px";
}"#;
const MINI_MIN: (f64, f64) = (320.0, 240.0);
/// Height of the peek overlay's bar.
const PEEK_BAR: f64 = 36.0;
/// Corner radius of the overlay card, matching the page's own rounded corners.
const PEEK_RADIUS: f64 = 12.0;

/// Rounds the outer corners of a WebView: the top two for the bar, the bottom two for the page.
#[cfg(windows)]
fn round_corners(wv: &WebView, width: f64, height: f64, scale: f64, top: bool) {
    use windows::Win32::{
        Foundation::HWND,
        Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, SetWindowRgn, HGDIOBJ},
    };
    let mut hwnd = HWND::default();
    unsafe {
        if wv.controller().ParentWindow(&mut hwnd).is_err() || hwnd.0.is_null() {
            return;
        }
        let w = (width * scale).round().max(1.0) as i32;
        let h = (height * scale).round().max(1.0) as i32;
        let r = (PEEK_RADIUS * scale).round() as i32 * 2;
        // The rectangle runs past the edge that stays square, so only the other corners curve.
        let region = if top {
            CreateRoundRectRgn(0, 0, w + 1, h + r + 1, r, r)
        } else {
            CreateRoundRectRgn(0, -r, w + 1, h + 1, r, r)
        };
        if region.0.is_null() {
            return;
        }
        if SetWindowRgn(hwnd, Some(region), true) == 0 {
            let _ = DeleteObject(HGDIOBJ(region.0));
        }
    }
}
#[cfg(not(windows))]
fn round_corners(_: &WebView, _: f64, _: f64, _: f64, _: bool) {}

/// The link-peek overlay: a bar and a page, drawn over the page that was showing.
pub(super) struct Peek {
    bar: WebView,
    page: WebView,
    /// The tab the overlay sits on; its page is dimmed while the overlay is open.
    tab: String,
}

pub(super) fn peek_script(on: bool) -> String {
    PEEK_JS.replace("__ON__", if on { "true" } else { "false" })
}

fn bar_ipc(
    proxy: EventLoopProxy<UserEvent>,
    make: impl Fn(String) -> UserEvent + 'static,
) -> impl Fn(wry::http::Request<String>) + 'static {
    move |req| {
        let cmd = serde_json::from_str::<Value>(req.body())
            .ok()
            .and_then(|v| v["cmd"].as_str().map(str::to_string));
        if let Some(cmd) = cmd {
            let _ = proxy.send_event(make(cmd));
        }
    }
}

impl Browser {
    /// Whether a link from another app should open in the mini window.
    pub(super) fn wants_mini(&self, url: &str) -> bool {
        self.p.mini_window
            && url
                .get(..4)
                .is_some_and(|head| head.eq_ignore_ascii_case("http"))
    }

    /// Opens the mini window; with an empty address it starts as a quick search.
    pub(crate) fn open_mini_window(
        &mut self,
        url: &str,
        target: &EventLoopWindowTarget<UserEvent>,
    ) {
        if !self.p.mini_window {
            return;
        }
        if let Err(error) = self.build_mini(url, target) {
            log::warn!("mini window failed: {error:#}");
            if ok_url(url, 4096, false) {
                let _ = self.new_tab(url);
            }
        }
    }

    fn build_mini(&mut self, url: &str, target: &EventLoopWindowTarget<UserEvent>) -> Result<()> {
        use tao::window::WindowBuilder;
        let size = LogicalSize::new(MINI_SIZE.0, MINI_SIZE.1 + BAR_HEIGHT);
        let window = WindowBuilder::new()
            .with_title("poof")
            .with_decorations(false)
            .with_always_on_top(true)
            .with_window_icon(crate::app_icon::window_icon())
            .with_inner_size(size)
            .with_min_inner_size(LogicalSize::new(MINI_MIN.0, MINI_MIN.1 + BAR_HEIGHT))
            .build(target)?;
        let id = window.id();

        let html = popup_bar_html(&self.p.theme, &self.cur_space().color, "", "mini");
        let bar_ipc = bar_ipc(self.proxy.clone(), move |cmd| UserEvent::PopupCmd {
            window: id,
            cmd,
        });
        let bar_builder = WebViewBuilder::new_with_web_context(&mut self.ui_ctx)
            .with_bounds(rect(0.0, 0.0, size.width, BAR_HEIGHT))
            .with_devtools(false)
            .with_html(html)
            .with_ipc_handler(bar_ipc);
        #[cfg(windows)]
        let bar_builder = {
            use wry::WebViewBuilderExtWindows;
            bar_builder.with_additional_browser_args(WEBVIEW2_UI_ARGS)
        };
        let bar = bar_builder.build_as_child(&window)?;

        let profile_id = self.cur_space().profile_id.clone();
        let dir = self.root.join("profiles").join(&profile_id);
        let (title_proxy, link_proxy) = (self.proxy.clone(), self.proxy.clone());
        let start = if ok_url(url, 4096, false) {
            url.to_string()
        } else {
            "about:blank".to_string()
        };
        let scrollbar = scrollbar_script(&self.cur_space().color, true);
        let ctx = self
            .contexts
            .entry(profile_id)
            .or_insert_with(|| WebContext::new(Some(dir)));
        let page_builder = WebViewBuilder::new_with_web_context(ctx)
            .with_initialization_script_for_main_only(scrollbar, false)
            .with_bounds(rect(0.0, BAR_HEIGHT, size.width, size.height - BAR_HEIGHT))
            .with_devtools(false)
            .with_url(&start)
            .with_document_title_changed_handler(move |title| {
                let _ = title_proxy.send_event(UserEvent::PopupTitle { window: id, title });
            })
            .with_new_window_req_handler(move |url, _| {
                let _ = link_proxy.send_event(UserEvent::OpenTab { url });
                NewWindowResponse::Deny
            });
        #[cfg(windows)]
        let page_builder = {
            use wry::WebViewBuilderExtWindows;
            // Same options as the tabs of this profile: WebView2 refuses (or silently fails to
            // show) a second WebView on one data folder with different environment options.
            page_builder
                .with_browser_extensions_enabled(true)
                .with_additional_browser_args(WEBVIEW2_BROWSER_ARGS)
        };
        let page = page_builder.build_as_child(&window)?;
        let _ = page.set_visible(true);

        let popup = Popup {
            window,
            bar,
            page,
            mini: true,
        };
        popup.layout();
        let _ = popup.bar.focus();
        self.popups.insert(id, popup);
        Ok(())
    }

    pub(super) fn set_peek_links(&mut self, on: bool) {
        self.p.peek_links = on;
        if !on {
            self.close_peek();
        }
        self.save();
        let js = format!("window.__poofPeekOn&&window.__poofPeekOn({on})");
        for t in &self.tabs {
            if let Some(wv) = &t.webview {
                let _ = wv.evaluate_script(&js);
            }
        }
    }

    /// Where the overlay sits: the content area with a margin all round.
    fn peek_box(&self) -> [f64; 4] {
        let [x, y, w, h] = self.content_box();
        let mx = (w * 0.08).clamp(16.0, w / 4.0);
        let my = (h * 0.03).clamp(10.0, h / 4.0);
        [x + mx, y + my, (w - 2.0 * mx).max(1.0), (h - 2.0 * my).max(1.0)]
    }

    pub(super) fn layout_peek(&self) {
        if let Some(peek) = &self.peek_view() {
            let [x, y, w, h] = self.peek_box();
            let scale = self.window.scale_factor();
            let page_h = (h - PEEK_BAR).max(1.0);
            let _ = peek.bar.set_bounds(rect(x, y, w, PEEK_BAR));
            let _ = peek.page.set_bounds(rect(x, y + PEEK_BAR, w, page_h));
            let [cx, cy, _, _] = self.content_box();
            self.dim_tab(&peek.tab, Some([x - cx, y - cy, w, h]));
            round_corners(&peek.bar, w, PEEK_BAR, scale, true);
            round_corners(&peek.page, w, page_h, scale, false);
        }
    }

    /// Closes the overlay and takes the dimming off the page behind it.
    pub(super) fn close_peek(&mut self) {
        if let Some(peek) = self.link_peek.take() {
            self.dim_tab(&peek.tab, None);
        }
    }

    /// Dims the page behind the overlay and draws the card's shadow (`None` removes both).
    /// A click on the dimmed page closes the overlay. The page is a separate WebView that
    /// cannot be shaded natively, so this is a layer inside it.
    fn dim_tab(&self, tab: &str, card: Option<[f64; 4]>) {
        let Some(wv) = self
            .tabs
            .iter()
            .find(|t| t.id == tab)
            .and_then(|t| t.webview.as_ref())
        else {
            return;
        };
        let arg = card.map_or("null".to_string(), |c| json!(c).to_string());
        let _ = wv.evaluate_script(&format!("({DIM_JS})({arg})"));
    }

    fn peek_view(&self) -> Option<&Peek> {
        self.link_peek.as_ref()
    }

    /// Previews a link over the page that is showing.
    pub(super) fn open_peek(&mut self, url: &str) -> Result<()> {
        if !self.p.peek_links || self.overlay || self.fullscreen {
            return Ok(());
        }
        let Some(profile_id) = self
            .visible
            .as_ref()
            .and_then(|id| self.tabs.iter().find(|t| &t.id == id))
            .map(|t| t.profile_id.clone())
        else {
            return Ok(());
        };
        self.close_peek();
        let [x, y, w, h] = self.peek_box();

        let host = url::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.trim_start_matches("www.").to_string()))
            .unwrap_or_default();
        let html = popup_bar_html(&self.p.theme, &self.cur_space().color, &host, "peek");
        let ipc = bar_ipc(self.proxy.clone(), |cmd| UserEvent::PeekCmd { cmd });
        let bar_builder = WebViewBuilder::new_with_web_context(&mut self.ui_ctx)
            .with_bounds(rect(x, y, w, PEEK_BAR))
            .with_devtools(false)
            .with_html(html)
            .with_ipc_handler(ipc);
        #[cfg(windows)]
        let bar_builder = {
            use wry::WebViewBuilderExtWindows;
            bar_builder.with_additional_browser_args(WEBVIEW2_UI_ARGS)
        };
        let bar = bar_builder.build_as_child(&self.window)?;

        let dir = self.root.join("profiles").join(&profile_id);
        let (title_proxy, link_proxy, esc_proxy) = (
            self.proxy.clone(),
            self.proxy.clone(),
            self.proxy.clone(),
        );
        let scrollbar = scrollbar_script(&self.cur_space().color, true);
        let ctx = self
            .contexts
            .entry(profile_id)
            .or_insert_with(|| WebContext::new(Some(dir)));
        let page_builder = WebViewBuilder::new_with_web_context(ctx)
            .with_initialization_script_for_main_only(scrollbar, false)
            .with_bounds(rect(x, y + PEEK_BAR, w, (h - PEEK_BAR).max(1.0)))
            .with_devtools(false)
            .with_url(url)
            .with_initialization_script_for_main_only(
                "addEventListener('keydown',function(e){if(e.key==='Escape')try{window.ipc.postMessage('esc')}catch(x){}},true)",
                false,
            )
            .with_ipc_handler(move |req| {
                if req.body() == "esc" {
                    let _ = esc_proxy.send_event(UserEvent::PeekCmd {
                        cmd: "close".into(),
                    });
                }
            })
            .with_document_title_changed_handler(move |title| {
                let _ = title_proxy.send_event(UserEvent::PeekTitle { title });
            })
            .with_new_window_req_handler(move |url, _| {
                let _ = link_proxy.send_event(UserEvent::OpenTab { url });
                NewWindowResponse::Deny
            });
        #[cfg(windows)]
        let page_builder = {
            use wry::WebViewBuilderExtWindows;
            // Same options as the tabs of this profile: WebView2 refuses (or silently fails to
            // show) a second WebView on one data folder with different environment options.
            page_builder
                .with_browser_extensions_enabled(true)
                .with_additional_browser_args(WEBVIEW2_BROWSER_ARGS)
        };
        let page = page_builder.build_as_child(&self.window)?;
        let _ = page.set_visible(true);
        let _ = bar.set_visible(true);
        let _ = page.focus();
        let tab = self.visible.clone().unwrap_or_default();
        self.link_peek = Some(Peek { bar, page, tab });
        self.layout_peek();
        Ok(())
    }

    pub(super) fn set_peek_title(&self, title: &str) {
        if let Some(peek) = &self.link_peek {
            let script = format!("window.__popupTitle({})", json!(clip(title, 120)));
            let _ = peek.bar.evaluate_script(&script);
        }
    }

    /// A button of the overlay's bar. Only these exact commands are accepted.
    pub(super) fn on_peek_cmd(&mut self, cmd: &str) {
        match cmd {
            "close" => self.close_peek(),
            "expand" => {
                let url = self
                    .link_peek
                    .as_ref()
                    .and_then(|p| p.page.url().ok())
                    .unwrap_or_default();
                self.close_peek();
                if ok_external_url(&url) {
                    if let Err(error) = self.new_tab(&url) {
                        log::warn!("{error:#}");
                    }
                }
                self.push_state();
            }
            _ => {}
        }
    }
}
