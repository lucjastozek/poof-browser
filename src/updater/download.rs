//! Network side of the updater: ask GitHub for the latest release and fetch the installer.

use super::release::Update;
use anyhow::{anyhow, bail, Result};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

/// Installers are tens of MB; anything far beyond that is not ours.
const MAX_INSTALLER_BYTES: u64 = 400 * 1024 * 1024;

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(60))
        .user_agent(concat!("poof/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// Blocking: call from a worker thread.
pub fn latest_release_json(repo: &str) -> Result<String> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let body = agent()
        .get(&url)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| anyhow!("update check failed: {e}"))?
        .into_string()?;
    Ok(body)
}

/// Blocking. Downloads to `<dir>/<file>.part`, checks size and checksum, then renames it into
/// place, so a file with the final name is always complete and verified.
pub fn download_installer(update: &Update, dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let target = dir.join(&update.file_name);
    if target.is_file() {
        return Ok(target);
    }
    let partial = dir.join(format!("{}.part", update.file_name));
    let result = write_verified(update, &partial);
    if result.is_err() {
        let _ = fs::remove_file(&partial);
    }
    result?;
    fs::rename(&partial, &target)?;
    Ok(target)
}

fn write_verified(update: &Update, partial: &Path) -> Result<()> {
    if update.size == 0 || update.size > MAX_INSTALLER_BYTES {
        bail!("installer has an implausible size ({} bytes)", update.size);
    }
    let response = agent()
        .get(&update.url)
        .call()
        .map_err(|e| anyhow!("installer download failed: {e}"))?;
    let mut reader = response.into_reader().take(update.size + 1);
    let mut file = File::create(partial)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        hasher.update(&buffer[..n]);
        file.write_all(&buffer[..n])?;
    }
    file.flush()?;
    if total != update.size {
        bail!(
            "installer size {total} does not match the release ({})",
            update.size
        );
    }
    match &update.sha256 {
        Some(expected) if hex(&hasher.finalize()) != *expected => {
            bail!("installer checksum does not match the release")
        }
        Some(_) => {}
        None => log::warn!("release has no checksum; installer verified by size only"),
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Removes installers and partial downloads from earlier runs, keeping `keep`.
pub fn clean_old_downloads(dir: &Path, keep: Option<&str>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if Some(entry.file_name().to_string_lossy().as_ref()) != keep {
            let _ = fs::remove_file(entry.path());
        }
    }
}
