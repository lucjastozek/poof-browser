//! Auto-update. A background thread checks the project's latest GitHub release right at startup;
//! if a newer installer exists the browser asks the user. "Update now" downloads and verifies it,
//! installs it and starts the browser again; "Later" downloads it quietly and installs it when
//! the browser closes. Nothing runs without that answer.
//!
//! Enabled only for release builds made by CI (`POOF_UPDATE_REPO` is baked in at build time) and
//! only on Windows, where the installer exists. Set `POOF_NO_UPDATE=1` to turn it off.

mod download;
mod release;
mod version;

pub use release::Update;

use crate::UserEvent;
use std::{path::PathBuf, time::Duration};
use tao::event_loop::EventLoopProxy;

/// `owner/name` of the GitHub repository that publishes releases.
const REPO: Option<&str> = option_env!("POOF_UPDATE_REPO");
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
    // Checked immediately: the check is a single small request on this background thread.
    loop {
        match find_update(repo, &current) {
            Ok(Some(update)) => {
                let _ = proxy.send_event(UserEvent::UpdateFound(update));
                return; // the user's answer drives everything else
            }
            Ok(None) => {}
            Err(error) => log::info!("update check: {error:#}"),
        }
        std::thread::sleep(CHECK_EVERY);
    }
}

fn find_update(repo: &str, current: &version::Version) -> anyhow::Result<Option<Update>> {
    let json = download::latest_release_json(repo)?;
    release::pick_update(&json, repo, current)
}

/// Downloads and verifies the installer on a background thread, then reports
/// `UpdateReady` (or `UpdateFailed`).
pub fn spawn_download(update: Update, data_dir: PathBuf, proxy: EventLoopProxy<UserEvent>) {
    let downloads = data_dir.join("updates");
    let spawned = std::thread::Builder::new()
        .name("update-download".into())
        .spawn(move || {
            let result = download::download_installer(&update, &downloads);
            if result.is_ok() {
                download::clean_old_downloads(&downloads, Some(&update.file_name));
            }
            let _ = proxy.send_event(match result {
                Ok(installer) => UserEvent::UpdateReady(installer),
                Err(error) => {
                    log::warn!("update download failed: {error:#}");
                    UserEvent::UpdateFailed
                }
            });
        });
    if let Err(error) = spawned {
        log::warn!("could not start the update download: {error}");
    }
}

/// Starts the downloaded installer silently, detached from this process. Called as the browser
/// closes; the installer waits briefly for this process to exit and replaces the files. With
/// `relaunch` the browser is started again once the installer has finished.
#[cfg(windows)]
pub fn install_on_exit(installer: &std::path::Path, relaunch: bool) {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    const FLAGS: &str = "/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP-";
    let mut command;
    match std::env::current_exe().ok().filter(|_| relaunch) {
        Some(exe) => {
            // One detached shell: wait for the installer, then start the new version.
            command = std::process::Command::new("cmd.exe");
            command.raw_arg(format!(
                "/D /C start \"\" /wait \"{}\" {FLAGS} & start \"\" \"{}\"",
                installer.display(),
                exe.display()
            ));
        }
        None => {
            command = std::process::Command::new(installer);
            command.args(FLAGS.split(' '));
        }
    }
    let started = command
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW)
        .spawn();
    if let Err(error) = started {
        log::warn!("could not start the update installer: {error}");
    }
}

#[cfg(not(windows))]
pub fn install_on_exit(_installer: &std::path::Path, _relaunch: bool) {}
