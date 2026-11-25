//! The page picture shown behind full-area views such as settings, and shortcut edits.
//!
//! Native page WebViews always draw above the sidebar's WebView, so a view can only show the
//! page by hiding it and displaying a picture of it instead.

use super::*;
use base64::Engine;

/// A picture larger than this (a very big, busy window) is skipped: the view just opens
/// over an empty stage instead.
const MAX_SNAPSHOT_BYTES: usize = 12 * 1024 * 1024;

impl Browser {
    /// Hides or shows the page behind a full-area sidebar view.
    pub(super) fn apply_overlay(&mut self, open: bool) {
        self.overlay = open;
        if open {
            let _ = self.sidebar.focus();
        }
        self.relayout();
        if !open {
            if let Some(wv) = self.visible_webview() {
                let _ = wv.focus();
            }
        }
    }

    /// Starts capturing the visible page. Returns false when there is nothing to capture or the
    /// platform cannot; the caller then opens the view straight away.
    pub(super) fn start_page_snapshot(&self) -> bool {
        match self.visible_webview() {
            Some(wv) => capture_page(wv, &self.proxy),
            None => false,
        }
    }

    pub(super) fn on_page_snapshot(&mut self, png: Option<Vec<u8>>) {
        if !self.awaiting_backdrop {
            return;
        }
        let picture = png.filter(|p| p.len() <= MAX_SNAPSHOT_BYTES);
        let Some(picture) = picture else {
            self.awaiting_backdrop = false;
            self.apply_overlay(true);
            return;
        };
        let url = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(picture)
        );
        self.call_sidebar("__pageBackdrop", &Value::String(url));
    }

    /// Persists and applies the shortcut table after any change.
    pub(super) fn shortcuts_changed(&mut self) {
        shortcuts::apply_keymap(&self.p.shortcuts);
        self.save();
        self.push_state();
    }

    pub(super) fn set_shortcut(&mut self, id: &str, chord: Option<&str>) {
        match shortcuts::assign(&mut self.p.shortcuts, id, chord) {
            shortcuts::Assigned::Invalid => {
                self.toast("That shortcut can’t be used.");
                return;
            }
            shortcuts::Assigned::Moved(from) => {
                self.toast(&format!("Moved from “{from}”"));
            }
            shortcuts::Assigned::Done => {}
        }
        self.shortcuts_changed();
    }
}

#[cfg(windows)]
fn capture_page(wv: &WebView, proxy: &EventLoopProxy<UserEvent>) -> bool {
    use webview2_com::{CapturePreviewCompletedHandler, Microsoft::Web::WebView2::Win32::*};
    use windows::Win32::{
        Foundation::HGLOBAL,
        System::Com::{
            IStream, StructuredStorage::CreateStreamOnHGlobal, STATFLAG_NONAME, STATSTG,
            STREAM_SEEK_SET,
        },
    };

    /// Reads a whole in-memory stream.
    unsafe fn read_all(stream: &IStream) -> Option<Vec<u8>> {
        stream.Seek(0, STREAM_SEEK_SET, None).ok()?;
        let mut stat = STATSTG::default();
        stream.Stat(&mut stat, STATFLAG_NONAME).ok()?;
        let len = stat.cbSize as usize;
        if len == 0 || len > MAX_SNAPSHOT_BYTES {
            return None;
        }
        let mut buffer = vec![0u8; len];
        let mut read = 0u32;
        stream
            .Read(
                buffer.as_mut_ptr().cast(),
                len as u32,
                Some(&mut read as *mut u32),
            )
            .ok()
            .ok()?;
        buffer.truncate(read as usize);
        Some(buffer)
    }

    unsafe {
        let Ok(core) = wv.controller().CoreWebView2() else {
            return false;
        };
        let Ok(stream) = CreateStreamOnHGlobal(HGLOBAL::default(), true) else {
            return false;
        };
        let reader = stream.clone();
        let proxy = proxy.clone();
        let handler = CapturePreviewCompletedHandler::create(Box::new(move |result| {
            let png = result.ok().and_then(|_| read_all(&reader));
            let _ = proxy.send_event(UserEvent::PageSnapshot(png));
            Ok(())
        }));
        match core.CapturePreview(
            COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
            &stream,
            &handler,
        ) {
            Ok(()) => true,
            Err(error) => {
                log::warn!("capturing the page failed: {error}");
                false
            }
        }
    }
}

/// Other platforms: no picture; the view opens over an empty stage.
#[cfg(not(windows))]
fn capture_page(_wv: &WebView, _proxy: &EventLoopProxy<UserEvent>) -> bool {
    false
}
