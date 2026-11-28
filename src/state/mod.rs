//! Persisted browser data: the model types, state.json and history.json.

mod boosts;
mod downloads;
mod history;
mod link_rules;
mod live;
mod model;
mod persisted;
mod storage;
#[cfg(test)]
mod tests;

pub use boosts::{boosts_for, ok_selector, Boost, BOOSTS_MAX, BOOST_ZAP_MAX};
pub use downloads::{
    file_name, load_downloads, save_downloads, unique_path, DownloadEntry, DownloadStatus,
};
pub use history::{load_history, save_history, HistoryEntry};
pub use link_rules::host_of;
pub use link_rules::{
    clean_pattern, first_match, first_rule, resolve_space, target_exists, LinkRule, RuleMatch,
    RuleTarget, LINK_RULES_MAX,
};
pub use live::{
    LiveFolder, LiveItem, LiveSource, LIVE_INTERVAL_MAX, LIVE_INTERVAL_MIN, LIVE_ITEMS_MAX,
};
pub use model::{new_id, valid_icon, Kind, Profile, Saved, Space, TabFolder};
pub use persisted::{Persisted, SpaceSchedule, SCHEDULES_MAX};
