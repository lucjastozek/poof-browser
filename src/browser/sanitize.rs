//! Cleaning of untrusted text: addresses, colors, icon names and favicons.

use super::*;

/// Turns what the user typed into an address; plain text becomes a search with `engine`.
pub fn normalize(input: &str, engine: &str) -> String {
    let s = input.trim();
    if s.is_empty() {
        return "about:blank".into();
    }
    if let Ok(u) = url::Url::parse(s) {
        if matches!(u.scheme(), "http" | "https") || s == "about:blank" {
            return s.to_string();
        }
    }
    if !s.contains(char::is_whitespace) {
        if s.starts_with("localhost") || s.starts_with("127.0.0.1") {
            return format!("http://{s}");
        }
        if s.contains('.') {
            return format!("https://{s}");
        }
    }
    search::search_url(engine, s)
}

pub(super) fn ok_url(s: &str, max: usize, allow_data: bool) -> bool {
    s.len() <= max
        && (s.starts_with("http://")
            || s.starts_with("https://")
            || (allow_data && s.starts_with("data:image/")))
}

/// An address handed over by another launch of the browser: a web link or a local file.
pub(super) fn ok_external_url(s: &str) -> bool {
    let s = s.trim();
    s.len() <= EXTERNAL_URL_MAX
        && [("http://", 7), ("https://", 8), ("file://", 7)]
            .iter()
            .any(|(scheme, len)| {
                s.len() > *len
                    && s.get(..*len)
                        .is_some_and(|head| head.eq_ignore_ascii_case(scheme))
            })
}

pub(super) fn clip(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

pub(super) fn ok_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

pub(super) fn clean_icon(s: &str) -> String {
    if valid_icon(s) {
        s.to_string()
    } else {
        "house".into()
    }
}

/// Keeps the last usable icon when WebView2 reports a transient empty result.
pub(super) fn update_favicon(current: &mut Option<String>, incoming: Option<String>) -> bool {
    let Some(favicon) = incoming.filter(|value| ok_url(value, FAVICON_MAX, true)) else {
        return false;
    };
    if current.as_deref() == Some(favicon.as_str()) {
        return false;
    }
    *current = Some(favicon);
    true
}

pub(super) fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(1)
        .max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_addresses_are_links_or_local_files_only() {
        assert!(ok_external_url("https://example.com/a"));
        assert!(ok_external_url("HTTP://example.com"));
        assert!(ok_external_url("file:///C:/docs/a.html"));
        assert!(!ok_external_url("javascript:alert(1)"));
        assert!(!ok_external_url("data:text/html,<p>"));
        assert!(!ok_external_url("https://"));
        assert!(!ok_external_url(&format!(
            "https://x/{}",
            "a".repeat(EXTERNAL_URL_MAX)
        )));
    }
}
