//! Which sites may have credentials. Credentials belong to one exact origin
//! (scheme + host + port); `https://a.com` and `https://a.com:8443` are different sites.

use url::{Host, Url};

/// The origin of a page URL, or `None` when passwords must not be saved or filled there:
/// only `https`, plus plain `http` on the local machine for development.
pub fn origin_of(page_url: &str) -> Option<String> {
    let url = Url::parse(page_url).ok()?;
    let secure = match url.scheme() {
        "https" => true,
        "http" => is_loopback(&url),
        _ => false,
    };
    (secure && url.host().is_some()).then(|| url.origin().ascii_serialization())
}

fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(name)) => name == "localhost",
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::origin_of;

    #[test]
    fn keeps_scheme_host_and_port_only() {
        assert_eq!(
            origin_of("https://Example.com/a/b?q=1#x").as_deref(),
            Some("https://example.com")
        );
        assert_eq!(
            origin_of("https://example.com:8443/").as_deref(),
            Some("https://example.com:8443")
        );
        assert_eq!(
            origin_of("https://example.com:443/").as_deref(),
            Some("https://example.com")
        );
    }

    #[test]
    fn insecure_and_internal_pages_have_no_origin() {
        for url in [
            "http://example.com",
            "about:blank",
            "file:///c:/a",
            "data:text/html,x",
            "javascript:1",
            "not a url",
        ] {
            assert_eq!(origin_of(url), None, "{url}");
        }
    }

    #[test]
    fn local_development_over_http_is_allowed() {
        assert_eq!(
            origin_of("http://localhost:3000/login").as_deref(),
            Some("http://localhost:3000")
        );
        assert_eq!(
            origin_of("http://127.0.0.1:8080").as_deref(),
            Some("http://127.0.0.1:8080")
        );
    }
}
