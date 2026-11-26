//! GitHub's issue search: the request address and the links in its answer.

use super::clean::clean_title;
use crate::state::{LiveItem, LIVE_ITEMS_MAX};
use anyhow::{anyhow, bail, Result};
use serde_json::Value;
use url::Url;

const SEARCH_URL: &str = "https://api.github.com/search/issues";
const QUERY_MAX_CHARS: usize = 256;
const WEB_PREFIX: &str = "https://github.com/";

/// The search request for `query`, newest activity first.
pub fn search_url(query: &str) -> Result<String> {
    let query = query.trim();
    if query.is_empty() || query.chars().count() > QUERY_MAX_CHARS {
        bail!("The GitHub search must be between 1 and {QUERY_MAX_CHARS} characters.");
    }
    let per_page = LIVE_ITEMS_MAX.to_string();
    let url = Url::parse_with_params(
        SEARCH_URL,
        [
            ("q", query),
            ("sort", "updated"),
            ("order", "desc"),
            ("per_page", per_page.as_str()),
        ],
    )?;
    Ok(url.into())
}

/// Titled `name#12 Fix the crash`: the repository name keeps searches over several repositories
/// readable, and the full address is behind the link.
pub fn parse(json: &str) -> Result<Vec<LiveItem>> {
    let root: Value =
        serde_json::from_str(json).map_err(|_| anyhow!("GitHub sent an unreadable answer."))?;
    let found = root["items"]
        .as_array()
        .ok_or_else(|| anyhow!("GitHub sent an unexpected answer."))?;
    Ok(found.iter().filter_map(item).take(LIVE_ITEMS_MAX).collect())
}

fn item(issue: &Value) -> Option<LiveItem> {
    let url = issue["html_url"]
        .as_str()
        .filter(|u| u.starts_with(WEB_PREFIX))?;
    let number = issue["number"].as_u64()?;
    let repository = issue["repository_url"]
        .as_str()
        .and_then(|u| u.rsplit('/').next())
        .unwrap_or_default();
    let title = issue["title"].as_str().unwrap_or_default();
    Some(LiveItem {
        title: clean_title(&format!("{repository}#{number} {title}"), url),
        url: url.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_query_is_encoded_into_the_search_address() {
        let url = search_url("is:pr is:open repo:a/b").unwrap();
        assert!(url
            .starts_with("https://api.github.com/search/issues?q=is%3Apr+is%3Aopen+repo%3Aa%2Fb&"));
        assert!(url.contains("sort=updated") && url.contains("per_page=25"));
        assert!(search_url("   ").is_err());
        assert!(search_url(&"x".repeat(300)).is_err());
    }

    #[test]
    fn reads_issues_and_pull_requests() {
        let json = r#"{"total_count":2,"items":[
          {"number":12,"title":"Fix the crash","html_url":"https://github.com/acme/poof/pull/12",
           "repository_url":"https://api.github.com/repos/acme/poof","pull_request":{}},
          {"number":7,"title":"Add dark mode","html_url":"https://github.com/acme/other/issues/7",
           "repository_url":"https://api.github.com/repos/acme/other"},
          {"number":9,"title":"Phish","html_url":"https://evil.example/x","repository_url":"https://api.github.com/repos/a/b"}
        ]}"#;
        let items = parse(json).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "poof#12 Fix the crash");
        assert_eq!(items[0].url, "https://github.com/acme/poof/pull/12");
        assert_eq!(items[1].title, "other#7 Add dark mode");
    }

    #[test]
    fn errors_and_garbage_are_refused() {
        assert!(parse(r#"{"message":"Validation Failed"}"#).is_err());
        assert!(parse("<html>").is_err());
    }
}
