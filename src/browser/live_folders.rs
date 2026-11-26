//! Live folders: create, edit and refresh the folders whose links come from a feed or a search.
//! Fetching happens on short-lived threads (`crate::live`); results come back as a `UserEvent`.
//! Refreshes are scheduled by the existing maintenance tick, so an idle browser stays idle.

use super::*;
use crate::live;

/// Refreshes started per tick, so many due folders do not open many connections at once.
const MAX_REFRESHES_PER_TICK: usize = 4;
const FOLDER_NAME_MAX: usize = 40;
const ERROR_MAX: usize = 200;

impl Browser {
    /// Adds a live folder to the current space and fills it right away.
    pub(super) fn create_live_folder(
        &mut self,
        name: &str,
        source: LiveSource,
        interval_minutes: u32,
    ) {
        let Some(source) = source.cleaned() else {
            self.toast("That address or search is not valid.");
            return;
        };
        let name = clip(name.trim(), FOLDER_NAME_MAX);
        let id = new_id();
        let space_id = self.cur_space().id.clone();
        self.p.folders.push(TabFolder {
            id: id.clone(),
            space_id,
            name: if name.is_empty() {
                source.default_name()
            } else {
                name
            },
            collapsed: false,
            pinned: false,
            order: usize::MAX,
            live: Some(LiveFolder::new(source, interval_minutes)),
        });
        self.save();
        self.fetch_live_folder(&id);
    }

    /// Changes a live folder of the current space. A different source empties it and refills it.
    pub(super) fn edit_live_folder(
        &mut self,
        id: &str,
        name: &str,
        source: LiveSource,
        interval_minutes: u32,
    ) {
        let Some(source) = source.cleaned() else {
            self.toast("That address or search is not valid.");
            return;
        };
        let space = self.p.current_space.clone();
        let Some(folder) = self
            .p
            .folders
            .iter_mut()
            .find(|f| f.id == id && f.space_id == space)
        else {
            return;
        };
        let name = clip(name.trim(), FOLDER_NAME_MAX);
        if !name.is_empty() {
            folder.name = name;
        }
        let Some(live) = folder.live.as_mut() else {
            return;
        };
        live.interval_minutes = interval_minutes.clamp(LIVE_INTERVAL_MIN, LIVE_INTERVAL_MAX);
        let source_changed = live.source != source;
        if source_changed {
            *live = LiveFolder::new(source, live.interval_minutes);
        }
        self.save();
        if source_changed {
            self.fetch_live_folder(id);
        }
    }

    /// Refreshes one live folder now, whether or not it is due.
    pub(super) fn refresh_live_folder(&mut self, id: &str) {
        self.fetch_live_folder(id);
    }

    /// Starts a refresh for every live folder that is due. Called by the maintenance tick and when
    /// the sidebar first loads.
    pub fn refresh_due_live_folders(&mut self) {
        let now = now_secs();
        let due: Vec<String> = self
            .p
            .folders
            .iter()
            .filter(|f| f.live.as_ref().is_some_and(|live| live.is_due(now)))
            .map(|f| f.id.clone())
            .take(MAX_REFRESHES_PER_TICK)
            .collect();
        for id in due {
            self.fetch_live_folder(&id);
        }
    }

    /// Fetches on a worker thread. A folder already being fetched from the same source is left alone.
    fn fetch_live_folder(&mut self, id: &str) {
        let Some(source) = self.live_source(id) else {
            return;
        };
        if self.live_fetching.get(id) == Some(&source) {
            return;
        }
        let (proxy, folder, fetch_source) = (self.proxy.clone(), id.to_string(), source.clone());
        let spawned = std::thread::Builder::new()
            .name("live-folder".into())
            .spawn(move || {
                let fetched = std::panic::catch_unwind(|| {
                    live::fetch(&fetch_source).map_err(|e| format!("{e:#}"))
                });
                let result = fetched
                    .unwrap_or_else(|_| Err("Reading this source failed unexpectedly.".into()));
                let _ = proxy.send_event(UserEvent::LiveFolder {
                    id: folder,
                    source: fetch_source,
                    result,
                });
            });
        match spawned {
            Ok(_) => {
                self.live_fetching.insert(id.to_string(), source);
            }
            Err(error) => log::warn!("could not start a live folder refresh: {error}"),
        }
    }

    fn live_source(&self, id: &str) -> Option<LiveSource> {
        let folder = self.p.folders.iter().find(|f| f.id == id)?;
        Some(folder.live.as_ref()?.source.clone())
    }

    /// Stores what a refresh found. A result for a source the folder no longer has is dropped.
    pub(super) fn apply_live_result(
        &mut self,
        id: &str,
        source: &LiveSource,
        result: Result<Vec<LiveItem>, String>,
    ) {
        if self.live_fetching.get(id) == Some(source) {
            self.live_fetching.remove(id);
        }
        let Some(live) = self
            .p
            .folders
            .iter_mut()
            .find(|f| f.id == id)
            .and_then(|f| f.live.as_mut())
            .filter(|live| &live.source == source)
        else {
            return;
        };
        live.checked_at = now_secs();
        match result {
            Ok(items) => {
                live.items = items;
                live.error = None;
            }
            Err(message) => live.error = Some(clip(&message, ERROR_MAX)),
        }
        self.save();
        self.push_state();
    }
}
