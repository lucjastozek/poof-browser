//! Browser extensions as seen from the browser: preinstalling, installing and listing them.

use super::*;

impl Browser {
    pub(super) fn preinstall(&mut self) {
        for id in store::PREINSTALL {
            let id = id.to_string();
            if self.p.preinstalled.contains(&id) {
                continue;
            }
            if self
                .root
                .join("extensions")
                .join(&id)
                .join("manifest.json")
                .is_file()
            {
                self.p.preinstalled.push(id);
            } else {
                self.spawn_install(id);
            }
        }
    }

    pub(super) fn spawn_install(&self, id: String) {
        let root = self.root.clone();
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let ev = match store::install(&root, &id) {
                Ok((dir, name)) => UserEvent::ExtResult {
                    ok: true,
                    msg: format!("Installed {name}"),
                    dir: Some(dir),
                    id: Some(id),
                },
                Err(e) => UserEvent::ExtResult {
                    ok: false,
                    msg: format!("Couldn’t install: {e}"),
                    dir: None,
                    id: Some(id),
                },
            };
            let _ = proxy.send_event(ev);
        });
    }

    pub(super) fn push_exts(&self) {
        let list = serde_json::to_string(&store::list(&self.root)).unwrap_or_else(|_| "[]".into());
        let _ = self
            .sidebar
            .evaluate_script(&format!("window.__setExts&&window.__setExts({list})"));
    }

    /// If the visible tab is a Chrome Web Store detail page, the extension's ID.
    pub(super) fn store_ext(&self) -> Option<String> {
        let t = self
            .visible
            .as_ref()
            .and_then(|id| self.tabs.iter().find(|t| &t.id == id))?;
        let u = url::Url::parse(&t.url).ok()?;
        match u.host_str()? {
            "chromewebstore.google.com" | "chrome.google.com" if u.path().contains("/detail/") => {
                store::parse_id(u.path())
            }
            _ => None,
        }
    }
}
