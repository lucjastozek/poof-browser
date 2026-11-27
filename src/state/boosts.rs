//! Boosts: per-site custom CSS, JavaScript and zapped (hidden) elements.

use super::link_rules::{clean_pattern, host_of, RuleMatch};
use serde::{Deserialize, Serialize};

pub const BOOSTS_MAX: usize = 100;
pub const BOOST_CODE_MAX: usize = 100_000;
pub const BOOST_ZAP_MAX: usize = 200;
pub const BOOST_SELECTOR_MAX: usize = 500;

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct Boost {
    /// Bare host; also covers its subdomains.
    pub domain: String,
    #[serde(default = "on")]
    pub enabled: bool,
    #[serde(default)]
    pub css: String,
    #[serde(default)]
    pub js: String,
    /// CSS selectors of elements to hide.
    #[serde(default)]
    pub zap: Vec<String>,
}

fn on() -> bool {
    true
}

impl Boost {
    pub fn applies_to_host(&self, host: &str) -> bool {
        let host = host.strip_prefix("www.").unwrap_or(host);
        host == self.domain
            || host
                .strip_suffix(self.domain.as_str())
                .is_some_and(|rest| rest.ends_with('.'))
    }

    /// Trims the domain and limits sizes. `None` when the domain is unusable.
    pub fn cleaned(mut self) -> Option<Self> {
        self.domain = clean_pattern(&self.domain, RuleMatch::Domain)?;
        self.css = self.css.chars().take(BOOST_CODE_MAX).collect();
        self.js = self.js.chars().take(BOOST_CODE_MAX).collect();
        self.zap = clean_selectors(self.zap);
        Some(self)
    }
}

/// A selector is kept when it is short, single-line text that cannot close a CSS rule.
pub fn ok_selector(s: &str) -> bool {
    let s = s.trim();
    !s.is_empty()
        && s.chars().count() <= BOOST_SELECTOR_MAX
        && !s
            .chars()
            .any(|c| c.is_control() || matches!(c, '{' | '}' | '<'))
}

fn clean_selectors(list: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in list {
        let s = s.trim().to_string();
        if ok_selector(&s) && !out.contains(&s) && out.len() < BOOST_ZAP_MAX {
            out.push(s);
        }
    }
    out
}

/// Boosts that are on and cover the `http(s)` address `url`.
pub fn boosts_for<'a>(boosts: &'a [Boost], url: &str) -> Vec<&'a Boost> {
    match host_of(url) {
        Some(host) => boosts
            .iter()
            .filter(|b| b.enabled && b.applies_to_host(&host))
            .collect(),
        None => vec![],
    }
}
