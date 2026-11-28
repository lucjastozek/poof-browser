//! Undo for destructive sidebar actions: closing tabs, clearing, removing a favorite or pin,
//! deleting a folder or a space. Each action pushes one entry and shows one toast with Undo;
//! Ctrl+Z in the sidebar (or the Undo button) restores the latest entry.
//!
//! Nothing here keeps a WebView alive: a restored tab comes back asleep and loads when shown.

use super::*;
use crate::state::LinkRule;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

const MAX_UNDO: usize = 10;

/// For this long after an undoable action, Ctrl+Z also works while a page has focus (when the
/// page is not in a text field), so it never steals a page's own undo for long.
const SOFT_UNDO_MS: u64 = 8_000;
static UNDO_UNTIL: AtomicU64 = AtomicU64::new(0);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// True while Ctrl+Z from a page should be offered as undo (read by the key handler).
pub(super) fn undo_window_open() -> bool {
    now_ms() < UNDO_UNTIL.load(Ordering::Relaxed)
}

fn open_undo_window() {
    UNDO_UNTIL.store(now_ms() + SOFT_UNDO_MS, Ordering::Relaxed);
}

/// Is the page's focused element something that has its own undo (a field, an editor, a frame)?
const EDITING_JS: &str = "(()=>{let a=document.activeElement;while(a&&a.shadowRoot&&a.shadowRoot.activeElement)a=a.shadowRoot.activeElement;return !!(a&&(a.isContentEditable||/^(INPUT|TEXTAREA|SELECT|IFRAME)$/.test(a.tagName)))})()";

/// A tab taken out of `Browser::tabs`, with the position it had there.
pub(super) struct ClosedTab {
    index: usize,
    tab: Tab,
    was_visible: bool,
}

/// What closing collected while an undoable command runs (see `close_tab`).
#[derive(Default)]
pub(super) struct UndoRec {
    closed: Vec<ClosedTab>,
    /// Pinned or favorite tabs that were only reset to their home page: (id, url, was visible).
    resets: Vec<(String, String, bool)>,
}

impl UndoRec {
    pub(super) fn closed_tab(&mut self, index: usize, tab: Tab, was_visible: bool) {
        self.closed.push(ClosedTab {
            index,
            tab,
            was_visible,
        });
    }

    pub(super) fn reset(&mut self, id: &str, url: &str, was_visible: bool) {
        self.resets
            .push((id.to_string(), url.to_string(), was_visible));
    }
}

pub(super) enum Undo {
    Tabs {
        closed: Vec<ClosedTab>,
        resets: Vec<(String, String, bool)>,
    },
    Folder {
        folder: TabFolder,
        members: Vec<String>,
    },
    Kind {
        id: String,
        kind: Kind,
        space_id: Option<String>,
        home: String,
    },
    Space {
        space: Space,
        index: usize,
        tabs: Vec<ClosedTab>,
        folders: Vec<TabFolder>,
        rules: Vec<LinkRule>,
        fallback: Option<RuleTarget>,
        was_current: bool,
    },
}

fn name_of(title: &str, url: &str) -> String {
    let t = title.trim();
    clip(if t.is_empty() { url } else { t }, 36)
}

impl Browser {
    fn push_undo(&mut self, label: String, undo: Undo) {
        self.call_sidebar("__undoToast", &json!(label));
        open_undo_window();
        self.undo_stack.push((label, undo));
        if self.undo_stack.len() > MAX_UNDO {
            self.undo_stack.remove(0);
        }
    }

    /// Turns what `close_tab` collected into one undo entry.
    fn finish_tab_undo(&mut self) {
        let Some(rec) = self.undo_rec.take() else {
            return;
        };
        let n = rec.closed.len() + rec.resets.len();
        if n == 0 {
            return;
        }
        let label = if n > 1 {
            format!("Closed {n} tabs")
        } else if let Some(c) = rec.closed.first() {
            format!("Closed “{}”", name_of(&c.tab.title, &c.tab.url))
        } else {
            let id = &rec.resets[0].0;
            let name = self
                .tabs
                .iter()
                .find(|t| &t.id == id)
                .map(|t| name_of(&t.title, &t.home))
                .unwrap_or_default();
            format!("Reset “{name}” to its pinned page")
        };
        self.push_undo(
            label,
            Undo::Tabs {
                closed: rec.closed,
                resets: rec.resets,
            },
        );
    }

