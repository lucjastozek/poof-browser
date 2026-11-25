//! WebView2 download hooks. Windows only; elsewhere downloads are not tracked.
//!
//! Every download of a tab is reported to the main loop (start, throttled progress, end), and
//! its live operation is kept here so the sidebar can pause, resume or cancel it. WebView2
//! calls back on the UI thread, which is also the event-loop thread, so the registry below is
//! thread-local instead of locked.

use super::*;

/// What the sidebar asked to do with a running download.
#[derive(Clone, Copy)]
pub(super) enum DownloadControl {
    Pause,
    Resume,
    Cancel,
    /// Drop our handle without touching the download (its WebView is gone).
    Forget,
}

#[cfg(windows)]
pub(super) use windows_impl::{control_download, watch_downloads};

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use crate::events::DownloadEvent;
    use std::cell::RefCell;
    use webview2_com::{
        BytesReceivedChangedEventHandler, DownloadStartingEventHandler,
        Microsoft::Web::WebView2::Win32::*, StateChangedEventHandler,
    };
    use windows::core::{Interface, BOOL, HSTRING, PWSTR};
    use windows::Win32::System::Com::CoTaskMemFree;

    type Result<T> = windows::core::Result<T>;

    thread_local! {
        /// Running downloads: the WebView2 operation and the file it writes to.
        static LIVE: RefCell<HashMap<String, (ICoreWebView2DownloadOperation, PathBuf)>> =
            RefCell::new(HashMap::new());
    }

    /// Hooks the download events of one tab's WebView.
    pub(in crate::browser) fn watch_downloads(
        wv: &WebView,
        proxy: &EventLoopProxy<UserEvent>,
        tab: &str,
    ) {
        let proxy = proxy.clone();
        let tab = tab.to_string();
        unsafe {
            let Ok(core) = wv.controller().CoreWebView2() else {
                log::warn!("getting WebView2 core for download handler failed");
                return;
            };
            let Ok(core4) = core.cast::<ICoreWebView2_4>() else {
                log::warn!("WebView2 runtime lacks download events (ICoreWebView2_4)");
                return;
            };
            let handler = DownloadStartingEventHandler::create(Box::new(move |_sender, args| {
                let Some(args) = args else { return Ok(()) };
                if let Err(error) = begin_download(&proxy, &tab, &args) {
                    log::warn!("tracking a download failed: {error}");
                }
                Ok(())
            }));
            let mut token = 0;
            if let Err(error) = core4.add_DownloadStarting(&handler, &mut token) {
                log::warn!("registering WebView2 download handler failed: {error}");
            }
        }
    }

    /// Pauses, resumes or cancels a running download. False when there is nothing to control.
    pub(in crate::browser) fn control_download(id: &str, action: DownloadControl) -> bool {
        if matches!(action, DownloadControl::Forget) {
            return LIVE.with(|live| live.borrow_mut().remove(id).is_some());
        }
        let Some(op) = LIVE.with(|live| live.borrow().get(id).map(|(op, _)| op.clone())) else {
            return false;
        };
        let result = unsafe {
            match action {
                DownloadControl::Pause => op.Pause(),
                DownloadControl::Resume => op.Resume(),
                DownloadControl::Cancel => op.Cancel(),
                DownloadControl::Forget => Ok(()),
            }
        };
        if let Err(error) = &result {
            log::warn!("download control failed: {error}");
        }
        result.is_ok()
    }

    unsafe fn begin_download(
        proxy: &EventLoopProxy<UserEvent>,
        tab: &str,
        args: &ICoreWebView2DownloadStartingEventArgs,
    ) -> Result<()> {
        let op = args.DownloadOperation()?;
        let url = read_string(|out| op.Uri(out))?;
        let proposed = read_string(|out| args.ResultFilePath(out))?;
        let path = reserve_path(Path::new(&proposed));
        args.SetResultFilePath(&HSTRING::from(path.to_string_lossy().as_ref()))?;
        // The browser shows its own download UI; this hides the WebView2 default one.
        args.SetHandled(true)?;
        let (_, total) = byte_counts(&op)?;

        let id = new_id();
        LIVE.with(|live| {
            live.borrow_mut()
                .insert(id.clone(), (op.clone(), path.clone()))
        });
        let _ = proxy.send_event(UserEvent::Download(DownloadEvent::Started {
            id: id.clone(),
            tab: tab.to_string(),
            url,
            path,
            total,
        }));
        watch_progress(proxy, &id, &op);
        watch_state(proxy, &id, &op);
        Ok(())
    }

    fn watch_progress(
        proxy: &EventLoopProxy<UserEvent>,
        id: &str,
        op: &ICoreWebView2DownloadOperation,
    ) {
        let proxy = proxy.clone();
        let id = id.to_string();
        let mut last = Instant::now() - DOWNLOAD_PROGRESS_INTERVAL;
        let handler = BytesReceivedChangedEventHandler::create(Box::new(move |sender, _| {
            let Some(op) = sender else { return Ok(()) };
            if last.elapsed() < DOWNLOAD_PROGRESS_INTERVAL {
                return Ok(());
            }
            last = Instant::now();
            let (received, total) = unsafe { byte_counts(&op)? };
            let _ = proxy.send_event(UserEvent::Download(DownloadEvent::Progress {
                id: id.clone(),
                received,
                total,
            }));
            Ok(())
        }));
        let mut token = 0;
        if let Err(error) = unsafe { op.add_BytesReceivedChanged(&handler, &mut token) } {
            log::warn!("registering WebView2 download progress handler failed: {error}");
        }
    }

    fn watch_state(
        proxy: &EventLoopProxy<UserEvent>,
        id: &str,
        op: &ICoreWebView2DownloadOperation,
    ) {
        let proxy = proxy.clone();
        let id = id.to_string();
        let handler = StateChangedEventHandler::create(Box::new(move |sender, _| {
            let Some(op) = sender else { return Ok(()) };
            let status = unsafe { status_of(&op)? };
            let (received, total) = unsafe { byte_counts(&op) }.unwrap_or((0, 0));
            let path = (status == DownloadStatus::Completed)
                .then(|| unsafe { read_string(|out| op.ResultFilePath(out)) }.ok())
                .flatten()
                .map(PathBuf::from);
            if !status.is_live() {
                LIVE.with(|live| live.borrow_mut().remove(&id));
            }
            let _ = proxy.send_event(UserEvent::Download(DownloadEvent::State {
                id: id.clone(),
                status,
                path,
                received,
                total,
            }));
            Ok(())
        }));
        let mut token = 0;
        if let Err(error) = unsafe { op.add_StateChanged(&handler, &mut token) } {
            log::warn!("registering WebView2 download state handler failed: {error}");
        }
    }

    /// Maps WebView2's state and interrupt reason onto the browser's own states.
    unsafe fn status_of(op: &ICoreWebView2DownloadOperation) -> Result<DownloadStatus> {
        let mut state = COREWEBVIEW2_DOWNLOAD_STATE::default();
        op.State(&mut state)?;
        if state == COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED {
            return Ok(DownloadStatus::Completed);
        }
        if state == COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS {
            return Ok(DownloadStatus::Active);
        }
        let mut reason = COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON::default();
        op.InterruptReason(&mut reason)?;
        if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_CANCELED {
            return Ok(DownloadStatus::Cancelled);
        }
        let mut can_resume = BOOL::default();
        op.CanResume(&mut can_resume)?;
        Ok(if can_resume.as_bool() {
            DownloadStatus::Paused
        } else {
            DownloadStatus::Failed
        })
    }

    /// (received, total) in bytes; total is 0 while unknown.
    unsafe fn byte_counts(op: &ICoreWebView2DownloadOperation) -> Result<(u64, u64)> {
        let (mut received, mut total) = (0i64, 0i64);
        op.BytesReceived(&mut received)?;
        op.TotalBytesToReceive(&mut total)?;
        Ok((received.max(0) as u64, total.max(0) as u64))
    }

    /// The path WebView2 proposed, made unique among files and running downloads.
    fn reserve_path(proposed: &Path) -> PathBuf {
        let taken: HashSet<PathBuf> = LIVE.with(|live| {
            live.borrow()
                .values()
                .map(|(_, path)| path.clone())
                .collect()
        });
        unique_path(proposed, &taken)
    }

    /// Reads a string WebView2 allocated (and frees it).
    unsafe fn read_string(fetch: impl FnOnce(&mut PWSTR) -> Result<()>) -> Result<String> {
        let mut raw = PWSTR::null();
        fetch(&mut raw)?;
        if raw.is_null() {
            return Ok(String::new());
        }
        let text = raw.to_string().unwrap_or_default();
        CoTaskMemFree(Some(raw.0 as *const std::ffi::c_void));
        Ok(text)
    }
}

/// Without WebView2 there is nothing to hook.
#[cfg(not(windows))]
pub(super) fn watch_downloads(_wv: &WebView, _proxy: &EventLoopProxy<UserEvent>, _tab: &str) {}

#[cfg(not(windows))]
pub(super) fn control_download(_id: &str, _action: DownloadControl) -> bool {
    false
}
