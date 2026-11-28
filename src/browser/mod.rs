//! The browser: window, sidebar, tabs and everything that connects them.
//!
//! `Browser` owns all state. Its behavior is split by topic into the sibling modules below, each
//! contributing an `impl Browser` block (tabs, spaces, split view, DevTools, ...). Free helpers
//! live in the small modules (`sanitize`, `geometry`, `platform`, ...); every module sees the
//! imports and helpers of this file through `use super::*`.

// Some helpers are only used by the Windows-specific code paths.
#![cfg_attr(not(windows), allow(dead_code, unused_imports, unused_variables))]

use crate::{
    default_browser,
    extensions::{self, store},
    passwords::PasswordStore,
    pip,
    state::{
        boosts_for, file_name, load_downloads, load_history, new_id, ok_selector, save_downloads,
        save_history, unique_path, valid_icon, Boost, DownloadEntry, DownloadStatus, HistoryEntry,
        Kind, LiveFolder, LiveItem, LiveSource, Persisted, Profile, RuleMatch, RuleTarget, Saved,
        Space, TabFolder, BOOSTS_MAX, BOOST_ZAP_MAX, LIVE_INTERVAL_MAX, LIVE_INTERVAL_MIN,
    },
    updater, UserEvent,
};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    cell::{Cell, RefCell},
    collections::{hash_map::DefaultHasher, BTreeMap, HashMap, HashSet},
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tao::{
    event_loop::EventLoopProxy,
    window::{Theme, Window},
};
use wry::{
    dpi::{LogicalPosition, LogicalSize},
    NewWindowResponse, Rect, WebContext, WebView, WebViewBuilder,
};

#[cfg(windows)]
use wry::WebViewExtWindows;

mod arc_import;
mod boosts;
mod capture;
mod clean_link;
mod commands;
mod constants;
mod context_menu;
mod default_app;
mod devtools;
mod devtools_icons;
mod download_hooks;
mod downloads;
mod events;
mod extensions_ui;
mod external;
mod folders;
mod fullscreen;
mod geometry;
mod history;
mod layout;
mod link_routing;
mod live_folders;
mod maintenance;
mod messages;
mod mini;
mod page_messages;
mod passwords;
mod pdf_viewer;
mod persistence;
mod platform;
mod popup;
mod popup_watch;
mod profile_cleanup;
mod sanitize;
mod scripts;
pub mod search;
mod settings;
mod shortcuts;
mod sidebar_page;
mod sidebar_state;
mod spaces;
mod split;
mod switcher;
mod tabs;
mod types;
mod undo;
mod update;
mod view_backdrop;
mod webview;
mod webview_events;

use constants::*;
use devtools::*;
use devtools_icons::*;
use download_hooks::*;
use downloads::Lingering;
use geometry::*;
use messages::*;
use persistence::*;
use platform::*;
use sanitize::*;
use scripts::*;
use shortcuts::*;
use sidebar_page::*;
use types::*;
use webview_events::*;

pub use persistence::prepare_data_dir;

