//! Messages the browser receives: commands from the sidebar and reports from pages.

use super::*;

/// What the DevTools frontend reports back to the native side.
#[derive(Deserialize, Default)]
pub(super) struct DevToolsReport {
    #[serde(default)]
    pub(super) ready: bool,
    #[serde(default)]
    pub(super) responsive: bool,
    /// [x, y, w, h] of the emulated screen, in the frontend's own coordinates.
    #[serde(default)]
    pub(super) bounds: Option<[f64; 4]>,
    #[serde(default)]
    pub(super) iw: f64,
    #[serde(default)]
    pub(super) ih: f64,
    /// Version of the icon set the frontend has applied (0 = none).
    #[serde(default)]
    pub(super) icons_key: u64,
    /// DevTools icon names that have no equivalent in the browser's icon set.
    #[serde(default)]
    pub(super) icon_misses: Vec<String>,
}

/// A space the user asked to create, optionally together with a brand-new profile.
#[derive(Deserialize)]
pub(super) struct NewSpace {
    pub(super) name: String,
    pub(super) icon: String,
    pub(super) color: String,
    #[serde(default)]
    pub(super) profile_id: Option<String>,
    #[serde(default)]
    pub(super) new_profile: Option<String>,
}

/// How long a profile keeps tabs, cookies, cache and history (0 = keep / off, see the UI).
#[derive(Deserialize)]
pub(super) struct CleanupHours {
    pub(super) tab_retention_hours: u64,
    pub(super) cookie_retention_hours: u64,
    pub(super) cache_retention_hours: u64,
    pub(super) history_retention_hours: u64,
}

#[derive(Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub(super) enum Cmd {
    Ready,
    SidebarCollapsed {
        collapsed: bool,
    },
    SidebarCompact {
        compact: bool,
    },
    SidebarWidth {
        width: f64,
        #[serde(default)]
        persist: bool,
    },
    WindowMinimize,
    WindowToggleMaximize,
    WindowDrag,
    WindowClose,
    /// The sidebar is showing a full-area view (new tab bar, dialogs): hide/show the page.
    Overlay {
        open: bool,
    },
    NewTab {
        input: String,
    },
    Navigate {
        input: String,
    },
    SelectTab {
        id: String,
    },
    CloseTab {
        id: String,
    },
    UnloadTab {
        id: String,
    },
    ClearTabs,
    PinTab {
        id: String,
    },
    UnpinTab {
        id: String,
    },
    FavTab {
        id: String,
    },
    UnfavTab {
        id: String,
    },
    SwitchSpace {
        id: String,
    },
    CycleSpace {
        dir: i32,
    },
    AddSpace(NewSpace),
    EditSpace {
        id: String,
        name: String,
        icon: String,
        color: String,
    },
    /// Drag & drop: put a tab into the pinned list, the tab list or the favorites at `index`.
    MoveTab {
        id: String,
        to: String,
        index: usize,
    },
    CreateFolder {
        name: String,
    },
    RenameFolder {
        id: String,
        name: String,
    },
    DeleteFolder {
        id: String,
    },
    ToggleFolderCollapsed {
        id: String,
    },
    MoveFolder {
        id: String,
        to: String,
        index: usize,
    },
    MoveTabToFolder {
        id: String,
        folder_id: Option<String>,
        /// Position among the folder's tabs (None = end).
        #[serde(default)]
        index: Option<usize>,
    },
    /// The user answered the update prompt: install now, or later (when the browser closes).
    UpdateAccept,
    UpdateLater,
    ListHistory {
        #[serde(default)]
        query: String,
    },
    DeleteHistory {
        id: String,
    },
    ClearHistory,
    SetTheme {
        theme: String,
    },
    SetRestoreTabs {
        on: bool,
    },
    SetAutoPip {
        on: bool,
    },
    UpdateProfileCleanup(CleanupHours),
    DeleteSpace {
        id: String,
    },
    ListExt,
    InstallExt {
        input: String,
    },
    RemoveExt {
        dir: String,
    },
    Back,
    Forward,
    Reload,
    SwitcherPoll,
    SwitcherCommit,
    SwitcherCancel,
    SwitcherPick {
        id: String,
    },
    SidebarPeek {
        open: bool,
    },
    SplitTab {
        id: String,
        /// The tab that was dropped on; it becomes the other pane. Defaults to the visible tab.
        #[serde(default)]
        target: Option<String>,
        #[serde(default)]
        stacked: bool,
        /// The dragged tab becomes the first (left/top) pane.
        #[serde(default)]
        first: bool,
    },
    SplitClose {
        #[serde(default)]
        id: Option<String>,
    },
    SplitOrientation,
    SplitRatio {
        ratio: f64,
    },
    ToggleScrollbar,
    /// F pressed while the sidebar has focus: fullscreen (or leave) the active page's player.
    FullscreenToggle,
    /// Escape pressed while the sidebar has focus during page fullscreen.
    FullscreenLeave,
    /// The user answered the "save password?" offer.
    PasswordOfferAccept,
    PasswordOfferDismiss {
        #[serde(default)]
        never: bool,
    },
    /// The Downloads view opened: send the list.
    ListDownloads,
    PauseDownload {
        id: String,
    },
    ResumeDownload {
        id: String,
    },
    /// Cancels a running download.
    CancelDownload {
        id: String,
    },
    OpenDownload {
        id: String,
    },
    /// Show the downloaded file in the system file manager.
    ShowDownload {
        id: String,
    },
    /// Remove one finished download from the list (the file stays).
    RemoveDownload {
        id: String,
    },
    ClearDownloads,
    /// The settings view asks whether poof is the default browser.
    DefaultBrowserStatus,
    SetDefaultBrowser,
    ListPasswords,
    RevealPassword {
        id: String,
    },
    DeletePassword {
        id: String,
    },
    SetSavePasswords {
        on: bool,
    },
    TabDrag {
        active: bool,
    },
    #[serde(rename = "devtools_toggle", alias = "dev_tools_toggle")]
    DevToolsToggle,
    #[serde(rename = "devtools_position", alias = "dev_tools_position")]
    DevToolsPosition {
        position: String,
    },
    #[serde(rename = "devtools_size", alias = "dev_tools_size")]
    DevToolsSize {
        size: f64,
        #[serde(default)]
        persist: bool,
    },
    /// The browser's own icon set as SVG data URIs, for the DevTools frontend.
    #[serde(rename = "devtools_icons", alias = "dev_tools_icons")]
    DevToolsIcons {
        icons: HashMap<String, String>,
    },
}

