//! Windows: registers poof under HKEY_CURRENT_USER with `reg.exe` (no extra dependency, no
//! administrator rights) and sends the user to "Default apps" to confirm.

use super::{
    registry_text::{self, CLIENT_KEY, PROG_ID},
    Outcome, APP_NAME,
};
use anyhow::{bail, Context, Result};
use std::{os::windows::process::CommandExt, path::Path, process::Command};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const USER_CHOICE: &str = r"HKCU\Software\Microsoft\Windows\Shell\Associations\UrlAssociations";

/// Runs `reg.exe` without a console window and returns its output.
fn reg(args: &[&str]) -> Result<String> {
    let output = Command::new("reg.exe")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .context("could not run reg.exe")?;
    if !output.status.success() {
        bail!("reg.exe {} failed", args.first().copied().unwrap_or(""));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The ProgId Windows currently uses for a link scheme.
fn chosen_prog_id(scheme: &str) -> Option<String> {
    let key = format!(r"{USER_CHOICE}\{scheme}\UserChoice");
    let output = reg(&["query", &key, "/v", "ProgId"]).ok()?;
    registry_text::query_value(&output, "ProgId").map(str::to_string)
}

pub fn is_default() -> bool {
    ["http", "https"]
        .iter()
        .all(|scheme| chosen_prog_id(scheme).as_deref() == Some(PROG_ID))
}

/// Writes the registration and opens the page where the user picks poof.
pub fn make_default(exe: &Path) -> Result<Outcome> {
    register(exe)?;
    open_default_apps_page();
    Ok(Outcome::ConfirmInSystemSettings)
}

fn register(exe: &Path) -> Result<()> {
    let file = std::env::temp_dir().join(format!("poof-register-{}.reg", std::process::id()));
    std::fs::write(
        &file,
        registry_text::utf16_le_with_bom(&registry_text::reg_file(exe)),
    )
    .context("could not write the registration file")?;
    let imported = reg(&["import", &file.to_string_lossy()]);
    let _ = std::fs::remove_file(&file);
    imported.map(|_| ())
}

/// Opens Settings on poof's own "Default apps" page (or the general one on older builds).
fn open_default_apps_page() {
    let uri = format!("ms-settings:defaultapps?registeredAppUser={APP_NAME}");
    let started = Command::new("rundll32.exe")
        .args(["url.dll,FileProtocolHandler", &uri])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    if let Err(error) = started {
        log::warn!("could not open the Default apps settings: {error}");
    }
}

/// Re-registers when poof was registered before but the stored command no longer matches
/// this executable. Never registers a user who did not ask for it.
pub fn refresh_registration(exe: &Path) {
    let key = format!(r"HKCU\Software\Clients\StartMenuInternet\{CLIENT_KEY}\shell\open\command");
    let Ok(output) = reg(&["query", &key, "/ve"]) else {
        return; // never registered
    };
    let wanted = format!("\"{}\"", exe.display());
    if registry_text::query_default_value(&output) == Some(wanted.as_str()) {
        return;
    }
    if let Err(error) = register(exe) {
        log::warn!("refreshing the browser registration failed: {error:#}");
    }
}
