//! Chrome Web Store support: download a CRX by extension ID, unpack it into
//! `<data>/extensions/<id>/`, and list / remove installed extensions.
//! (Loading into WebView2 happens in extensions.rs.)
use anyhow::{anyhow, bail, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
use serde_json::Value;
use std::{
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    time::Duration,
};

/// Installed automatically on first run: AdBlocker Ultimate.
pub const PREINSTALL: &[&str] = &["ohahllgiabjaoigichmmfljhkcfikeof"];

/// Accepts a raw ID or any Web Store URL containing one (32 chars, a–p).
pub fn parse_id(input: &str) -> Option<String> {
    input
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| t.len() == 32 && t.bytes().all(|b| (b'a'..=b'p').contains(&b)))
        .next_back()
        .map(str::to_string)
}

fn zip_part(b: &[u8]) -> Result<&[u8]> {
    if b.len() < 16 || &b[0..4] != b"Cr24" {
        bail!("download was not a CRX file");
    }
    let ver = u32::from_le_bytes(b[4..8].try_into()?);
    let start = match ver {
        3 => 12 + u32::from_le_bytes(b[8..12].try_into()?) as usize,
        2 => {
            16 + u32::from_le_bytes(b[8..12].try_into()?) as usize
                + u32::from_le_bytes(b[12..16].try_into()?) as usize
        }
        v => bail!("unsupported CRX version {v}"),
    };
    b.get(start..).ok_or_else(|| anyhow!("truncated CRX"))
}

/// Blocking: call from a worker thread. Returns (folder, display name).
pub fn install(root: &Path, id: &str) -> Result<(PathBuf, String)> {
    let url = format!(
        "https://clients2.google.com/service/update2/crx?response=redirect&prodversion=130.0.0.0&acceptformat=crx2,crx3&x=id%3D{id}%26uc"
    );
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(90))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36")
        .build();
    let resp = agent.get(&url).call().map_err(|e| anyhow!("download failed: {e}"))?;
    let mut buf = Vec::new();
    resp.into_reader().take(120 * 1024 * 1024).read_to_end(&mut buf)?;

    let exts = root.join("extensions");
    let tmp = exts.join(format!("_tmp_{id}"));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp)?;

    let mut ar = zip::ZipArchive::new(Cursor::new(zip_part(&buf)?))?;
    for i in 0..ar.len() {
        let mut f = ar.by_index(i)?;
        let Some(rel) = f.enclosed_name() else { continue }; // blocks zip-slip
        // Chrome adds _metadata/ ; WebView2 refuses folders starting with '_' (but _locales is fine).
        if rel.components().next().map(|c| c.as_os_str() == "_metadata").unwrap_or(false) {
            continue;
        }
        let out = tmp.join(&rel);
        if f.is_dir() {
            fs::create_dir_all(&out)?;
        } else {
            if let Some(p) = out.parent() {
                fs::create_dir_all(p)?;
            }
            let mut o = fs::File::create(&out)?;
            std::io::copy(&mut f, &mut o)?;
        }
    }
    if !tmp.join("manifest.json").is_file() {
        let _ = fs::remove_dir_all(&tmp);
        bail!("package has no manifest.json");
    }
    let name = info_for(&tmp, id).name;
    let dest = exts.join(id);
    let _ = fs::remove_dir_all(&dest);
    fs::rename(&tmp, &dest)?;
    Ok((dest, name))
}

pub fn remove(root: &Path, dir: &str) -> Result<()> {
    if dir.is_empty() || dir.contains(['/', '\\']) || dir.starts_with('.') || dir.starts_with('_') {
        bail!("invalid extension folder");
    }
    fs::remove_dir_all(root.join("extensions").join(dir))?;
    Ok(())
}

#[derive(Serialize)]
pub struct ExtInfo {
    pub dir: String,
    pub name: String,
    pub version: String,
    pub icon: Option<String>,
}

fn read_json(p: &Path) -> Option<Value> {
    let s = fs::read_to_string(p).ok()?;
    serde_json::from_str(s.trim_start_matches('\u{feff}')).ok()
}

/// Resolves "__MSG_appName__" style names through _locales.
fn localized(dir: &Path, m: &Value, s: &str) -> String {
    if let Some(k) = s.strip_prefix("__MSG_").and_then(|x| x.strip_suffix("__")) {
        let loc = m["default_locale"].as_str().unwrap_or("en");
        for l in [loc, "en"] {
            if let Some(j) = read_json(&dir.join("_locales").join(l).join("messages.json")) {
                if let Some(obj) = j.as_object() {
                    for (key, val) in obj {
                        if key.eq_ignore_ascii_case(k) {
                            if let Some(msg) = val["message"].as_str() {
                                return msg.to_string();
                            }
                        }
                    }
                }
            }
        }
    }
    s.to_string()
}

fn info_for(dir: &Path, folder: &str) -> ExtInfo {
    let m = read_json(&dir.join("manifest.json")).unwrap_or(Value::Null);
    let name = localized(dir, &m, m["name"].as_str().unwrap_or(folder));
    let version = m["version"].as_str().unwrap_or("").to_string();
    let icon = m["icons"]
        .as_object()
        .and_then(|o| o.iter().filter_map(|(k, v)| Some((k.parse::<u32>().ok()?, v.as_str()?))).max_by_key(|x| x.0))
        .and_then(|(_, rel)| {
            let path = dir.join(rel.trim_start_matches('/'));
            let bytes = fs::read(&path).ok().filter(|b| b.len() < 300_000)?;
            let mime = if rel.ends_with(".svg") { "image/svg+xml" } else { "image/png" };
            Some(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
        });
    ExtInfo { dir: folder.to_string(), name, version, icon }
}

pub fn list(root: &Path) -> Vec<ExtInfo> {
    let mut v = Vec::new();
    if let Ok(rd) = fs::read_dir(root.join("extensions")) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('_') || !e.path().join("manifest.json").is_file() {
                continue;
            }
            v.push(info_for(&e.path(), &name));
        }
    }
    v.sort_by_key(|a| a.name.to_lowercase());
    v
}
