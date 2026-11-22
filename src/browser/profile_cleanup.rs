//! Per-profile retention: closing stale tabs and clearing cookies, cache and history.

use super::*;

impl Browser {
    /// Applies the retention settings of the current space's profile.
    pub(super) fn update_profile_cleanup(&mut self, hours: CleanupHours) {
        let CleanupHours {
            tab_retention_hours,
            cookie_retention_hours,
            cache_retention_hours,
            history_retention_hours,
        } = hours;
        let valid = |v: u64, allow_off: bool| {
            (allow_off && v == 0) || matches!(v, 24 | 48 | 72 | 168 | 336)
        };
        let profile_id = self.cur_space().profile_id.clone();
        if let Some(profile) = self.p.profiles.iter_mut().find(|p| p.id == profile_id) {
            if valid(tab_retention_hours, false) {
                profile.tab_retention_hours = tab_retention_hours;
            }
            if valid(cookie_retention_hours, true) {
                profile.cookie_retention_hours = cookie_retention_hours;
            }
            if valid(cache_retention_hours, true) {
                profile.cache_retention_hours = cache_retention_hours;
            }
            if valid(history_retention_hours, true) {
                profile.history_retention_hours = history_retention_hours;
            }
            profile.last_cleanup_at = 0;
        }
        self.save();
        self.prune_history();
    }

    #[cfg(windows)]
    pub(super) fn cleanup_profile_data(&mut self) {
        use webview2_com::{
            ClearBrowsingDataCompletedHandler,
            Microsoft::Web::WebView2::Win32::{
                ICoreWebView2Profile2, ICoreWebView2_13,
                COREWEBVIEW2_BROWSING_DATA_KINDS_BROWSING_HISTORY,
                COREWEBVIEW2_BROWSING_DATA_KINDS_COOKIES,
                COREWEBVIEW2_BROWSING_DATA_KINDS_DISK_CACHE,
            },
        };
        use windows::core::Interface;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();
        let due_profiles: Vec<String> = self
            .p
            .profiles
            .iter()
            .filter(|p| {
                p.cookie_retention_hours > 0
                    || p.cache_retention_hours > 0
                    || p.history_retention_hours > 0
            })
            .filter(|p| now.saturating_sub(p.last_cleanup_at) >= 3600)
            .map(|p| p.id.clone())
            .collect();

        for profile_id in due_profiles {
            let Some(webview) = self
                .tabs
                .iter()
                .find(|t| t.profile_id == profile_id && t.webview.is_some())
                .and_then(|t| t.webview.as_ref())
            else {
                continue;
            };

            let settings = self
                .p
                .profiles
                .iter()
                .find(|p| p.id == profile_id)
                .map(|p| {
                    (
                        p.cookie_retention_hours,
                        p.cache_retention_hours,
                        p.history_retention_hours,
                    )
                })
                .unwrap_or((0, 0, 0));

            let result = (|| -> anyhow::Result<()> {
                unsafe {
                    let core = webview.controller().CoreWebView2()?;
                    let core13: ICoreWebView2_13 = core.cast()?;
                    let profile = core13.Profile()?;
                    let profile2: ICoreWebView2Profile2 = profile.cast()?;
                    for (hours, kind) in [
                        (settings.0, COREWEBVIEW2_BROWSING_DATA_KINDS_COOKIES),
                        (settings.1, COREWEBVIEW2_BROWSING_DATA_KINDS_DISK_CACHE),
                        (
                            settings.2,
                            COREWEBVIEW2_BROWSING_DATA_KINDS_BROWSING_HISTORY,
                        ),
                    ] {
                        if hours == 0 {
                            continue;
                        }
                        let cutoff = now.saturating_sub(hours.saturating_mul(3600)) as f64;
                        let handler =
                            ClearBrowsingDataCompletedHandler::create(Box::new(|_| Ok(())));
                        profile2.ClearBrowsingDataInTimeRange(kind, 0.0, cutoff, &handler)?;
                    }
                }
                Ok(())
            })();

            match result {
                Ok(()) => {
                    if let Some(p) = self.p.profiles.iter_mut().find(|p| p.id == profile_id) {
                        p.last_cleanup_at = now;
                    }
                    self.save();
                }
                Err(e) => log::warn!("profile browsing-data cleanup failed: {e:#}"),
            }
        }
    }

    /// Age-scoped profile cleanup is only available through WebView2.
    #[cfg(not(windows))]
    pub(super) fn cleanup_profile_data(&mut self) {}
}
