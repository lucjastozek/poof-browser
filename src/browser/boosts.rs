//! Boosts: per-site CSS, JavaScript and zapped elements.
//!
//! CSS and zapped selectors are sent to the page's init script (`js/boost.js`) so they apply
//! before the first paint. JavaScript is run from here, which also works on pages whose
//! content security policy forbids inline script.

use super::*;

/// The init script for a new page WebView, with the boosts that are on.
pub(super) fn boost_script(boosts: &[Boost]) -> String {
    BOOST_JS.replace("__BOOSTS__", &boost_json(boosts))
}

fn boost_json(boosts: &[Boost]) -> String {
    let list: Vec<Value> = boosts
        .iter()
        .filter(|b| b.enabled)
        .map(
            |b| json!({ "d": b.domain, "css": b.css, "zap": b.zap, "js": !b.js.trim().is_empty() }),
        )
        .collect();
    // `</` and U+2028/9 cannot end up inside the script text.
    Value::Array(list)
        .to_string()
        .replace("</", "<\\/")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

fn wrap_js(code: &str) -> String {
    format!("(function(){{try{{\n{code}\n}}catch(e){{console.error('poof boost',e)}}}})()")
}

impl Browser {
    /// Sends the current boosts to every loaded page so CSS and zaps apply without a reload.
    fn push_boosts(&self) {
        let js = format!(
            "window.__poofBoosts&&window.__poofBoosts({})",
            boost_json(&self.p.boosts)
        );
        for t in &self.tabs {
            if let Some(wv) = &t.webview {
                let _ = wv.evaluate_script(&js);
            }
        }
    }

    /// Runs the JavaScript of every boost that covers the tab's address.
    pub(super) fn run_boost_js(&self, tab: &str) {
        let Some(t) = self.tabs.iter().find(|t| t.id == tab) else {
            return;
        };
        let Some(wv) = &t.webview else { return };
        for b in boosts_for(&self.p.boosts, &t.url) {
            if !b.js.trim().is_empty() {
                let _ = wv.evaluate_script(&wrap_js(&b.js));
            }
        }
    }

    pub(super) fn save_boost(
        &mut self,
        domain: String,
        enabled: bool,
        css: String,
        js: String,
        zap: Vec<String>,
    ) {
        let Some(boost) = (Boost {
            domain,
            enabled,
            css,
            js,
            zap,
        })
        .cleaned() else {
            self.toast("That doesn’t look like a site.");
            return;
        };
        let rerun = match self.p.boosts.iter().position(|b| b.domain == boost.domain) {
            Some(i) => {
                let changed = self.p.boosts[i].js != boost.js || !self.p.boosts[i].enabled;
                self.p.boosts[i] = boost.clone();
                changed
            }
            None if self.p.boosts.len() >= BOOSTS_MAX => {
                self.toast("That is the most boosts poof keeps. Remove one first.");
                return;
            }
            None => {
                self.p.boosts.push(boost.clone());
                true
            }
        };
        self.save();
        self.push_boosts();
        if rerun && boost.enabled {
            let ids: Vec<String> = self
                .tabs
                .iter()
                .filter(|t| boosts_for(std::slice::from_ref(&boost), &t.url).len() == 1)
                .map(|t| t.id.clone())
                .collect();
            for id in ids {
                self.run_boost_js(&id);
            }
        }
    }

    pub(super) fn delete_boost(&mut self, domain: &str) {
        self.p.boosts.retain(|b| b.domain != domain);
        self.save();
        self.push_boosts();
    }

    /// Starts the element picker on the page that is showing.
    pub(super) fn start_zap(&self) {
        if let Some(wv) = self.visible_webview() {
            let _ = wv.focus();
            let _ = wv.evaluate_script("window.__poofZap&&window.__poofZap()");
        }
    }

    /// The picker chose an element: hides it from now on on this tab's site.
    pub(super) fn add_zap(&mut self, tab: &str, selector: String) {
        let selector = selector.trim().to_string();
        if !ok_selector(&selector) {
            return;
        }
        let Some(host) = self
            .tabs
            .iter()
            .find(|t| t.id == tab)
            .and_then(|t| crate::state::host_of(&t.url))
        else {
            return;
        };
        // The most specific boost that already covers the site, else a new one for the host.
        let index = self
            .p
            .boosts
            .iter()
            .position(|b| b.applies_to_host(&host))
            .or_else(|| {
                if self.p.boosts.len() >= BOOSTS_MAX {
                    return None;
                }
                self.p.boosts.push(Boost {
                    domain: host.clone(),
                    enabled: true,
                    css: String::new(),
                    js: String::new(),
                    zap: vec![],
                });
                Some(self.p.boosts.len() - 1)
            });
        let Some(i) = index else {
            self.toast("That is the most boosts poof keeps. Remove one first.");
            return;
        };
        let boost = &mut self.p.boosts[i];
        if !boost.zap.contains(&selector) && boost.zap.len() < BOOST_ZAP_MAX {
            boost.zap.push(selector);
        }
        boost.enabled = true;
        self.save();
        self.push_boosts();
        self.toast("Element hidden. Undo it in Settings › Boosts.");
    }
}