#[derive(Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub(super) enum PageMsg {
    Meta {
        u: Option<String>,
    },
    Favicon {
        f: Option<String>,
    },
    Key {
        k: String,
    },
    Fullscreen {
        active: bool,
    },
    /// The page has a visible password field.
    PwForm,
    /// The user submitted a login. UNTRUSTED values; the site comes from the tab, not the page.
    PwSubmit {
        u: String,
        p: String,
    },
    /// The page's WebView took keyboard focus (used to track the active split pane).
    Focus,
    /// A video was started by the user on this page, so auto-PIP may apply when leaving it.
    Vid,
    /// The page started / stopped showing a Picture-in-Picture window.
    Pip {
        active: bool,
    },
    /// The page started / stopped live capture or a call (see `js/live.js`).
    Live {
        on: bool,
    },
    /// Internal (empty tab id only): open a link in a background tab (Ctrl/middle click).
    #[serde(rename = "open_background")]
    OpenBackground {
        u: String,
    },
    /// Internal (empty tab id only): re-run the layout once the window has settled.
    #[serde(rename = "relayout")]
    Relayout,
    /// Internal (empty tab id only): periodic DevTools poll trigger.
    #[serde(rename = "devtools_tick")]
    DevToolsTick,
    /// Internal (empty tab id only): result of a DevTools poll.
    #[serde(rename = "devtools_state")]
    DevToolsState {
        s: Option<DevToolsReport>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouped_commands_keep_their_flat_json_shape() {
        let add: Cmd = serde_json::from_str(
            r##"{"cmd":"add_space","name":"Work","icon":"house","color":"#d64933","new_profile":"Work"}"##,
        )
        .unwrap();
        match add {
            Cmd::AddSpace(s) => {
                assert_eq!(s.name, "Work");
                assert_eq!(s.new_profile.as_deref(), Some("Work"));
                assert!(s.profile_id.is_none());
            }
            _ => panic!("expected AddSpace"),
        }
        let clean: Cmd = serde_json::from_str(
            r#"{"cmd":"update_profile_cleanup","tab_retention_hours":48,"cookie_retention_hours":0,"cache_retention_hours":24,"history_retention_hours":72}"#,
        )
        .unwrap();
        match clean {
            Cmd::UpdateProfileCleanup(c) => assert_eq!(c.history_retention_hours, 72),
            _ => panic!("expected UpdateProfileCleanup"),
        }
    }

    #[test]
    fn devtools_commands_accept_both_spellings() {
        assert!(serde_json::from_str::<Cmd>(r#"{"cmd":"devtools_toggle"}"#).is_ok());
        assert!(serde_json::from_str::<Cmd>(r#"{"cmd":"dev_tools_toggle"}"#).is_ok());
    }

    #[test]
    fn download_and_default_browser_commands_parse() {
        let pause: Cmd = serde_json::from_str(r#"{"cmd":"pause_download","id":"d1"}"#).unwrap();
        assert!(matches!(pause, Cmd::PauseDownload { id } if id == "d1"));
        assert!(matches!(
            serde_json::from_str::<Cmd>(r#"{"cmd":"clear_downloads"}"#),
            Ok(Cmd::ClearDownloads)
        ));
        assert!(matches!(
            serde_json::from_str::<Cmd>(r#"{"cmd":"set_default_browser"}"#),
            Ok(Cmd::SetDefaultBrowser)
        ));
    }

    #[test]
    fn page_messages_parse() {
        assert!(matches!(
            serde_json::from_str::<PageMsg>(r#"{"t":"pip","active":true}"#),
            Ok(PageMsg::Pip { active: true })
        ));
        assert!(matches!(
            serde_json::from_str::<PageMsg>(r#"{"t":"vid"}"#),
            Ok(PageMsg::Vid)
        ));
    }
}
