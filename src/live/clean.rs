//! Making text and addresses from the network safe to show and open.

use url::Url;

const TITLE_MAX_CHARS: usize = 200;
pub const URL_MAX_BYTES: usize = 2048;

/// A link as an absolute `http(s)` address, or `None` for anything else (`javascript:`, `data:`,
/// `mailto:`...). `base` is the address the link was found at, for relative links.
pub fn resolve_link(base: &str, link: &str) -> Option<String> {
    let link = link.trim();
    if link.is_empty() {
        return None;
    }
    let absolute = match Url::parse(link) {
        Ok(url) => url,
        Err(_) => Url::parse(base).ok()?.join(link).ok()?,
    };
    let web = matches!(absolute.scheme(), "http" | "https");
    let text = absolute.to_string();
    (web && text.len() <= URL_MAX_BYTES).then_some(text)
}

/// Plain one-line text: tags and control characters removed, white space collapsed, shortened.
/// An empty title becomes the address without its scheme.
pub fn clean_title(raw: &str, url: &str) -> String {
    let mut text = String::with_capacity(raw.len());
    let mut in_tag = false;
    for c in raw.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if in_tag => {}
            c => text.push(if c.is_control() { ' ' } else { c }),
        }
    }
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return url
            .split_once("://")
            .map_or(url, |(_, rest)| rest)
            .chars()
            .take(TITLE_MAX_CHARS)
            .collect();
    }
    collapsed.chars().take(TITLE_MAX_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_web_addresses_survive() {
        let base = "https://example.com/a/feed";
        assert_eq!(
            resolve_link(base, " /x ").as_deref(),
            Some("https://example.com/x")
        );
        assert_eq!(
            resolve_link(base, "y").as_deref(),
            Some("https://example.com/a/y")
        );
        assert_eq!(
            resolve_link(base, "http://other.org/").as_deref(),
            Some("http://other.org/")
        );
        for bad in [
            "javascript:alert(1)",
            "data:text/html,hi",
            "mailto:a@b.c",
            "file:///etc/passwd",
            "",
        ] {
            assert_eq!(resolve_link(base, bad), None, "{bad}");
        }
    }

    #[test]
    fn titles_are_plain_single_line_and_bounded() {
        assert_eq!(
            clean_title("  A <b>bold</b>\n\ttitle ", "https://x.y/"),
            "A bold title"
        );
        assert_eq!(clean_title("", "https://x.y/z"), "x.y/z");
        assert_eq!(
            clean_title("a\u{202e}b\u{0}c", "https://x.y/")
                .chars()
                .count(),
            5
        );
        assert_eq!(
            clean_title(&"x".repeat(500), "https://x.y/")
                .chars()
                .count(),
            TITLE_MAX_CHARS
        );
    }
}