pub struct Browser {
    window: Window,
    proxy: EventLoopProxy<UserEvent>,
    root: PathBuf,
    sidebar: WebView,
    ui_ctx: WebContext,
    /// One WebContext (= one user-data folder = one browser profile) per profile.
    contexts: HashMap<String, WebContext>,
    ext_done: HashSet<String>,
    p: Persisted,
    tabs: Vec<Tab>,
    /// Last active tab per space.
    active: HashMap<String, String>,
    visible: Option<String>,
    overlay: bool,
    /// Box of the Downloads popover (window coordinates) while it is open; cut out of the page.
    downloads_hole: Option<[f64; 4]>,
    fullscreen: bool,
    closed_tabs: Vec<String>,
    /// Destructive sidebar actions that can still be undone (newest last).
    undo_stack: Vec<(String, undo::Undo)>,
    /// Collects tabs closed by the command that is running, for one undo entry.
    undo_rec: Option<undo::UndoRec>,
    devtools_open: bool,
    devtools: Option<WebView>,
    devtools_tab: Option<String>,
    /// The browser's icon set (DevTools icon name -> SVG data URI), sanitized.
    devtools_icons: BTreeMap<String, String>,
    /// Bumped whenever `devtools_icons` changes; 0 = no icons received yet.
    devtools_icons_version: u64,
    /// The current DevTools frontend has applied the current icon version.
    devtools_icons_applied: bool,
    /// Unmatched DevTools icon names already logged.
    devtools_icon_misses: HashSet<String>,
    /// DevTools device mode is on: the frontend covers the whole content area and the page
    /// WebView is placed over the emulated screen.
    responsive_mode: bool,
    responsive_bounds: Option<[f64; 4]>,
    /// The poll thread only ticks while this is set (and is parked otherwise).
    devtools_polling: Arc<AtomicBool>,
    poll_thread: std::thread::Thread,
    /// Start time (ms) of the poll in flight, 0 when idle.
    poll_busy: Arc<AtomicU64>,
    chrome_shown: Cell<bool>,
    /// Hash of the last state pushed to the sidebar; identical states are not re-sent.
    last_state_hash: Cell<u64>,
    /// Cached new-tab suggestions, rebuilt only when visit stats change.
    sugg_cache: RefCell<Option<Vec<Value>>>,
    save_dirty: bool,
    last_save: Instant,
    /// Recent crashes per tab (window start, count), to avoid reload loops.
    crashes: HashMap<String, (Instant, u32)>,
    history: Vec<HistoryEntry>,
    history_dirty: bool,
    last_history_save: Instant,
    /// Filter currently typed into the history view.
    history_query: String,
    /// Last URL written to history per tab, so the same page is not logged twice.
    logged_urls: HashMap<String, String>,
    /// Tabs opened in the background that have not been visited yet (keeps ctrl+click order).
    bg_tabs: HashSet<String>,
    /// Whether the sidebar WebView is currently painted as a black backdrop (fullscreen).
    fs_css: Cell<bool>,
    /// The window was maximized when fullscreen started, so it can be maximized again afterwards.
    was_maximized: bool,
    /// Tab ids, most recently used first.
    mru: Vec<String>,
    switcher: Option<Switcher>,
    split: Option<Split>,
    peek: bool,
    scrollbar_visible: bool,
    split_chrome_shown: Cell<bool>,
    tab_drag: bool,
    saved_splits: Vec<Split>,
    /// Tabs where the user started a video (only those are worth waking for auto-PIP).
    vid_tabs: HashSet<String>,
    /// The link-peek overlay, when one is open.
    link_peek: Option<mini::Peek>,
    /// Media a page reported, by tab: (playing, title, artist). Shown as the mini player.
    media: HashMap<String, (bool, String, String)>,
    /// Open popup windows (sign-in, `window.open`), by window id.
    popups: HashMap<tao::window::WindowId, popup::Popup>,
    /// Tabs currently showing a Picture-in-Picture window: never slept or memory-trimmed.
    pip_tabs: HashSet<String>,
    /// Tabs whose page reported live capture or a call: never slept, never memory-trimmed.
    live_tabs: HashSet<String>,
    /// Tabs holding unsent form input or a long scroll position: never auto-slept.
    sleep_hold: HashSet<String>,
    /// Scroll / form state of tabs, restored when they wake.
    sleep_snaps: HashMap<String, SleepSnap>,
    /// Hidden tabs whose WebView2 memory target is currently LOW.
    low_mem: HashSet<String>,
    /// When the sidebar last received a state snapshot, and whether a newer one is waiting.
    last_push: Cell<Instant>,
    /// True only while a swipe switch pushes the new space's tabs before its WebView exists;
    /// tells the sidebar not to open the start page for the momentarily empty page area.
    switching_space: Cell<bool>,
    state_dirty: Cell<bool>,
    /// A downloaded, verified installer to run when the browser closes.
    pending_update: Option<PathBuf>,
    /// The page picture is being captured; the page is hidden once the sidebar shows it.
    awaiting_backdrop: bool,
    /// A newer release the user has not answered yet.
    offered_update: Option<updater::Update>,
    /// The user chose "Update now": install and restart as soon as the download is ready.
    update_now: bool,
    /// Saved logins (encrypted on disk, see `crate::passwords`).
    passwords: PasswordStore,
    /// A login the user hasn't answered the save prompt for yet.
    password_offer: Option<passwords::PasswordOffer>,
    /// An import from another browser is reading files on a background thread.
    importing: bool,
    /// Live folders being refreshed right now, with the source each fetch was started for.
    live_fetching: HashMap<String, LiveSource>,
    /// Newest first; running ones are backed by a WebView2 operation (`download_hooks`).
    downloads: Vec<DownloadEntry>,
    /// WebViews of closed tabs kept alive while their downloads run.
    lingering: Vec<Lingering>,
    /// The sidebar has loaded and asked for its first state.
    sidebar_ready: bool,
    /// Links handed over before the sidebar was ready; opened as soon as it is.
    startup_urls: Vec<String>,
}

