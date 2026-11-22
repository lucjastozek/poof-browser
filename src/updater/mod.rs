//! Auto-update. A background thread checks the project's latest GitHub release; if a newer
//! installer exists it is downloaded and verified, and it runs silently when the browser closes.
//! Nothing interrupts the user, and nothing runs while the browser is open.
//!
//! Enabled only for release builds made by CI (`POOF_UPDATE_REPO` is baked in at build time) and
//! only on Windows, where the installer exists. Set `POOF_NO_UPDATE=1` to turn it off.

mod download;
mod release;
mod version;

use crate::UserEvent;
use std::{path::PathBuf, time::Duration};
use tao::event_loop::EventLoopProxy;

/// `owner/name` of the GitHub repository that publishes releases.
const REPO: Option<&str> = option_env!("POOF_UPDATE_REPO");
/// Let startup finish before touching the network.
const FIRST_CHECK_DELAY: Duration = Duration::from_secs(45);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

/// Starts the background checker. `data_dir` is the browser's data folder.
pub fn spawn_check(proxy: EventLoopProxy<UserEvent>, data_dir: PathBuf) {
    let Some(repo) = REPO.filter(|_| cfg!(windows) && std::env::var_os("POOF_NO_UPDATE").is_none())
    else {
        return;
    };
    let downloads = data_dir.join("updates");
    let spawned = std::thread::Builder::new()
        .name("updater".into())
        .spawn(move || run(repo, &downloads, &proxy));
    if let Err(error) = spawned {
        log::warn!("could not start the updater: {error}");
    }
}

fn run(repo: &str, downloads: &std::path::Path, proxy: &EventLoopProxy<UserEvent>) {
    let Some(current) = version::Version::parse(env!("CARGO_PKG_VERSION")) else {
        return;
    };
    download::clean_old_downloads(downloads, None);
    std::thread::sleep(FIRST_CHECK_DELAY);
    loop {
        match check_once(repo, &current, downloads) {
            Ok(Some(installer)) => {
                let _ = proxy.send_event(UserEvent::UpdateReady(installer));
                return; // nothing more to do until the next launch
            }
            Ok(None) => {}
            Err(error) => log::info!("update check: {error:#}"),
        }
        std::thread::sleep(CHECK_EVERY);
    }
}

fn check_once(
    repo: &str,
    current: &version::Version,
    downloads: &std::path::Path,
) -> anyhow::Result<Option<PathBuf>> {
    let json = download::latest_release_json(repo)?;
    let Some(update) = release::pick_update(&json, repo, current)? else {
        return Ok(None);
    };
    log::info!("update {} found, downloading", update.version);
    let installer = download::download_installer(&update, downloads)?;
    download::clean_old_downloads(downloads, Some(&update.file_name));
    Ok(Some(installer))
}

/// Starts the downloaded installer silently, detached from this process. Called as the browser
/// closes; the installer waits briefly for this process to exit, replaces the files and does
/// not relaunch the app.
#[cfg(windows)]
pub fn install_on_exit(installer: &std::path::Path) {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let started = std::process::Command::new(installer)
        .args(["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/SP-"])
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW)
        .spawn();
    if let Err(error) = started {
        log::warn!("could not start the update installer: {error}");
    }
}

#[cfg(not(windows))]
pub fn install_on_exit(_installer: &std::path::Path) {}
