//! Reads the GitHub "latest release" response and decides whether it is an update.

use super::version::Version;
use anyhow::{bail, Result};
use serde::Deserialize;

/// The installer asset every release publishes (see `.github/workflows/release.yml`).
const INSTALLER_SUFFIX: &str = "-setup.exe";

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    size: u64,
    browser_download_url: String,
    /// `sha256:<hex>`; GitHub computes it for every uploaded asset.
    digest: Option<String>,
}

/// A newer release whose installer can be downloaded.
#[derive(Debug, Clone, PartialEq)]
pub struct Update {
    pub version: Version,
    pub file_name: String,
    pub url: String,
    pub size: u64,
    pub sha256: Option<String>,
}

/// Returns the release if it is newer than `current` and ships an installer. The download URL
/// must belong to `repo`, so a malformed or hostile response can't point elsewhere.
pub fn pick_update(json: &str, repo: &str, current: &Version) -> Result<Option<Update>> {
    let release: ApiRelease = serde_json::from_str(json)?;
    if release.draft || release.prerelease {
        return Ok(None);
    }
    let Some(version) = Version::parse(&release.tag_name) else {
        bail!("unrecognised release tag {:?}", release.tag_name);
    };
    if version <= *current {
        return Ok(None);
    }
    let allowed_prefix = format!("https://github.com/{repo}/releases/download/");
    let asset = release.assets.into_iter().find(|a| {
        a.name.ends_with(INSTALLER_SUFFIX) && a.browser_download_url.starts_with(&allowed_prefix)
    });
    let Some(asset) = asset else {
        bail!("release {version} has no installer asset");
    };
    Ok(Some(Update {
        version,
        file_name: asset.name,
        url: asset.browser_download_url,
        size: asset.size,
        sha256: asset
            .digest
            .and_then(|d| d.strip_prefix("sha256:").map(str::to_ascii_lowercase)),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPO: &str = "me/poof";

    fn release(tag: &str, extra: &str) -> String {
        format!(
            r#"{{"tag_name":"{tag}",{extra}"assets":[
              {{"name":"poof-{tag}-setup.exe","size":10,
                "browser_download_url":"https://github.com/me/poof/releases/download/{tag}/poof-{tag}-setup.exe",
                "digest":"sha256:ABCDEF"}},
              {{"name":"notes.txt","size":1,"browser_download_url":"https://github.com/me/poof/releases/download/{tag}/notes.txt"}}]}}"#
        )
    }

    fn current(v: &str) -> Version {
        Version::parse(v).unwrap()
    }

    #[test]
    fn offers_a_newer_release_with_its_installer() {
        let update = pick_update(&release("v1.2.0", ""), REPO, &current("1.1.9"))
            .unwrap()
            .unwrap();
        assert_eq!(update.version, current("1.2.0"));
        assert_eq!(update.sha256.as_deref(), Some("abcdef"));
        assert!(update.file_name.ends_with("-setup.exe"));
    }

    #[test]
    fn ignores_same_older_draft_and_prerelease() {
        assert!(pick_update(&release("v1.2.0", ""), REPO, &current("1.2.0"))
            .unwrap()
            .is_none());
        assert!(pick_update(&release("v1.0.0", ""), REPO, &current("1.2.0"))
            .unwrap()
            .is_none());
        assert!(pick_update(
            &release("v2.0.0", r#""draft":true,"#),
            REPO,
            &current("1.0.0")
        )
        .unwrap()
        .is_none());
        assert!(pick_update(
            &release("v2.0.0", r#""prerelease":true,"#),
            REPO,
            &current("1.0.0")
        )
        .unwrap()
        .is_none());
    }

    #[test]
    fn refuses_downloads_from_another_repository() {
        assert!(pick_update(&release("v2.0.0", ""), "someone/else", &current("1.0.0")).is_err());
    }
}
