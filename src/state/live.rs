//! Live folders: folders whose links come from an RSS/Atom feed or a GitHub search and refresh
//! themselves. Only the source and the last links are saved; the links are rebuilt on refresh.

use serde::{Deserialize, Serialize};
use url::Url;

/// Links kept per live folder. More would only make a long sidebar.
pub const LIVE_ITEMS_MAX: usize = 25;
pub const LIVE_INTERVAL_DEFAULT: u32 = 30;
/// Bounds of the refresh interval in minutes: often enough to be live, rare enough to be polite
/// to the server (GitHub allows 10 unauthenticated searches a minute).
pub const LIVE_INTERVAL_MIN: u32 = 5;
pub const LIVE_INTERVAL_MAX: u32 = 24 * 60;
/// After a failed refresh the next try comes sooner than a full interval, but never in a loop.
const LIVE_RETRY_MINUTES: u64 = 5;

/// Where a live folder gets its links from.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LiveSource {
    /// An RSS or Atom feed.
    Feed { url: String },
    /// Issues and pull requests matching a GitHub search query (`is:pr is:open repo:a/b`).
    Github { query: String },
}

/// Longest feed address or GitHub search accepted from the UI.
const SOURCE_URL_MAX: usize = 2048;
const SOURCE_QUERY_MAX_CHARS: usize = 256;

impl LiveSource {
    /// The source in a normal form, or `None` when it cannot work: a feed needs an `http(s)`
    /// address, a search needs some text. Everything from the UI passes through here.
    pub fn cleaned(self) -> Option<Self> {
        match self {
            Self::Feed { url } => {
                let parsed = Url::parse(url.trim()).ok()?;
                let web =
                    matches!(parsed.scheme(), "http" | "https") && parsed.host_str().is_some();
                let url = parsed.to_string();
                (web && url.len() <= SOURCE_URL_MAX).then_some(Self::Feed { url })
            }
            Self::Github { query } => {
                let query = query.split_whitespace().collect::<Vec<_>>().join(" ");
                let fits = !query.is_empty() && query.chars().count() <= SOURCE_QUERY_MAX_CHARS;
                fits.then_some(Self::Github { query })
            }
        }
    }

    /// A folder name to use when the user gave none.
    pub fn default_name(&self) -> String {
        match self {
            Self::Feed { url } => Url::parse(url)
                .ok()
                .and_then(|u| {
                    u.host_str()
                        .map(|h| h.trim_start_matches("www.").to_string())
                })
                .unwrap_or_else(|| "Feed".into()),
            Self::Github { .. } => "GitHub".into(),
        }
    }
}

/// One link of a live folder. The address is checked when it is fetched.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct LiveItem {
    pub title: String,
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LiveFolder {
    pub source: LiveSource,
    #[serde(default = "default_interval")]
    pub interval_minutes: u32,
    #[serde(default)]
    pub items: Vec<LiveItem>,
    /// Unix seconds of the last refresh attempt, successful or not; 0 = never.
    #[serde(default)]
    pub checked_at: u64,
    /// Why the last refresh failed; cleared by the next success.
    #[serde(default)]
    pub error: Option<String>,
}

fn default_interval() -> u32 {
    LIVE_INTERVAL_DEFAULT
}

impl LiveFolder {
    pub fn new(source: LiveSource, interval_minutes: u32) -> Self {
        Self {
            source,
            interval_minutes: interval_minutes.clamp(LIVE_INTERVAL_MIN, LIVE_INTERVAL_MAX),
            items: Vec::new(),
            checked_at: 0,
            error: None,
        }
    }

    /// Whether a refresh is due at `now` (Unix seconds). A clock that went backwards counts as due.
    pub fn is_due(&self, now: u64) -> bool {
        let minutes = if self.error.is_some() {
            LIVE_RETRY_MINUTES.min(u64::from(self.interval_minutes))
        } else {
            u64::from(self.interval_minutes)
        };
        self.checked_at == 0 || self.checked_at > now || now - self.checked_at >= minutes * 60
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed() -> LiveSource {
        LiveSource::Feed {
            url: "https://example.com/feed.xml".into(),
        }
    }

    #[test]
    fn a_new_folder_is_due_at_once_and_then_after_its_interval() {
        let mut live = LiveFolder::new(feed(), 30);
        assert!(live.is_due(1_000));
        live.checked_at = 1_000;
        assert!(!live.is_due(1_000 + 29 * 60));
        assert!(live.is_due(1_000 + 30 * 60));
    }

    #[test]
    fn failures_retry_sooner_but_not_immediately() {
        let mut live = LiveFolder::new(feed(), 120);
        live.checked_at = 1_000;
        live.error = Some("offline".into());
        assert!(!live.is_due(1_000 + 4 * 60));
        assert!(live.is_due(1_000 + 5 * 60));
    }

    #[test]
    fn a_clock_that_went_backwards_does_not_freeze_refreshing() {
        let mut live = LiveFolder::new(feed(), 30);
        live.checked_at = 10_000;
        assert!(live.is_due(5_000));
    }

    #[test]
    fn only_working_sources_are_accepted() {
        let github = |q: &str| LiveSource::Github { query: q.into() };
        let feed = |u: &str| LiveSource::Feed { url: u.into() };
        assert_eq!(
            feed(" https://example.com/feed ").cleaned(),
            Some(feed("https://example.com/feed"))
        );
        for bad in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "example.com/feed",
            "",
            "https://",
        ] {
            assert_eq!(feed(bad).cleaned(), None, "{bad}");
        }
        assert_eq!(
            github("  is:pr   is:open \n repo:a/b ").cleaned(),
            Some(github("is:pr is:open repo:a/b"))
        );
        assert_eq!(github("   ").cleaned(), None);
        assert_eq!(github(&"x".repeat(300)).cleaned(), None);
    }

    #[test]
    fn folders_are_named_after_their_source() {
        assert_eq!(feed().default_name(), "example.com");
        let github = LiveSource::Github {
            query: "is:pr".into(),
        };
        assert_eq!(github.default_name(), "GitHub");
    }

    #[test]
    fn the_interval_is_kept_within_bounds() {
        assert_eq!(
            LiveFolder::new(feed(), 0).interval_minutes,
            LIVE_INTERVAL_MIN
        );
        assert_eq!(
            LiveFolder::new(feed(), u32::MAX).interval_minutes,
            LIVE_INTERVAL_MAX
        );
    }

    #[test]
    fn saved_state_without_optional_fields_still_loads() {
        let json = r#"{"source":{"kind":"github","query":"is:pr is:open"}}"#;
        let live: LiveFolder = serde_json::from_str(json).unwrap();
        assert_eq!(live.interval_minutes, LIVE_INTERVAL_DEFAULT);
        assert!(live.items.is_empty() && live.checked_at == 0 && live.error.is_none());
    }
}
