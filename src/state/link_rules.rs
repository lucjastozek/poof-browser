//! Air Traffic Control: rules that decide which space (or profile) a link from another app opens in.
//!
//! Pure data and matching, so it can be tested without a window. Rules are checked top to bottom
//! and the first match wins; the order is the user's.

use super::model::{Profile, Space};
use serde::{Deserialize, Serialize};

/// Most rules a user can keep; the list is shown and saved in full, so it stays small.
pub const LINK_RULES_MAX: usize = 100;
/// Longest pattern text, in characters.
pub const LINK_PATTERN_MAX: usize = 200;

/// How a rule's pattern is compared with a link.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum RuleMatch {
    /// The link's host is the pattern or one of its subdomains (`github.com` covers `gist.github.com`).
    Domain,
    /// The pattern appears anywhere in the address, ignoring case.
    Contains,
}

/// Where a matching link opens.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum RuleTarget {
    /// This space.
    Space(String),
    /// The first space of this profile (the one already showing wins, see `resolve_space`).
    Profile(String),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct LinkRule {
    pub id: String,
    pub matches: RuleMatch,
    /// Already cleaned with `clean_pattern`.
    pub pattern: String,
    pub target: RuleTarget,
}

impl LinkRule {
    /// True when `url` (an `http(s)` address) is covered by this rule.
    pub fn applies_to(&self, url: &str) -> bool {
        match self.matches {
            RuleMatch::Contains => url.to_lowercase().contains(&self.pattern),
            RuleMatch::Domain => host_of(url).is_some_and(|host| {
                host == self.pattern
                    || host
                        .strip_suffix(self.pattern.as_str())
                        .is_some_and(|rest| rest.ends_with('.'))
            }),
        }
    }
}

/// The lowercase host of an `http(s)` link, without `www.`; `None` for anything else.
pub fn host_of(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let host = parsed.host_str()?.to_lowercase();
    Some(host.strip_prefix("www.").unwrap_or(&host).to_string())
}

/// Turns what the user typed into a pattern, or `None` when it cannot match anything useful.
/// For a domain, a pasted link, `www.`, `*.`, a port or a path are reduced to the bare host.
pub fn clean_pattern(input: &str, kind: RuleMatch) -> Option<String> {
    let text: String = input.trim().chars().take(LINK_PATTERN_MAX).collect();
    let text = text.to_lowercase();
    if text.is_empty() || text.chars().any(char::is_control) {
        return None;
    }
    if kind == RuleMatch::Contains {
        return Some(text);
    }
    let without_scheme = text
        .split_once("://")
        .map_or(text.as_str(), |(_, rest)| rest);
    let host = without_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .rsplit('@')
        .next()
        .unwrap_or_default();
    let host = host.rsplit_once(':').map_or(host, |(name, port)| {
        if port.chars().all(|c| c.is_ascii_digit()) {
            name
        } else {
            host
        }
    });
    let host = host.trim_start_matches("*.").trim_start_matches("www.");
    let valid = !host.is_empty()
        && !host.starts_with(['.', '-'])
        && !host.ends_with(['.', '-'])
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    valid.then(|| host.to_string())
}

/// The first rule whose target still exists and that covers `url`.
pub fn first_match<'a>(
    rules: &'a [LinkRule],
    url: &str,
    spaces: &[Space],
    profiles: &[Profile],
) -> Option<&'a RuleTarget> {
    first_rule(rules, url, spaces, profiles).map(|rule| &rule.target)
}

/// The first rule that covers `url` and points somewhere that still exists.
pub fn first_rule<'a>(
    rules: &'a [LinkRule],
    url: &str,
    spaces: &[Space],
    profiles: &[Profile],
) -> Option<&'a LinkRule> {
    rules
        .iter()
        .filter(|rule| target_exists(&rule.target, spaces, profiles))
        .find(|rule| rule.applies_to(url))
}

pub fn target_exists(target: &RuleTarget, spaces: &[Space], profiles: &[Profile]) -> bool {
    match target {
        RuleTarget::Space(id) => spaces.iter().any(|s| &s.id == id),
        RuleTarget::Profile(id) => {
            profiles.iter().any(|p| &p.id == id) && spaces.iter().any(|s| &s.profile_id == id)
        }
    }
}

/// The space a target stands for. A profile prefers the space already showing, so a link for
/// the profile you are in does not move you.
pub fn resolve_space<'a>(
    target: &RuleTarget,
    spaces: &'a [Space],
    current_space: &str,
) -> Option<&'a Space> {
    match target {
        RuleTarget::Space(id) => spaces.iter().find(|s| &s.id == id),
        RuleTarget::Profile(id) => {
            let mut of_profile = spaces.iter().filter(|s| &s.profile_id == id);
            let first = of_profile.next()?;
            std::iter::once(first)
                .chain(of_profile)
                .find(|s| s.id == current_space)
                .or(Some(first))
        }
    }
}
