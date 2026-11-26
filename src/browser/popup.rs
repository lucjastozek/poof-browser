//! Popup windows (Google sign-in, `window.open`) shown as a poof window: the same top bar and
//! window buttons as the main window, the page below it, no URL bar. Any failure falls back to
//! the default popup, so a login can never break.

use super::popup_watch::{watch_close, watch_popups};
use super::*;
use crate::events::PopupRequest;
use tao::{
    dpi::LogicalSize, event::WindowEvent, event_loop::EventLoopWindowTarget, window::WindowId,
};

/// Size used when the page does not ask for one, in logical pixels.
const POPUP_DEFAULT_SIZE: (f64, f64) = (500.0, 650.0);
/// Bounds that keep a page-requested size usable.
const POPUP_MIN_SIZE: (f64, f64) = (320.0, 240.0);
const POPUP_MAX_SIZE: (f64, f64) = (1600.0, 1200.0);
/// Height of the top bar in logical pixels: `--topbar-height` in `ui/styles/layout/topbar.css`.
const BAR_HEIGHT: f64 = 40.0;

const BAR_PAGE: &str = include_str!("../../ui/popup-bar.html");
const BAR_STYLES: &[&str] = &[
    include_str!("../../ui/styles/foundation/theme.css"),
    include_str!("../../ui/styles/foundation/base.css"),
    include_str!("../../ui/styles/layout/topbar.css"),
    include_str!("../../ui/styles/layout/popup-bar.css"),
];
const BAR_SCRIPTS: &[&str] = &[
    include_str!("../../ui/scripts/core/colors.js"),
    include_str!("../../ui/scripts/core/theme.js"),
    include_str!("../../ui/scripts/popup-bar.js"),
];

/// A popup window with its top bar and page; dropping it closes all three.
pub(super) struct Popup {
    window: Window,
    bar: WebView,
    page: WebView,
}

/// The top bar document with styles, scripts and the window's initial state inlined.
fn popup_bar_html(theme: &str, space: &str, title: &str) -> String {
    let init = json!({ "theme": theme, "space": space, "title": title })
        .to_string()
        .replace('<', "\\u003c");
    let styles = format!("<style>{}</style>", BAR_STYLES.concat());
    let scripts = format!(
        "<script>window.__popupInit = {init};</script>{}",
        BAR_SCRIPTS
            .iter()
            .map(|script| format!("<script>{script}</script>"))
            .collect::<String>()
    );
    BAR_PAGE
        .replace("/*STYLES*/", &styles)
        .replace("/*SCRIPTS*/", &scripts)
}

impl Popup {
    /// Puts the bar on top and the page under it, filling the window.
    fn layout(&self) {
        let (width, height) = logical(&self.window);
        let _ = self.bar.set_bounds(rect(0.0, 0.0, width, BAR_HEIGHT));
        let _ = self
            .page
            .set_bounds(rect(0.0, BAR_HEIGHT, width, height - BAR_HEIGHT));
    }
}

impl Browser {
    pub(crate) fn is_main_window(&self, id: WindowId) -> bool {
        self.window.id() == id
    }

    /// Builds the popup window and attaches it to the page that asked for it. On any failure
    /// the default popup is used instead.
    #[cfg(windows)]
    pub(crate) fn open_popup_window(
        &mut self,
        request: PopupRequest,
        target: &EventLoopWindowTarget<UserEvent>,
    ) {
        use wry::WebViewExtWindows;
        let attached = self.build_popup(&request, target).and_then(|popup| {
            let page = popup.page.webview();
            unsafe {
                request.args.0.SetHandled(true)?;
                request.args.0.SetNewWindow(&page)?;
            }
            Ok(popup)
        });
        match attached {
            Ok(popup) => {
                log::info!("popup window opened");
                self.popups.insert(popup.window.id(), popup);
            }
            Err(error) => {
                log::warn!("popup window failed, using the default popup: {error:#}");
                unsafe {
                    let _ = request.args.0.SetHandled(false);
                }
            }
        }
        unsafe {
            let _ = request.deferral.0.Complete();
        }
    }

    #[cfg(not(windows))]
    pub(crate) fn open_popup_window(
        &mut self,
        _: PopupRequest,
        _: &EventLoopWindowTarget<UserEvent>,
    ) {
    }