    pub(super) fn close_tab_undoable(&mut self, id: &str) -> Result<()> {
        self.undo_rec = Some(UndoRec::default());
        let result = self.close_tab(id);
        self.finish_tab_undo();
        result
    }

    pub(super) fn clear_tabs_undoable(&mut self) -> Result<()> {
        self.undo_rec = Some(UndoRec::default());
        let result = self.clear_tabs();
        self.finish_tab_undo();
        result
    }

    /// Unpin / remove from favorites.
    pub(super) fn unset_kind_undoable(&mut self, id: &str) {
        let Some(t) = self.tabs.iter().find(|t| t.id == id) else {
            return;
        };
        if t.kind == Kind::Normal {
            return;
        }
        let (kind, space_id, home) = (t.kind, t.space_id.clone(), t.home.clone());
        let label = if kind == Kind::Favorite {
            format!("Removed “{}” from favorites", name_of(&t.title, &t.url))
        } else {
            format!("Unpinned “{}”", name_of(&t.title, &t.url))
        };
        self.set_kind(id, Kind::Normal);
        self.push_undo(
            label,
            Undo::Kind {
                id: id.to_string(),
                kind,
                space_id,
                home,
            },
        );
    }

    pub(super) fn delete_folder_undoable(&mut self, id: &str) {
        let Some(folder) = self
            .p
            .folders
            .iter()
            .find(|f| f.id == id && f.space_id == self.p.current_space)
            .cloned()
        else {
            return;
        };
        let members: Vec<String> = self
            .tabs
            .iter()
            .filter(|t| t.folder_id.as_deref() == Some(id))
            .map(|t| t.id.clone())
            .collect();
        let label = format!("Deleted folder “{}”", clip(&folder.name, 30));
        self.delete_folder(id);
        self.push_undo(label, Undo::Folder { folder, members });
    }

