//! "Copy clean link": the page address without tracking parameters.

use super::*;

/// Parameters that only identify a campaign or a click, on any site.
const TRACKING_KEYS: &[&str] = &[
    "fbclid",
    "gclid",
    "gclsrc",
    "dclid",
    "gbraid",
    "wbraid",
    "msclkid",
    "yclid",
    "twclid",
    "ttclid",
    "li_fat_id",
    "igshid",
    "igsh",
    "epik",
    "scid",
    "mc_cid",
    "mc_eid",
    "mkt_tok",
    "vero_id",
    "oly_enc_id",
    "oly_anon_id",
    "rb_clickid",
    "s_cid",
    "wickedid",
    "ncid",
    "cmpid",
    "trk",
    "trkinfo",
    "ref_src",
    "ref_url",
    "_ga",
    "_gl",
    "spm",
    "share_id",
];
/// Parameter name prefixes of the same kind (`utm_source`, Amazon's `pf_rd_p`, ...).
const TRACKING_PREFIXES: &[&str] = &["utm_", "pk_", "mtm_", "hsa_", "_hs", "pd_rd_", "pf_rd_"];
/// Parameters that are tracking on one site but meaningful elsewhere: (host suffix, key).
const SITE_KEYS: &[(&str, &str)] = &[
    ("youtube.com", "si"),
    ("youtube.com", "feature"),
    ("youtu.be", "si"),
    ("youtu.be", "feature"),
    ("spotify.com", "si"),
    ("twitter.com", "s"),
    ("twitter.com", "t"),
    ("x.com", "s"),
    ("x.com", "t"),
    ("amazon.com", "ref"),
    ("amazon.com", "ref_"),
    ("amazon.com", "tag"),
    ("amazon.com", "linkCode"),
    ("amazon.com", "linkId"),
];

fn host_matches(host: &str, suffix: &str) -> bool {
    host == suffix || host.ends_with(&format!(".{suffix}"))
}

fn is_tracking(host: &str, key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    TRACKING_KEYS.contains(&key.as_str())
        || TRACKING_PREFIXES.iter().any(|p| key.starts_with(p))
        || SITE_KEYS
            .iter()
            .any(|(h, k)| host_matches(host, h) && key == k.to_ascii_lowercase())
}

/// Returns the address without tracking parameters and how many were removed. Anything that is
/// not an http(s) address, and addresses with nothing to remove, come back byte for byte.
pub(super) fn clean_link(raw: &str) -> (String, usize) {
    let unchanged = || (raw.to_string(), 0);
    let Ok(mut parsed) = url::Url::parse(raw) else {
        return unchanged();
    };
    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    if !matches!(parsed.scheme(), "http" | "https") {
        return unchanged();
    }
    let Some(query) = parsed.query().map(str::to_owned) else {
        return unchanged();
    };
    // Kept parameters stay as written: re-encoding them could change what the page receives.
    let (kept, dropped): (Vec<&str>, Vec<&str>) = query
        .split('&')
        .filter(|part| !part.is_empty())
        .partition(|part| !is_tracking(&host, part.split('=').next().unwrap_or_default()));
    if dropped.is_empty() {
        return unchanged();
    }
    let rest = kept.join("&");
    parsed.set_query(if rest.is_empty() { None } else { Some(&rest) });
    (parsed.to_string(), dropped.len())
}

impl Browser {
    /// Copies the address of `tab` without tracking parameters, through the sidebar (it owns the
    /// clipboard code and the toast). The sidebar needs focus and a user gesture to write to the
    /// clipboard, and hands focus back to the tab when it is done.
    pub(super) fn copy_clean_link(&self, tab: &str) {
        let Some(t) = self.tabs.iter().find(|t| t.id == tab) else {
            return;
        };
        if !matches!(t.url.split(':').next(), Some("http" | "https")) {
            return;
        }
        let (link, removed) = clean_link(&t.url);
        let message = match removed {
            0 => "Link copied, nothing to clean".to_string(),
            1 => "Clean link copied, 1 tracker removed".to_string(),
            n => format!("Clean link copied, {n} trackers removed"),
        };
        let _ = self.sidebar.focus();
        crate::gesture::eval_with_gesture(
            &self.sidebar,
            &format!(
                "window.__copyLink&&window.__copyLink({},{},{})",
                json!(link),
                json!(message),
                json!(tab)
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::clean_link;

    #[test]
    fn removes_campaign_and_click_parameters() {
        let (link, n) =
            clean_link("https://example.com/a?id=7&utm_source=x&fbclid=abc&utm_medium=y");
        assert_eq!(link, "https://example.com/a?id=7");
        assert_eq!(n, 3);
    }

    #[test]
    fn drops_the_question_mark_when_nothing_is_left() {
        let (link, n) = clean_link("https://example.com/a?utm_source=x&gclid=1#top");
        assert_eq!(link, "https://example.com/a#top");
        assert_eq!(n, 2);
    }

    #[test]
    fn leaves_clean_and_foreign_addresses_alone() {
        for raw in [
            "https://example.com/a?q=rust+lang&page=2",
            "https://example.com/",
            "about:blank",
            "file:///C:/a.html?utm_source=x",
            "not a url",
        ] {
            assert_eq!(clean_link(raw), (raw.to_string(), 0));
        }
    }

    #[test]
    fn keeps_remaining_parameters_exactly_as_written() {
        let (link, _) = clean_link("https://example.com/s?q=a%20b&utm_x=1&tags=a,b");
        assert_eq!(link, "https://example.com/s?q=a%20b&tags=a,b");
    }

    #[test]
    fn site_specific_parameters_only_apply_on_their_site() {
        assert_eq!(
            clean_link("https://youtu.be/abc?si=zzz&t=42").0,
            "https://youtu.be/abc?t=42"
        );
        assert_eq!(
            clean_link("https://www.youtube.com/watch?v=abc&si=zzz").0,
            "https://www.youtube.com/watch?v=abc"
        );
        // `si` and `s` mean something on other sites.
        assert_eq!(clean_link("https://example.com/?si=1&s=2").1, 0);
        assert_eq!(
            clean_link("https://x.com/u/status/1?s=20&t=abc").0,
            "https://x.com/u/status/1"
        );
    }

    #[test]
    fn matches_parameter_names_case_insensitively() {
        assert_eq!(
            clean_link("https://example.com/?UTM_Source=x&FBCLID=1").1,
            2
        );
    }
}
