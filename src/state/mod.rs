//! Persisted browser data: the model types, state.json and history.json.

mod history;
mod model;
mod persisted;
mod storage;
#[cfg(test)]
mod tests;

pub use history::{load_history, save_history, HistoryEntry};
pub use model::{new_id, valid_icon, Kind, Profile, Saved, Space, TabFolder};
pub use persisted::Persisted;
