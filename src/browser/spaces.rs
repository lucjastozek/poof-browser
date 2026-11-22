//! Spaces: groups of tabs that share a profile and a color.

use super::*;

impl Browser {
    pub(super) fn cur_space(&self) -> &Space {
        self.p
            .spaces
            .iter()
            .find(|s| s.id == self.p.current_space)
            .unwrap_or(&self.p.spaces[0])
    }

    /// Tabs reachable from the current space, in sidebar order.
    pub(super) fn space_tab_ids(&self) -> Vec<String> {
        self.tabs
            .iter()
            .filter(|t| {
                t.space_id.as_deref() == Some(self.p.current_space.as_str())
                    || t.kind == Kind::Favorite
            })
            .map(|t| t.id.clone())
            .collect()
    }

    pub(super) fn restore_space_tab(&mut self) -> Result<()> {
        let sp = self.p.current_space.clone();
        let remembered = self
            .active
            .get(&sp)
            .cloned()
            .filter(|id| self.tabs.iter().any(|t| &t.id == id));
        let pick = remembered.or_else(|| {
            self.tabs
                .iter()
                .filter(|t| t.space_id.as_deref() == Some(sp.as_str()))
                .min_by_key(|t| (t.kind != Kind::Pinned) as u8)
                .map(|t| t.id.clone())
        });
        if let Some(id) = pick {
            self.activate(&id)?;
        } else if self.devtools_open {
            self.close_devtools();
        }
        Ok(())
    }

    pub(super) fn switch_space(&mut self, id: &str) -> Result<()> {
        if !self.p.spaces.iter().any(|s| s.id == id) || self.p.current_space == id {
            return Ok(());
        }
        self.hide_visible(); // triggers auto-PiP for the outgoing tab
        self.p.current_space = id.to_string();
        self.restore_space_tab()?;
        self.save();
        Ok(())
    }

    pub(super) fn cycle_space(&mut self, dir: i32) -> Result<()> {
        let n = self.p.spaces.len() as i32;
        let cur = self
            .p
            .spaces
            .iter()
            .position(|s| s.id == self.p.current_space)
            .unwrap_or(0) as i32;
        let next = (cur + dir).rem_euclid(n) as usize;
        let id = self.p.spaces[next].id.clone();
        self.switch_space(&id)
    }

    /// Creates a space (with a new profile if asked) and switches to it.
    pub(super) fn add_space(&mut self, space: NewSpace) -> Result<()> {
        let NewSpace {
            name,
            icon,
            color,
            profile_id,
            new_profile,
        } = space;
        let pid = match new_profile
            .map(|n| clip(n.trim(), 40))
            .filter(|n| !n.is_empty())
        {
            Some(n) => {
                let id = new_id();
                self.p.profiles.push(Profile {
                    id: id.clone(),
                    name: n,
                    tab_retention_hours: 48,
                    cookie_retention_hours: 0,
                    cache_retention_hours: 0,
                    history_retention_hours: 0,
                    last_cleanup_at: 0,
                });
                id
            }
            None => profile_id
                .filter(|id| self.p.profiles.iter().any(|p| &p.id == id))
                .unwrap_or_else(|| self.cur_space().profile_id.clone()),
        };
        let id = new_id();
        self.p.spaces.push(Space {
            id: id.clone(),
            profile_id: pid,
            name: clip(name.trim(), 40),
            icon: clean_icon(&icon),
            color: if ok_color(&color) {
                color
            } else {
                "#6dc5e2".into()
            },
        });
        self.switch_space(&id)?;
        Ok(())
    }

    /// Renames a space, changes its icon or color.
    pub(super) fn edit_space(&mut self, id: &str, name: &str, icon: &str, color: String) {
        if let Some(s) = self.p.spaces.iter_mut().find(|s| s.id == id) {
            s.name = clip(name.trim(), 40);
            s.icon = clean_icon(icon);
            if ok_color(&color) {
                s.color = color;
            }
        }
        self.save();
        self.sync_scrollbar();
    }

    pub(super) fn delete_space(&mut self, id: &str) -> Result<()> {
        if self.p.spaces.len() <= 1 {
            return Ok(());
        }
        if self.p.current_space == id {
            self.hide_visible();
        }
        self.tabs.retain(|t| t.space_id.as_deref() != Some(id));
        self.p.spaces.retain(|s| s.id != id);
        // Folders of a deleted space would otherwise live in state.json forever.
        self.p.folders.retain(|f| f.space_id != id);
        self.active.remove(id);
        if self.p.current_space == id {
            self.p.current_space = self.p.spaces[0].id.clone();
            self.restore_space_tab()?;
        }
        self.prune_ext_done();
        self.save();
        Ok(())
    }
}
