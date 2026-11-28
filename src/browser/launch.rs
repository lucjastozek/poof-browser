//! What happens when the browser opens: the space schedule and the "Continue" cards.

use super::*;

/// Most tabs remembered per space for its "Continue" card.
const RESUME_MAX: usize = 5;
/// A card needs at least this many tabs to be worth showing.
const RESUME_MIN: usize = 2;

impl Browser {
    /// Switches to the space a schedule rule names for the current local time. Returns the
    /// space's name when it moved, so the caller can say why.
    pub(super) fn apply_schedule(&mut self, local_min: Option<u16>) -> Option<String> {
        let minute = local_min.filter(|m| *m < 1440)?;
        let space = self
            .p
            .schedules
            .iter()
            .find(|r| r.covers(minute))
            .map(|r| r.space.clone())?;
        if space == self.p.current_space {
            return None;
        }
        let name = self.p.spaces.iter().find(|s| s.id == space)?.name.clone();
        self.p.current_space = space;
        self.save();
        Some(name)
    }

    pub(super) fn set_schedules(&mut self, rules: Vec<SpaceSchedule>) {
        let spaces = &self.p.spaces;
        // A rule whose times don't parse, or whose range is empty, covers no minute: dropped.
        self.p.schedules = rules
            .into_iter()
            .filter(|r| spaces.iter().any(|s| s.id == r.space))
            .filter(|r| (0..1440).any(|m| r.covers(m)))
            .take(crate::state::SCHEDULES_MAX)
            .collect();
        self.save();
    }

    /// Remembers which tabs each space is being used with: loaded tabs join its list (the
    /// visible one last), and tabs that no longer exist or are not saved leave it.
    pub(super) fn update_resume(&mut self) {
        let saved: HashSet<&str> = self.p.saved.iter().map(|s| s.id.as_str()).collect();
        let mut resume = std::mem::take(&mut self.p.resume);
        for t in self.tabs.iter() {
            let Some(space) = t.space_id.as_ref() else {
                continue;
            };
            if t.webview.is_none() || t.kind == Kind::Favorite || !saved.contains(t.id.as_str()) {
                continue;
            }
            let list = resume.entry(space.clone()).or_default();
            let visible = self.visible.as_deref() == Some(t.id.as_str());
            if visible {
                list.retain(|x| x != &t.id);
            }
            if !list.contains(&t.id) {
                list.push(t.id.clone());
            }
        }
        for (space, list) in resume.iter_mut() {
            list.retain(|id| {
                saved.contains(id.as_str())
                    && self
                        .tabs
                        .iter()
                        .any(|t| &t.id == id && t.space_id.as_ref() == Some(space))
            });
            let extra = list.len().saturating_sub(RESUME_MAX);
            list.drain(..extra);
        }
        resume.retain(|_, list| !list.is_empty());
        self.p.resume = resume;
    }

    /// Chooses the spaces that get a card on this launch (the current space first).
    pub(super) fn build_resume_cards(&mut self) {
        let current = self.p.current_space.clone();
        let mut ids: Vec<String> = self
            .p
            .spaces
            .iter()
            .filter(|s| self.resume_tabs(&s.id).len() >= RESUME_MIN)
            .map(|s| s.id.clone())
            .collect();
        ids.sort_by_key(|id| *id != current);
        self.resume_cards = ids;
    }

    /// The remembered tabs of a space that still exist, oldest first.
    fn resume_tabs(&self, space: &str) -> Vec<usize> {
        self.p
            .resume
            .get(space)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.tabs.iter().position(|t| &t.id == id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Card data for the sidebar: only spaces with something still asleep to wake.
    pub(super) fn resume_cards_json(&self) -> Vec<Value> {
        self.resume_cards
            .iter()
            .filter_map(|space| {
                let sp = self.p.spaces.iter().find(|s| &s.id == space)?;
                let idx = self.resume_tabs(space);
                if idx.len() < RESUME_MIN || idx.iter().all(|&i| self.tabs[i].webview.is_some()) {
                    return None;
                }
                // Most recent first, so the label starts with what was being used last.
                let names: Vec<String> = idx
                    .iter()
                    .rev()
                    .take(3)
                    .map(|&i| {
                        let t = &self.tabs[i];
                        let name = if t.title.trim().is_empty() {
                            t.url
                                .split("://")
                                .nth(1)
                                .unwrap_or(&t.url)
                                .split('/')
                                .next()
                                .unwrap_or("")
                        } else {
                            t.title.trim()
                        };
                        clip(name, 22)
                    })
                    .collect();
                Some(json!({
                    "space": sp.id, "name": sp.name, "color": sp.color,
                    "label": format!("Continue: {} ({} tabs)", names.join(", "), idx.len()),
                }))
            })
            .collect()
    }

    pub(super) fn dismiss_resume(&mut self, id: Option<&str>) {
        match id {
            Some(id) => self.resume_cards.retain(|s| s != id),
            None => self.resume_cards.clear(),
        }
        self.push_state();
    }

    /// Opens the space and wakes only the tabs it was last used with; other sleeping tabs
    /// stay asleep. The most recently used one is shown.
    pub(super) fn resume_space(&mut self, id: &str) -> Result<()> {
        if !self.p.spaces.iter().any(|s| s.id == id) {
            return Ok(());
        }
        self.resume_cards.retain(|s| s != id);
        let idx = self.resume_tabs(id);
        if idx.is_empty() {
            return Ok(());
        }
        if self.p.current_space != id {
            self.hide_visible();
            self.p.current_space = id.to_string();
        }
        let show = self.tabs[*idx.last().unwrap_or(&idx[0])].id.clone();
        let ids: Vec<String> = idx.iter().map(|&i| self.tabs[i].id.clone()).collect();
        for tab in ids.iter().filter(|t| **t != show) {
            if let Some(i) = self.tabs.iter().position(|t| &t.id == tab) {
                if let Err(e) = self.ensure_webview(i) {
                    log::warn!("could not wake a tab: {e:#}");
                }
            }
        }
        self.activate(&show)?;
        self.save();
        Ok(())
    }
}
