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
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let window = WindowBuilder::new()
        .with_title("poof")
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
            Event::UserEvent(ev) => browser.on_user_event(ev),
            _ => {}
        }
    });
}
