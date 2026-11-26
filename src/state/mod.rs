//! Persisted browser data: the model types, state.json and history.json.

mod downloads;
mod history;
mod live;
mod model;
mod persisted;
mod storage;
#[cfg(test)]
mod tests;

pub use downloads::{
    file_name, load_downloads, save_downloads, unique_path, DownloadEntry, DownloadStatus,
};
pub use history::{load_history, save_history, HistoryEntry};
pub use live::{
    LiveFolder, LiveItem, LiveSource, LIVE_INTERVAL_MAX, LIVE_INTERVAL_MIN, LIVE_ITEMS_MAX,
};
pub use model::{new_id, valid_icon, Kind, Profile, Saved, Space, TabFolder};
pub use persisted::Persisted;