impl Browser {
    pub fn new(window: Window, proxy: EventLoopProxy<UserEvent>, root: PathBuf) -> Result<Self> {
        updater::spawn_check(proxy.clone(), root.clone());
        capture::spawn_screenshot_watch(proxy.clone());
        let data_dir = root.clone();
        let p = load_state(&root);

        let mut ui_ctx = WebContext::new(Some(root.join("ui")));
        let sidebar = create_sidebar(&window, &proxy, &mut ui_ctx)?;

        let tabs = p.saved.iter().map(Tab::from_saved).collect();

        // Only ticks while DevTools is open (see `relayout`); parked (zero wakeups) otherwise.
        let devtools_polling = Arc::new(AtomicBool::new(false));
        let poll_thread = spawn_devtools_poll(proxy.clone(), Arc::clone(&devtools_polling));

        let history = load_history(&root.join("history.json"));
        let downloads = load_downloads(&root.join("downloads.json"));
        let initial_active = if p.restore_tabs {
            p.active_tabs.clone()
        } else {
            HashMap::new()
        };

        let mut b = Self {
            window,
            proxy,
            root,
            sidebar,
            ui_ctx,
            contexts: HashMap::new(),
            ext_done: HashSet::new(),
            p,
            tabs,
            active: initial_active,
            visible: None,
            overlay: false,
            downloads_hole: None,
            fullscreen: false,
            closed_tabs: Vec::new(),
            undo_stack: Vec::new(),
            undo_rec: None,
            devtools_open: false,
            devtools: None,
            devtools_tab: None,
            devtools_icons: default_devtools_icons(),
            devtools_icons_version: 1,
            devtools_icons_applied: false,
            devtools_icon_misses: HashSet::new(),
            responsive_mode: false,
            responsive_bounds: None,
            devtools_polling,
            poll_thread,
            poll_busy: Arc::new(AtomicU64::new(0)),
            chrome_shown: Cell::new(false),
            last_state_hash: Cell::new(0),
            sugg_cache: RefCell::new(None),
            save_dirty: false,
            last_save: Instant::now() - SAVE_MIN_INTERVAL,
            crashes: HashMap::new(),
            history,
            history_dirty: false,
            last_history_save: Instant::now(),
            history_query: String::new(),
            logged_urls: HashMap::new(),
            bg_tabs: HashSet::new(),
            fs_css: Cell::new(false),
            was_maximized: false,
            mru: Vec::new(),
            switcher: None,
            split: None,
            peek: false,
            scrollbar_visible: true,
            split_chrome_shown: Cell::new(false),
            tab_drag: false,
            saved_splits: Vec::new(),
            vid_tabs: HashSet::new(),
            link_peek: None,
            media: HashMap::new(),
            popups: HashMap::new(),
            pip_tabs: HashSet::new(),
            live_tabs: HashSet::new(),
            sleep_hold: HashSet::new(),
            sleep_snaps: HashMap::new(),
            low_mem: HashSet::new(),
            last_push: Cell::new(Instant::now() - STATE_MIN_INTERVAL),
            switching_space: Cell::new(false),
            state_dirty: Cell::new(false),
            pending_update: None,
            awaiting_backdrop: false,
            offered_update: None,
            update_now: false,
            passwords: PasswordStore::open(&data_dir),
            password_offer: None,
            importing: false,
            live_fetching: HashMap::new(),
            downloads,
            lingering: Vec::new(),
            sidebar_ready: false,
            startup_urls: Vec::new(),
        };
        b.apply_theme();
        b.preinstall();
        b.prune_history();
        shortcuts::apply_keymap(&b.p.shortcuts);
        Ok(b)
    }
}
