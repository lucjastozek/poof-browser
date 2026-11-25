//! The search engines the address bar can use for text that is not an address.

/// Id used when nothing (or something unknown) is saved.
pub const DEFAULT_ENGINE: &str = "duckduckgo";

/// Id, display name and query URL; `%s` is replaced by the percent-encoded query.
pub const ENGINES: &[(&str, &str, &str)] = &[
    ("duckduckgo", "DuckDuckGo", "https://duckduckgo.com/?q=%s"),
    ("google", "Google", "https://www.google.com/search?q=%s"),
    ("bing", "Bing", "https://www.bing.com/search?q=%s"),
    (
        "brave",
        "Brave Search",
        "https://search.brave.com/search?q=%s",
    ),
    ("ecosia", "Ecosia", "https://www.ecosia.org/search?q=%s"),
    (
        "startpage",
        "Startpage",
        "https://www.startpage.com/do/search?q=%s",
    ),
];

pub fn is_engine(id: &str) -> bool {
    ENGINES.iter().any(|(engine, _, _)| *engine == id)
}

/// The URL that searches `query` with engine `id` (the default engine when `id` is unknown).
pub fn search_url(id: &str, query: &str) -> String {
    let template = ENGINES
        .iter()
        .find(|(engine, _, _)| *engine == id)
        .or_else(|| {
            ENGINES
                .iter()
                .find(|(engine, _, _)| *engine == DEFAULT_ENGINE)
        })
        .map_or(ENGINES[0].2, |(_, _, template)| template);
    let q: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    template.replace("%s", &q)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_query_url_per_engine() {
        assert_eq!(
            search_url("google", "rust & wry"),
            "https://www.google.com/search?q=rust+%26+wry"
        );
        assert_eq!(search_url("bing", "a"), "https://www.bing.com/search?q=a");
    }

    #[test]
    fn unknown_engine_falls_back_to_the_default() {
        assert_eq!(search_url("nope", "x"), "https://duckduckgo.com/?q=x");
        assert!(!is_engine("nope"));
        assert!(is_engine(DEFAULT_ENGINE));
    }

    #[test]
    fn every_engine_has_a_query_placeholder() {
        assert!(ENGINES.iter().all(|(_, _, url)| url.contains("%s")));
    }
}
