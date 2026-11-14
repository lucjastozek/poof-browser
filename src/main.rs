#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

mod browser;
mod extensions;
mod pip;
mod state;
mod store;

use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};

/// Everything that can wake the event loop from a webview callback thread.
pub enum UserEvent {
    /// Raw JSON command from the (trusted) sidebar UI.
    Ui(String),
    /// Raw JSON from a content page. UNTRUSTED: validated in browser.rs.
    Page { tab: String, body: String },
    Title { tab: String, title: String },
    OpenTab { url: String },
    /// Result of a background extension install (Web Store download).
    ExtResult { ok: bool, msg: String, dir: Option<std::path::PathBuf>, id: Option<String> },
    CloseWindow,
}

fn poof_icon() -> Option<tao::window::Icon> {
    let tree = resvg::usvg::Tree::from_data(
        include_bytes!("../poof-logo.svg"),
        &resvg::usvg::Options::default(),
    ).ok()?;
    let size = tree.size().to_int_size();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())?;
    resvg::render(&tree, resvg::tiny_skia::Transform::default(), &mut pixmap.as_mut());
    tao::window::Icon::from_rgba(pixmap.data().to_vec(), size.width(), size.height()).ok()
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let window = WindowBuilder::new()
        .with_title("poof")
        .with_decorations(false)
        .with_window_icon(poof_icon())
        .with_inner_size(LogicalSize::new(1280.0, 800.0))
        .build(&event_loop)?;

    let mut browser = browser::Browser::new(window, event_loop.create_proxy())?;

    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent { event: WindowEvent::Resized(_), .. } => browser.relayout(),
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                browser.save();
                *flow = ControlFlow::Exit;
            }
            Event::UserEvent(UserEvent::CloseWindow) => {
                browser.save();
                *flow = ControlFlow::Exit;
            }
            Event::UserEvent(ev) => browser.on_user_event(ev),
            _ => {}
        }
    });
}
