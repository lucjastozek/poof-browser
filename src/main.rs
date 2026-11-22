#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

mod app_icon;
mod browser;
mod events;
mod extensions;
mod gesture;
mod pip;
mod state;
mod updater;

pub use events::UserEvent;

use std::time::{Duration, Instant};
use tao::{
    dpi::LogicalSize,
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};

const MAINTENANCE_EVERY: Duration = Duration::from_secs(60);

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let window = WindowBuilder::new()
        .with_title("poof")
        .with_decorations(false)
        .with_window_icon(app_icon::window_icon())
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

        // Sleep until the next maintenance pass, or sooner if a deferred sidebar update or save is
        // waiting to be flushed. The loop is otherwise fully idle (no timers, no polling).
        let deadline = match browser.flush_delay() {
            Some(delay) => next_maintenance.min(Instant::now() + delay),
            None => next_maintenance,
        };
        *flow = ControlFlow::WaitUntil(deadline);
    });
}
