//! Chromium extension loading.
//!
//! On Windows we use WebView2's `Profile.AddBrowserExtension`, which loads *unpacked* Chromium
//! (MV2/MV3) extensions into the profile's user-data folder. Drop each extension (a folder with
//! manifest.json) into `<data dir>/extensions/<name>/`.
use std::path::Path;
use wry::WebView;

pub fn load_all(wv: &WebView, dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        // WebView2 rejects names starting with '_' (e.g. our _tmp_ staging dirs).
        if e.file_name().to_string_lossy().starts_with('_') {
            continue;
        }
        if p.join("manifest.json").is_file() {
            add(wv, &p);
        }
    }
}

#[cfg(windows)]
pub fn add(wv: &WebView, path: &Path) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2Profile7, ICoreWebView2_13};
    use webview2_com::ProfileAddBrowserExtensionCompletedHandler;
    use windows::core::{Interface, HSTRING};
    use wry::WebViewExtWindows;

    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    // WebView2 dislikes the \\?\ verbatim prefix that canonicalize() adds.
    let s = path
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_string();
    let name = s.clone();

    unsafe {
        let res: windows::core::Result<()> = (|| {
            let core = wv.controller().CoreWebView2()?;
            let core13: ICoreWebView2_13 = core.cast()?;
            let profile: ICoreWebView2Profile7 = core13.Profile()?.cast()?;
            let handler =
                ProfileAddBrowserExtensionCompletedHandler::create(Box::new(move |r, _ext| {
                    match r {
                        Ok(()) => log::info!("extension loaded: {name}"),
                        Err(e) => log::warn!("extension failed ({name}): {e}"),
                    }
                    Ok(())
                }));
            profile.AddBrowserExtension(&HSTRING::from(s.as_str()), &handler)
        })();
        if let Err(e) = res {
            log::warn!("AddBrowserExtension error: {e}");
        }
    }
}

#[cfg(not(windows))]
pub fn add(_wv: &WebView, path: &Path) {
    log::warn!(
        "extensions not supported by this backend yet (would load {}). \
         Needs a CEF backend on Linux/macOS.",
        path.display()
    );
}
