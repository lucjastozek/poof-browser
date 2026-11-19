#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

mod browser;
mod extensions;
mod pip;
mod state;
mod store;

use std::time::{Duration, Instant};
use tao::{
    dpi::LogicalSize,
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};

/// Everything that can wake the event loop from a webview callback thread.
pub enum UserEvent {
    /// Raw JSON command from the (trusted) sidebar UI.
    Ui(String),
    /// Raw JSON from a content page. UNTRUSTED: validated in browser.rs.
    Page {
        tab: String,
        body: String,
    },
    /// Native WebView2 notification that a page element entered or left fullscreen.
    Fullscreen {
        tab: String,
        active: bool,
    },
    Title {
        tab: String,
        title: String,
    },
    /// A page-specific DevTools frontend URL resolved from the profile's CDP endpoint.
    DevToolsReady {
        tab: String,
        url: String,
    },
    DevToolsFailed {
        tab: String,
        message: String,
    },
    OpenTab {
        url: String,
    },
    /// Result of a background extension install (Web Store download).
    ExtResult {
        ok: bool,
        msg: String,
        dir: Option<std::path::PathBuf>,
        id: Option<String>,
    },
    /// A WebView2 render process (or the whole browser process of a profile) died.
    TabCrashed {
        tab: String,
        browser: bool,
    },
    CloseWindow,
}

fn poof_icon() -> Option<tao::window::Icon> {
    let tree = resvg::usvg::Tree::from_data(
        include_bytes!("../poof-logo.svg"),
        &resvg::usvg::Options::default(),
    )
    .ok()?;
    let size = tree.size().to_int_size();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::default(),
        &mut pixmap.as_mut(),
    );
    tao::window::Icon::from_rgba(pixmap.data().to_vec(), size.width(), size.height()).ok()
}

const MAINTENANCE_EVERY: Duration = Duration::from_secs(60);
/// How soon a deferred (throttled) state save is flushed.
const SAVE_FLUSH_AFTER: Duration = Duration::from_millis(1500);

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let window = WindowBuilder::new()
        .with_title("poof")
        .with_decorations(false)
        .with_window_icon(poof_icon())
        .with_inner_size(LogicalSize::new(1280.0, 800.0))
        .build(&event_loop)?;

    let mut browser = browser::Browser::new(window, event_loop.create_proxy())?;

    let mut next_maintenance = Instant::now() + MAINTENANCE_EVERY;
    event_loop.run(move |event, _, flow| {
        match event {
            Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
                browser.flush_save();
                if Instant::now() >= next_maintenance {
                    browser.cleanup_expired_tabs();
                    next_maintenance = Instant::now() + MAINTENANCE_EVERY;
                }
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => browser.relayout(),
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                browser.shutdown();
                *flow = ControlFlow::Exit;
                return;
            }
            Event::UserEvent(UserEvent::CloseWindow) => {
                browser.shutdown();
                *flow = ControlFlow::Exit;
                return;
            }
            Event::UserEvent(ev) => browser.on_user_event(ev),
            _ => {}
        }

        // Sleep until the next maintenance pass, or sooner if a save is waiting to be flushed.
        // The loop is otherwise fully idle (no timers, no polling).
        let deadline = if browser.save_pending() {
            next_maintenance.min(Instant::now() + SAVE_FLUSH_AFTER)
        } else {
            next_maintenance
        };
        *flow = ControlFlow::WaitUntil(deadline);
    });
}