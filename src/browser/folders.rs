//! Tab folders of the current space.

use super::*;

impl Browser {
    /// Adds an empty folder to the current space.
    pub(super) fn create_folder(&mut self, name: &str) {
        let name = clip(name.trim(), 40);
        if !name.is_empty() {
            let space_id = self.cur_space().id.clone();
            self.p.folders.push(TabFolder {
                id: new_id(),
                space_id,
                name,
                collapsed: false,
                pinned: false,
                order: usize::MAX,
            });
            self.save();
        }
    }

    /// Renames a folder of the current space.
    pub(super) fn rename_folder(&mut self, id: &str, name: &str) {
        let name = clip(name.trim(), 40);
        if !name.is_empty() {
            if let Some(f) = self
                .p
                .folders
                .iter_mut()
                .find(|f| f.id == id && f.space_id == self.p.current_space)
            {
                f.name = name;
                self.save();
            }
        }
    }

    /// Deletes a folder; its tabs stay open, outside any folder.
    pub(super) fn delete_folder(&mut self, id: &str) {
        if self
            .p
            .folders
            .iter()
            .any(|f| f.id == id && f.space_id == self.p.current_space)
        {
            self.p.folders.retain(|f| f.id != id);
            for t in self
                .tabs
                .iter_mut()
                .filter(|t| t.folder_id.as_deref() == Some(id))
            {
                t.folder_id = None;
            }
            self.save();
        }
    }

    /// Expands or collapses a folder in the sidebar.
    pub(super) fn toggle_folder_collapsed(&mut self, id: &str) {
        if let Some(f) = self
            .p
            .folders
            .iter_mut()
            .find(|f| f.id == id && f.space_id == self.p.current_space)
        {
            f.collapsed = !f.collapsed;
            self.save();
        }
    }

    /// Moves a folder between the pinned and normal sections, to `index`.
    pub(super) fn move_folder(&mut self, id: &str, to: &str, index: usize) {
        let space = self.p.current_space.clone();
        let pinned = to == "pinned";
        if pinned || to == "tab" {
            if let Some(from) = self
                .p
                .folders
                .iter()
                .position(|f| f.id == id && f.space_id == space)
            {
                let mut folder = self.p.folders.remove(from);
                let old_order = folder.order;
                let old_pinned = folder.pinned;
                // Close the gap in the source section, then make room at the exact
                // mixed tab/folder position selected by the pointer.
                for peer in self.p.folders.iter_mut().filter(|f| {
                    f.space_id == space && f.pinned == old_pinned && f.order > old_order
                }) {
                    peer.order = peer.order.saturating_sub(1);
                }
                for peer in self
                    .p
                    .folders
                    .iter_mut()
                    .filter(|f| f.space_id == space && f.pinned == pinned && f.order >= index)
                {
                    peer.order = peer.order.saturating_add(1);
                }
                folder.pinned = pinned;
                folder.order = index;
                self.p.folders.push(folder);
                let kind = if pinned { Kind::Pinned } else { Kind::Normal };
                for tab in self.tabs.iter_mut().filter(|t| {
                    t.folder_id.as_deref() == Some(id)
                        && t.space_id.as_deref() == Some(space.as_str())
                }) {
                    if tab.kind != kind {
                        tab.home = tab.url.clone();
                        tab.kind = kind;
                    }
                }
                self.save();
            }
        }
    }

    /// Moves a tab into a folder (or out of any folder with `None`).
    pub(super) fn move_tab_to_folder(
        &mut self,
        id: &str,
        folder_id: Option<String>,
        index: Option<usize>,
    ) {
        let space = self.p.current_space.clone();
        let target_pinned = folder_id
            .as_ref()
            .and_then(|fid| {
                self.p
                    .folders
                    .iter()
                    .find(|f| f.id == *fid && f.space_id == space)
            })
            .map(|f| f.pinned);
        if folder_id.is_none() || target_pinned.is_some() {
            if let Some(from) = self
                .tabs
                .iter()
                .position(|t| t.id == id && t.space_id.as_deref() == Some(space.as_str()))
            {
                let mut t = self.tabs.remove(from);
                if let Some(pinned) = target_pinned {
                    let kind = if pinned { Kind::Pinned } else { Kind::Normal };
                    if t.kind != kind {
                        t.home = t.url.clone();
                        t.kind = kind;
                    }
                }
                t.folder_id = folder_id.clone();
                let pos = if folder_id.is_some() {
                    // Insert before the index-th remaining tab of the folder, or after the last.
                    let peers: Vec<usize> = self
                        .tabs
                        .iter()
                        .enumerate()
                        .filter(|(_, x)| {
                            x.space_id.as_deref() == Some(space.as_str())
                                && x.folder_id == folder_id
                        })
                        .map(|(i, _)| i)
                        .collect();
                    match index.and_then(|i| peers.get(i).copied()) {
                        Some(p) => p,
                        None => peers.last().map(|i| i + 1).unwrap_or(self.tabs.len()),
                    }
                } else {
                    from.min(self.tabs.len())
                };
                self.tabs.insert(pos, t);
                self.save();
            }
        }
    }
}
