//! Linux: installs a `.desktop` entry for the current user and switches the default browser
//! with `xdg-settings`.

use super::{
    desktop_entry::{desktop_entry, FILE_NAME},
    Outcome,
};
use anyhow::{bail, Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn applications_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("applications"))
}

fn xdg_settings(args: &[&str]) -> Option<String> {
    let output = Command::new("xdg-settings").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn is_default() -> bool {
    xdg_settings(&["get", "default-web-browser"]).as_deref() == Some(FILE_NAME)
}

fn write_entry(exe: &Path) -> Result<()> {
    let dir = applications_dir().context("no applications folder for this user")?;
    fs::create_dir_all(&dir)?;
    fs::write(dir.join(FILE_NAME), desktop_entry(exe))?;
    Ok(())
}

pub fn make_default(exe: &Path) -> Result<Outcome> {
    write_entry(exe)?;
    if xdg_settings(&["set", "default-web-browser", FILE_NAME]).is_none() {
        bail!("xdg-settings could not change the default browser");
    }
    Ok(Outcome::Done)
}

/// Keeps an existing entry pointing at this executable; never creates one.
pub fn refresh_registration(exe: &Path) {
    let Some(path) = applications_dir().map(|d| d.join(FILE_NAME)) else {
        return;
    };
    let Ok(current) = fs::read_to_string(&path) else {
        return;
    };
    if current != desktop_entry(exe) {
        if let Err(error) = write_entry(exe) {
            log::warn!("refreshing the desktop entry failed: {error:#}");
        }
    }
}
