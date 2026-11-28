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
    Ready {
        /// Local time of day in minutes since midnight (the sidebar knows the time zone).
        #[serde(default)]
        local_min: Option<u16>,
    },
    /// Replaces the space schedule (validated natively).
    SetSchedules {
        rules: Vec<SpaceSchedule>,
    },
    /// "Continue" card: open that space and wake only the tabs it was last used with.
    ResumeSpace {
        id: String,
    },
    /// Hide one "Continue" card, or all of them.
    ResumeDismiss {
        id: Option<String>,
    },
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
        /// Show a picture of the current page behind the view instead of an empty stage.
        #[serde(default)]
        snapshot: bool,
    },
    /// The sidebar painted the page picture: the real page may be hidden now.
    BackdropReady,
    /// The Downloads popover opened, moved or closed. `x`, `y`, `w`, `h` are its box in window
    /// coordinates; the page gets a hole there so the popover shows above the live page.
    DownloadsPanel {
        open: bool,
        #[serde(default)]
        x: f64,
        #[serde(default)]
        y: f64,
        #[serde(default)]
        w: f64,
        #[serde(default)]
        h: f64,
    },
    /// Bind `chord` to the action `id` (`""` removes it); a taken chord moves here.
    SetShortcut {
        id: String,
        chord: String,
    },
    /// Restore one action's default chord.
    ResetShortcut {
        id: String,
    },
    ResetShortcuts,
    /// The settings page started / stopped recording a chord.
    ShortcutCapture {
        active: bool,
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
    /// Restore the latest closed tab, deleted folder or space, or removed favorite or pin.
    Undo,
    /// Bring back the empty-list tips for favorites and pinned tabs.
    ResetHints,
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
        /// A touchpad swipe keeps the sidebar focused, so the next swipe still reaches it
        /// (wheel events go to the focused WebView, not the one under the pointer).
        #[serde(default)]
        keep_focus: bool,
    },
    /// The sidebar asks for every space's lists so a swipe can show the neighbours.
    SpaceSnapshots,
    /// Gives the keyboard focus back to a page after the sidebar borrowed it (clipboard copy).
    FocusTab {
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
    SetSearchEngine {
        id: String,
    },
    UpdateProfileCleanup(CleanupHours),
    DeleteSpace {
        id: String,
    },
    /// Adds a link-routing rule, or replaces the one with `id`.
    SaveLinkRule {
        #[serde(default)]
        id: Option<String>,
        matches: RuleMatch,
        pattern: String,
        target: RuleTarget,
    },
    DeleteLinkRule {
        id: String,
    },
    /// `delta` is -1 (earlier) or 1 (later).
    MoveLinkRule {
        id: String,
        delta: i32,
    },
    SetLinkFallback {
        #[serde(default)]
        target: Option<RuleTarget>,
    },
    /// Adds or replaces the boost of `domain`.
    SaveBoost {
        domain: String,
        #[serde(default)]
        enabled: bool,
        #[serde(default)]
        css: String,
        #[serde(default)]
        js: String,
        #[serde(default)]
        zap: Vec<String>,
    },
    DeleteBoost {
        domain: String,
    },
    /// Starts the "click an element to hide it" picker on the visible page.
    StartZap,
    SetMiniWindow {
        on: bool,
    },
    SetPeekLinks {
        on: bool,
    },
    /// The quiet drop tick on or off.
    SetFeedback {
        sound: bool,
    },
    /// Opens the small quick-search window.
    OpenMini,
    /// Play or pause the media of a tab (the mini player).
    MediaToggle {
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
    /// Capture button of the toolbar: area, element or full page of the visible page.
    Capture,
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
    /// Starts a native drag of a finished download (the pointer is still down).
    DragDownload {
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
    /// A folder whose links come from a feed or a GitHub search.
    CreateLiveFolder {
        name: String,
        source: LiveSource,
        interval_minutes: u32,
    },
    EditLiveFolder {
        id: String,
        name: String,
        source: LiveSource,
        interval_minutes: u32,
    },
    RefreshLiveFolder {
        id: String,
    },
    /// Settings > Import: bring spaces / passwords over from Arc.
    ImportArc {
        spaces: bool,
        passwords: bool,
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
    /// Boost picker result. UNTRUSTED: only a selector; the site comes from the tab.
    BoostZap {
        s: String,
    },
    /// The page loaded and wants the JavaScript of its site's boosts run.
    BoostRun,
    /// The page's current media element started, paused or ended. UNTRUSTED: text is clipped.
    Media {
        on: bool,
        #[serde(default)]
        playing: bool,
        #[serde(default)]
        title: String,
        #[serde(default)]
        artist: String,
    },
    /// Alt+click on a link while Peek is on. UNTRUSTED: checked like any address.
    Peek {
        u: String,
    },
    /// A click on the dimmed page behind the peek overlay.
    PeekClose,
    /// The page started / stopped live capture or a call (see `js/live.js`).
    Live {
        on: bool,
    },
    /// Sleep guard report (see `js/sleep-guard.js`). UNTRUSTED: sizes are clipped.
    Snap {
        hold: bool,
        #[serde(default)]
        u: String,
        #[serde(default)]
        y: f64,
        #[serde(default)]
        f: Value,
    },
    /// Internal (empty tab id only): open a link in a background tab (Ctrl/middle click).
    #[serde(rename = "open_background")]
    OpenBackground {
        u: String,
    },
    /// Internal (empty tab id only): native code listed a right-click menu for `tab`; show it.
    #[serde(rename = "ctx_open")]
    CtxOpen {
        tab: String,
        items: Value,
        x: f64,
        y: f64,
    },
    /// The right-click menu's choice (`id` 0 = dismissed). UNTRUSTED: only counts with the menu's
    /// one-time token `n`, and only from the tab it belongs to.
    #[serde(rename = "ctx_pick")]
    CtxPick {
        n: String,
        id: i32,
    },
    /// Internal (empty tab id only): re-run the layout once the window has settled.
    #[serde(rename = "relayout")]
    Relayout,
    /// The capture overlay's choice. UNTRUSTED: only counts with the capture's one-time token `n`.
    /// A width of 0 means it was cancelled.
    Capture {
        n: String,
        #[serde(default)]
        x: f64,
        #[serde(default)]
        y: f64,
        #[serde(default)]
        w: f64,
        #[serde(default)]
        h: f64,
        #[serde(default)]
        d: f64,
        /// Local time for the file name, e.g. "2026-11-27 at 14.03.22".
        #[serde(default)]
        s: String,
    },
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
        let rule: Cmd = serde_json::from_str(
            r#"{"cmd":"save_link_rule","matches":"domain","pattern":"github.com","target":{"kind":"profile","id":"p1"}}"#,
        )
        .unwrap();
        assert!(matches!(
            rule,
            Cmd::SaveLinkRule { id: None, matches: RuleMatch::Domain, target: RuleTarget::Profile(p), .. } if p == "p1"
        ));
        let fallback: Cmd = serde_json::from_str(r#"{"cmd":"set_link_fallback"}"#).unwrap();
        assert!(matches!(fallback, Cmd::SetLinkFallback { target: None }));
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
    fn import_command_carries_both_choices() {
        let cmd: Cmd =
            serde_json::from_str(r#"{"cmd":"import_arc","spaces":true,"passwords":false}"#)
                .unwrap();
        assert!(matches!(
            cmd,
            Cmd::ImportArc {
                spaces: true,
                passwords: false
            }
        ));
    }

    #[test]
    fn live_folder_commands_nest_the_source() {
        let create: Cmd = serde_json::from_str(
            r#"{"cmd":"create_live_folder","name":"News","source":{"kind":"feed","url":"https://example.com/f"},"interval_minutes":30}"#,
        )
        .unwrap();
        assert!(matches!(
            create,
            Cmd::CreateLiveFolder {
                source: LiveSource::Feed { .. },
                interval_minutes: 30,
                ..
            }
        ));
        let edit: Cmd = serde_json::from_str(
            r#"{"cmd":"edit_live_folder","id":"f","name":"","source":{"kind":"github","query":"is:pr"},"interval_minutes":60}"#,
        )
        .unwrap();
        assert!(matches!(
            edit,
            Cmd::EditLiveFolder {
                source: LiveSource::Github { .. },
                ..
            }
        ));
        assert!(serde_json::from_str::<Cmd>(r#"{"cmd":"refresh_live_folder","id":"f"}"#).is_ok());
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
        let panel: Cmd = serde_json::from_str(
            r#"{"cmd":"downloads_panel","open":true,"x":10.5,"y":44,"w":400,"h":300}"#,
        )
        .unwrap();
        assert!(matches!(panel, Cmd::DownloadsPanel { open: true, w, .. } if w == 400.0));
        assert!(matches!(
            serde_json::from_str::<Cmd>(r#"{"cmd":"downloads_panel","open":false}"#),
            Ok(Cmd::DownloadsPanel { open: false, .. })
        ));
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
