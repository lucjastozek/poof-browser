//! Registering poof with the operating system as a web browser, and checking whether it is
//! the default one.
//!
//! Windows has no API that lets an app make itself the default: poof registers its
//! capabilities (links, `.html` files) under the current user and sends the user to the
//! "Default apps" settings page to confirm. Linux can switch directly through
//! `xdg-settings`. macOS needs an app bundle, which this binary does not know about.

// The text builders are pure and tested everywhere, but only one platform uses each of them.
#![cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]

mod desktop_entry;
mod registry_text;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod win;

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(windows)]
use win as platform;

use anyhow::Result;

/// Name the browser registers under (also the Windows "registered app" name).
const APP_NAME: &str = "poof";

/// Whether poof can be the default browser here, and whether it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Status {
    pub supported: bool,
    pub is_default: bool,
}

/// What `make_default` achieved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// poof is now the default browser.
    Done,
    /// The user has to confirm the change in the system settings, which are now open.
    #[cfg_attr(not(windows), allow(dead_code))]
    ConfirmInSystemSettings,
}

#[cfg(any(windows, target_os = "linux"))]
pub fn status() -> Status {
    Status {
        supported: true,
        is_default: platform::is_default(),
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn status() -> Status {
    Status {
        supported: false,
        is_default: false,
    }
}

/// Registers poof with the system and makes it (or starts making it) the default browser.
#[cfg(any(windows, target_os = "linux"))]
pub fn make_default() -> Result<Outcome> {
    let exe = std::env::current_exe()?;
    platform::make_default(&exe)
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn make_default() -> Result<Outcome> {
    anyhow::bail!("Setting the default browser is not supported on this system yet")
}

/// If poof registered itself earlier, makes sure the registration still points at the
/// running executable (the user may have moved or reinstalled it). Runs on a short-lived
/// thread, so startup never waits for it.
pub fn spawn_refresh() {
    #[cfg(any(windows, target_os = "linux"))]
    {
        let spawned = std::thread::Builder::new()
            .name("default-browser-refresh".into())
            .spawn(|| {
                if let Ok(exe) = std::env::current_exe() {
                    platform::refresh_registration(&exe);
                }
            });
        if let Err(error) = spawned {
            log::warn!("could not start the default-browser refresh: {error}");
        }
    }
}