    pub(super) fn delete_space_undoable(&mut self, id: &str) -> Result<()> {
        if self.p.spaces.len() <= 1 {
            return Ok(());
        }
        let Some(index) = self.p.spaces.iter().position(|s| s.id == id) else {
            return Ok(());
        };
        let space = self.p.spaces[index].clone();
        let tabs: Vec<ClosedTab> = self
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, t)| t.space_id.as_deref() == Some(id))
            .map(|(i, t)| ClosedTab {
                index: i,
                tab: t.snapshot(),
                was_visible: false,
            })
            .collect();
        let folders: Vec<TabFolder> = self
            .p
            .folders
            .iter()
            .filter(|f| f.space_id == id)
            .cloned()
            .collect();
        let rules_before = self.p.link_rules.clone();
        let fallback = self.p.link_fallback.clone();
        let was_current = self.p.current_space == id;
        self.delete_space(id)?;
        let rules: Vec<LinkRule> = rules_before
            .into_iter()
            .filter(|r| !self.p.link_rules.contains(r))
            .collect();
        let fallback = if self.p.link_fallback.is_none() {
            fallback
        } else {
            None
        };
        let label = format!("Deleted space “{}”", clip(&space.name, 30));
        self.push_undo(
            label,
            Undo::Space {
                space,
                index,
                tabs,
                folders,
                rules,
                fallback,
                was_current,
            },
        );
        Ok(())
    }

    /// Puts a tab back and, when it was showing, shows it again.
    fn reinsert(&mut self, closed: ClosedTab) -> Option<String> {
        let ClosedTab {
            index,
            mut tab,
            was_visible,
        } = closed;
        if self.tabs.iter().any(|t| t.id == tab.id) {
            return None;
        }
        if let Some(space) = &tab.space_id {
            if !self.p.spaces.iter().any(|s| &s.id == space) {
                return None;
            }
        }
        if let Some(folder) = &tab.folder_id {
            if !self.p.folders.iter().any(|f| &f.id == folder) {
                tab.folder_id = None;
            }
        }
        if let Some(p) = self.closed_tabs.iter().position(|u| *u == tab.url) {
            self.closed_tabs.remove(p);
        }
        let id = tab.id.clone();
        let at = index.min(self.tabs.len());
        self.tabs.insert(at, tab);
        was_visible.then_some(id)
    }

    /// Shows a restored tab, switching to its space first when needed.
    fn show_restored(&mut self, id: &str) -> Result<()> {
        let space = self
            .tabs
            .iter()
            .find(|t| t.id == id)
            .and_then(|t| t.space_id.clone());
        if let Some(space) = space {
            if space != self.p.current_space {
                self.switch_space(&space)?;
            }
        }
        self.activate(id)
    }

    /// Ctrl+Shift+T: puts the last closed tab back exactly where it was; when the last thing
    /// undoable was something else, it reopens the most recent closed address as before.
    pub(super) fn reopen_or_undo(&mut self) -> Result<()> {
        let tab_close_last = matches!(
            self.undo_stack.last(),
            Some((_, Undo::Tabs { closed, .. })) if !closed.is_empty()
        );
        if tab_close_last {
            self.undo_last()
        } else {
            self.reopen_closed_tab()
        }
    }

    /// Ctrl+Z reached the page: `undo` runs at once, `undo_soft` first checks that the page is
    /// not in a text field (whose own undo must win).
    pub(super) fn on_undo_key(&mut self, k: &str) {
        if k == "undo" {
            if let Err(e) = self.undo_last() {
                log::error!("{e:#}");
            }
            return;
        }
        if self.undo_stack.is_empty() {
            return;
        }
        let Some(wv) = self.visible_webview() else {
            return;
        };
        let proxy = self.proxy.clone();
        let _ = wv.evaluate_script_with_callback(EDITING_JS, move |editing| {
            if editing.trim() == "false" {
                let _ = proxy.send_event(UserEvent::Page {
                    tab: String::new(),
                    body: json!({ "t": "key", "k": "undo" }).to_string(),
                });
            }
        });
    }

    /// Brings the empty-list tips back (Settings or the command palette).
    pub(super) fn reset_hints(&mut self) {
        self.p.used_favorites = false;
        self.p.used_pinned = false;
        // Tips only show between the 3rd and 49th tab opened, so put the count inside that range.
        self.p.tabs_opened = self.p.tabs_opened.clamp(3, 49);
        self.save();
        self.toast("Tips are back where lists are empty");
    }

    pub(super) fn undo_last(&mut self) -> Result<()> {
        let Some((_, undo)) = self.undo_stack.pop() else {
            self.toast("Nothing to undo");
            return Ok(());
        };
        let mut show: Option<String> = None;
        let mut said = "Restored";
        match undo {
            Undo::Tabs { closed, resets } => {
                for (id, url, was_visible) in resets {
                    if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
                        t.url = url;
                        if was_visible {
                            show = Some(id);
                        }
                    }
                }
                // Reverse of removal order, so every tab lands on the index it left.
                for c in closed.into_iter().rev() {
                    if let Some(id) = self.reinsert(c) {
                        show = Some(id);
                    }
                }
            }
            Undo::Folder { folder, members } => {
                let space_ok = self.p.spaces.iter().any(|s| s.id == folder.space_id);
                if space_ok && !self.p.folders.iter().any(|f| f.id == folder.id) {
                    let fid = folder.id.clone();
                    self.p.folders.push(folder);
                    for t in self
                        .tabs
                        .iter_mut()
                        .filter(|t| members.contains(&t.id) && t.folder_id.is_none())
                    {
                        t.folder_id = Some(fid.clone());
                    }
                }
            }
            Undo::Kind {
                id,
                kind,
                space_id,
                home,
            } => {
                if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
                    t.kind = kind;
                    t.space_id = space_id;
                    t.home = home;
                }
            }
            Undo::Space {
                space,
                index,
                tabs,
                folders,
                rules,
                fallback,
                was_current,
            } => {
                let profile_ok = self.p.profiles.iter().any(|p| p.id == space.profile_id);
                if !profile_ok || self.p.spaces.iter().any(|s| s.id == space.id) {
                    self.toast("Couldn't restore the space");
                    return Ok(());
                }
                let id = space.id.clone();
                let at = index.min(self.p.spaces.len());
                self.p.spaces.insert(at, space);
                self.p.folders.extend(folders);
                for c in tabs {
                    self.reinsert(c);
                }
                for rule in rules {
                    if !self.p.link_rules.contains(&rule) {
                        self.p.link_rules.push(rule);
                    }
                }
                if self.p.link_fallback.is_none() {
                    self.p.link_fallback = fallback;
                }
                if was_current {
                    self.switch_space(&id)?;
                }
                said = "Space restored";
            }
        }
        if let Some(id) = show {
            self.show_restored(&id)?;
        }
        self.save();
        self.toast(said);
        if !self.undo_stack.is_empty() {
            open_undo_window();
        }
        Ok(())
    }
}