    #[cfg(windows)]
    fn build_popup(
        &mut self,
        request: &PopupRequest,
        target: &EventLoopWindowTarget<UserEvent>,
    ) -> Result<Popup> {
        use tao::window::WindowBuilder;
        use wry::WebViewBuilderExtWindows;
        let (width, height) = request.size.unwrap_or(POPUP_DEFAULT_SIZE);
        let size = LogicalSize::new(
            width.clamp(POPUP_MIN_SIZE.0, POPUP_MAX_SIZE.0),
            // The page asked for its own content size; the bar comes on top of it.
            height.clamp(POPUP_MIN_SIZE.1, POPUP_MAX_SIZE.1) + BAR_HEIGHT,
        );
        let window = WindowBuilder::new()
            .with_title("poof")
            .with_decorations(false)
            .with_window_icon(crate::app_icon::window_icon())
            .with_inner_size(size)
            .with_min_inner_size(LogicalSize::new(
                POPUP_MIN_SIZE.0,
                POPUP_MIN_SIZE.1 + BAR_HEIGHT,
            ))
            .with_theme(Some(self.popup_theme()))
            .build(target)?;
        self.center_over_main(&window);
        let id = window.id();

        let html = popup_bar_html(&self.p.theme, &self.cur_space().color, "");
        let bar_proxy = self.proxy.clone();
        let bar = WebViewBuilder::new_with_web_context(&mut self.ui_ctx)
            .with_bounds(rect(0.0, 0.0, size.width, BAR_HEIGHT))
            .with_devtools(false)
            .with_html(html)
            .with_additional_browser_args(WEBVIEW2_UI_ARGS)
            .with_ipc_handler(move |req| {
                let cmd = serde_json::from_str::<Value>(req.body())
                    .ok()
                    .and_then(|v| v["cmd"].as_str().map(str::to_string));
                if let Some(cmd) = cmd {
                    let _ = bar_proxy.send_event(UserEvent::PopupCmd { window: id, cmd });
                }
            })
            .build_as_child(&window)?;

        let (title_proxy, link_proxy) = (self.proxy.clone(), self.proxy.clone());
        let page = WebViewBuilder::new()
            .with_environment(request.environment.0.clone())
            .with_bounds(rect(0.0, BAR_HEIGHT, size.width, size.height - BAR_HEIGHT))
            .with_devtools(false)
            .with_document_title_changed_handler(move |title| {
                let _ = title_proxy.send_event(UserEvent::PopupTitle { window: id, title });
            })
            // Popups of popups are opened by `watch_popups`; plain links become tabs.
            .with_new_window_req_handler(move |url, features| {
                if features.size.is_none() && features.position.is_none() {
                    let _ = link_proxy.send_event(UserEvent::OpenTab { url });
                }
                NewWindowResponse::Deny
            })
            .build_as_child(&window)?;
        watch_close(&page, &self.proxy, id);
        watch_popups(&page, &self.proxy, false);
        show_link_status(&page);
        let popup = Popup { window, bar, page };
        popup.layout();
        Ok(popup)
    }

    /// The same light/dark choice as the main window.
    #[cfg(windows)]
    fn popup_theme(&self) -> Theme {
        match self.p.theme.as_str() {
            "light" => Theme::Light,
            "dark" => Theme::Dark,
            _ => self.window.theme(),
        }
    }

    #[cfg(windows)]
    fn center_over_main(&self, popup: &Window) {
        use tao::dpi::PhysicalPosition;
        let (Ok(pos), size, own) = (
            self.window.outer_position(),
            self.window.outer_size(),
            popup.outer_size(),
        ) else {
            return;
        };
        let x = pos.x + (size.width as i32 - own.width as i32) / 2;
        let y = pos.y + (size.height as i32 - own.height as i32) / 2;
        popup.set_outer_position(PhysicalPosition::new(x, y));
    }

    pub(crate) fn set_popup_title(&self, window: WindowId, title: &str) {
        let Some(popup) = self.popups.get(&window) else {
            return;
        };
        let title = clip(title, 120);
        popup
            .window
            .set_title(if title.is_empty() { "poof" } else { &title });
        let script = format!("window.__popupTitle({})", json!(title));
        let _ = popup.bar.evaluate_script(&script);
    }

    /// A title bar button or drag. Only these exact commands are accepted.
    pub(crate) fn on_popup_cmd(&mut self, window: WindowId, cmd: &str) {
        let Some(popup) = self.popups.get(&window) else {
            return;
        };
        match cmd {
            "min" => popup.window.set_minimized(true),
            "max" => popup.window.set_maximized(!popup.window.is_maximized()),
            "drag" => {
                if let Err(error) = popup.window.drag_window() {
                    log::warn!("popup drag failed: {error}");
                }
            }
            "close" => {
                self.popups.remove(&window);
            }
            _ => {}
        }
    }

    /// Resizes a popup's bar and page, and forgets it once it was closed.
    pub(crate) fn on_popup_window_event(&mut self, window: WindowId, event: &WindowEvent) {
        match event {
            WindowEvent::Resized(_) => {
                if let Some(popup) = self.popups.get(&window) {
                    popup.layout();
                }
            }
            WindowEvent::CloseRequested | WindowEvent::Destroyed => {
                self.popups.remove(&window);
            }
            _ => {}
        }
    }
}
