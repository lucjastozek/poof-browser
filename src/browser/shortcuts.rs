//! Keyboard shortcuts that must work while a page owns keyboard focus, and the user-editable
//! keymap behind them.
//!
//! A shortcut is a *chord* such as `Ctrl+Shift+T`. Every action has an id, a default chord and
//! an optional override stored in `state.json` (`""` means the user removed it). The merged
//! chord to command table lives in a global so WebView2's accelerator callback can read it
//! without touching `Browser`.

use super::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

/// One bindable action. `label` is empty for hidden aliases that are not user-editable.
struct Action {
    id: &'static str,
    command: &'static str,
    label: &'static str,
    group: &'static str,
    chord: &'static str,
}

const fn action(
    id: &'static str,
    command: &'static str,
    label: &'static str,
    group: &'static str,
    chord: &'static str,
) -> Action {
    Action {
        id,
        command,
        label,
        group,
        chord,
    }
}

const TABS: &str = "Tabs";
const NAV: &str = "Navigation";
const VIEWS: &str = "Pages & panels";
const LAYOUT: &str = "Spaces & layout";

/// Defaults, in the order they are listed in settings. Hidden aliases come last.
const ACTIONS: &[Action] = &[
    action("new_tab", "new_tab", "New tab", TABS, "Ctrl+T"),
    action("close", "close", "Close tab", TABS, "Ctrl+W"),
    action(
        "reopen",
        "reopen",
        "Reopen closed tab",
        TABS,
        "Ctrl+Shift+T",
    ),
    action("tab_next", "tab_next", "Next tab", TABS, "Ctrl+Tab"),
    action(
        "tab_prev",
        "tab_prev",
        "Previous tab",
        TABS,
        "Ctrl+Shift+Tab",
    ),
    action("tab_1", "tab_1", "Go to tab 1", TABS, "Ctrl+1"),
    action("tab_2", "tab_2", "Go to tab 2", TABS, "Ctrl+2"),
    action("tab_3", "tab_3", "Go to tab 3", TABS, "Ctrl+3"),
    action("tab_4", "tab_4", "Go to tab 4", TABS, "Ctrl+4"),
    action("tab_5", "tab_5", "Go to tab 5", TABS, "Ctrl+5"),
    action("tab_6", "tab_6", "Go to tab 6", TABS, "Ctrl+6"),
    action("tab_7", "tab_7", "Go to tab 7", TABS, "Ctrl+7"),
    action("tab_8", "tab_8", "Go to tab 8", TABS, "Ctrl+8"),
    action("tab_last", "tab_last", "Go to last tab", TABS, "Ctrl+9"),
    action("favorite", "favorite", "Add to favorites", TABS, "Ctrl+D"),
    action("focus_url", "focus_url", "Address bar", NAV, "Ctrl+L"),
    action("back", "back", "Back", NAV, "Alt+Left"),
    action("forward", "forward", "Forward", NAV, "Alt+Right"),
    action("reload", "reload", "Reload", NAV, "Ctrl+R"),
    action("print", "print", "Print", NAV, "Ctrl+P"),
    action(
        "copy_clean_link",
        "copy_clean_link",
        "Copy clean link",
        NAV,
        "Ctrl+Shift+C",
    ),
    action("history", "history", "History", VIEWS, "Ctrl+H"),
    action("downloads", "downloads", "Downloads", VIEWS, "Ctrl+J"),
    action(
        "devtools_toggle",
        "devtools_toggle",
        "Developer tools",
        VIEWS,
        "F12",
    ),
    action(
        "toggle_scrollbar",
        "toggle_scrollbar",
        "Toggle scrollbar",
        VIEWS,
        "Ctrl+S",
    ),
    action(
        "space_prev",
        "space_prev",
        "Previous space",
        LAYOUT,
        "Ctrl+Alt+Left",
    ),
    action(
        "space_next",
        "space_next",
        "Next space",
        LAYOUT,
        "Ctrl+Alt+Right",
    ),
    action(
        "split_toggle",
        "split_toggle",
        "Split view",
        LAYOUT,
        "Ctrl+Shift+E",
    ),
    action(
        "split_orient",
        "split_orient",
        "Split orientation",
        LAYOUT,
        "Ctrl+Shift+O",
    ),
    // Chrome-compatible extra chords; used only while nothing else owns them.
    action("reload_alt", "reload", "", "", "Ctrl+Shift+R"),
    action("devtools_alt", "devtools_toggle", "", "", "Ctrl+Shift+I"),
];

/// Chords a page or the system needs: clipboard and undo keys, closing a window.
const RESERVED: &[&str] = &[
    "Ctrl+C", "Ctrl+V", "Ctrl+X", "Ctrl+A", "Ctrl+Z", "Ctrl+Y", "Alt+F4",
];

type Keymap = HashMap<String, &'static str>;

static KEYMAP: Mutex<Option<Keymap>> = Mutex::new(None);
/// True while the settings page records a new chord: the browser must not act on keys then.
static CAPTURING: AtomicBool = AtomicBool::new(false);

/// Name of a virtual key as used in chords, or `None` for keys that cannot be bound.
fn key_name(vk: u32) -> Option<String> {
    let name = match vk {
        0x41..=0x5A | 0x30..=0x39 => char::from_u32(vk)?.to_string(),
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        0x09 => "Tab".into(),
        0x0D => "Enter".into(),
        0x20 => "Space".into(),
        0x08 => "Backspace".into(),
        0x2E => "Delete".into(),
        0x2D => "Insert".into(),
        0x24 => "Home".into(),
        0x23 => "End".into(),
        0x21 => "PageUp".into(),
        0x22 => "PageDown".into(),
        0x25 => "Left".into(),
        0x26 => "Up".into(),
        0x27 => "Right".into(),
        0x28 => "Down".into(),
        0xBA => ";".into(),
        0xBB => "=".into(),
        0xBC => ",".into(),
        0xBD => "-".into(),
        0xBE => ".".into(),
        0xBF => "/".into(),
        0xC0 => "`".into(),
        0xDB => "[".into(),
        0xDC => "\\".into(),
        0xDD => "]".into(),
        0xDE => "'".into(),
        _ => return None,
    };
    Some(name)
}

fn is_function_key(key: &str) -> bool {
    key.strip_prefix('F')
        .is_some_and(|n| n.parse::<u8>().is_ok())
}

fn format_chord(key: &str, ctrl: bool, alt: bool, shift: bool) -> String {
    let mut chord = String::new();
    for (on, name) in [(ctrl, "Ctrl"), (alt, "Alt"), (shift, "Shift")] {
        if on {
            chord.push_str(name);
            chord.push('+');
        }
    }
    chord.push_str(key);
    chord
}

/// Normalizes `chord` (any modifier order, any case) and checks the user may bind it: a known
/// key, plus Ctrl or Alt unless it is a function key, and not a reserved chord.
fn normalize_chord(chord: &str) -> Option<String> {
    let (mut ctrl, mut alt, mut shift) = (false, false, false);
    let mut key = None;
    for part in chord.split('+').filter(|p| !p.is_empty()) {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "alt" => alt = true,
            "shift" => shift = true,
            _ if key.is_none() => key = Some(part.to_string()),
            _ => return None,
        }
    }
    let mut key = key?;
    if key.len() == 1 {
        key = key.to_ascii_uppercase();
    } else if let Some(canonical) = (0u32..=0xFF)
        .filter_map(key_name)
        .find(|k| k.eq_ignore_ascii_case(&key))
    {
        key = canonical;
    }
    if !(0u32..=0xFF).filter_map(key_name).any(|k| k == key) {
        return None;
    }
    if !(ctrl || alt || is_function_key(&key)) {
        return None;
    }
    let chord = format_chord(&key, ctrl, alt, shift);
    (!RESERVED.contains(&chord.as_str())).then_some(chord)
}

/// The chord an action currently has: the user's override, else its default. Empty = removed.
fn effective<'a>(action: &'a Action, overrides: &'a HashMap<String, String>) -> &'a str {
    overrides
        .get(action.id)
        .map(String::as_str)
        .unwrap_or(action.chord)
}

fn build_keymap(overrides: &HashMap<String, String>) -> Keymap {
    let mut map = Keymap::new();
    for action in ACTIONS.iter().filter(|a| !a.label.is_empty()) {
        let chord = effective(action, overrides);
        if !chord.is_empty() {
            map.insert(chord.to_string(), action.command);
        }
    }
    for action in ACTIONS.iter().filter(|a| a.label.is_empty()) {
        map.entry(action.chord.to_string())
            .or_insert(action.command);
    }
    map
}

/// Makes `overrides` the active keymap. Called at startup and after every change.
pub(super) fn apply_keymap(overrides: &HashMap<String, String>) {
    if let Ok(mut keymap) = KEYMAP.lock() {
        *keymap = Some(build_keymap(overrides));
    }
}

/// While true, `browser_shortcut` ignores every key (the settings page is recording one).
pub(super) fn set_capturing(active: bool) {
    CAPTURING.store(active, Ordering::Relaxed);
}

/// The editable actions with their current and default chords, for the settings page.
pub(super) fn describe(overrides: &HashMap<String, String>) -> Vec<Value> {
    ACTIONS
        .iter()
        .filter(|a| !a.label.is_empty())
        .map(|a| {
            json!({
                "id": a.id, "label": a.label, "group": a.group,
                "chord": effective(a, overrides), "default": a.chord,
            })
        })
        .collect()
}

/// What `set_shortcut` did, so the caller can tell the user.
pub(super) enum Assigned {
    Done,
    /// The chord was taken; it moved here from the named action.
    Moved(&'static str),
    Invalid,
}

/// Gives `id` the chord `requested` (`""` removes it, `None` restores its default). An action
/// that already had the chord loses it, so a chord never triggers two actions.
pub(super) fn assign(
    overrides: &mut HashMap<String, String>,
    id: &str,
    requested: Option<&str>,
) -> Assigned {
    let Some(action) = ACTIONS.iter().find(|a| a.id == id && !a.label.is_empty()) else {
        return Assigned::Invalid;
    };
    let chord = match requested {
        None => action.chord.to_string(),
        Some("") => String::new(),
        Some(text) => match normalize_chord(text) {
            Some(chord) => chord,
            None => return Assigned::Invalid,
        },
    };
    let mut moved = None;
    if !chord.is_empty() {
        for other in ACTIONS.iter().filter(|a| !a.label.is_empty() && a.id != id) {
            if effective(other, overrides) == chord {
                overrides.insert(other.id.to_string(), String::new());
                moved = Some(other.label);
            }
        }
    }
    if chord == action.chord {
        overrides.remove(id);
    } else {
        overrides.insert(id.to_string(), chord);
    }
    moved.map_or(Assigned::Done, Assigned::Moved)
}

/// Maps WebView2 accelerator keys (Win32 VK_* values) to browser commands.
pub(super) fn browser_shortcut(
    key: u32,
    ctrl: bool,
    alt: bool,
    shift: bool,
) -> Option<&'static str> {
    if CAPTURING.load(Ordering::Relaxed) {
        return None;
    }
    let chord = format_chord(&key_name(key)?, ctrl, alt, shift);
    let mut keymap = KEYMAP.lock().ok()?;
    keymap
        .get_or_insert_with(|| build_keymap(&HashMap::new()))
        .get(&chord)
        .copied()
}

#[cfg(windows)]
pub(super) fn register_browser_shortcuts(
    webview: &WebView,
    proxy: EventLoopProxy<UserEvent>,
    tab: String,
) {
    use webview2_com::{AcceleratorKeyPressedEventHandler, Microsoft::Web::WebView2::Win32::*};
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;

    unsafe {
        let handler = AcceleratorKeyPressedEventHandler::create(Box::new(move |_sender, args| {
            let Some(args) = args else { return Ok(()) };
            let mut kind = COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN;
            args.KeyEventKind(&mut kind)?;
            if kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN
                && kind != COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN
            {
                return Ok(());
            }

            let mut key_event_lparam = 0;
            args.KeyEventLParam(&mut key_event_lparam)?;
            let mut virtual_key = 0;
            args.VirtualKey(&mut virtual_key)?;
            let is_down = |virtual_key| GetAsyncKeyState(virtual_key) < 0;
            let command = browser_shortcut(
                virtual_key,
                is_down(0x11), // VK_CONTROL
                is_down(0x12), // VK_MENU
                is_down(0x10), // VK_SHIFT
            );

            if let Some(command) = command {
                args.SetHandled(true)?;
                // Ignore auto-repeat after suppressing WebView2's default action.
                if key_event_lparam & (1 << 30) != 0 {
                    return Ok(());
                }
                let _ = proxy.send_event(UserEvent::Page {
                    tab: tab.clone(),
                    body: json!({ "t": "key", "k": command }).to_string(),
                });
            }
            Ok(())
        }));
        let mut token = 0;
        if let Err(error) = webview
            .controller()
            .add_AcceleratorKeyPressed(&handler, &mut token)
        {
            log::warn!("registering WebView2 keyboard shortcuts failed: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none() -> HashMap<String, String> {
        HashMap::new()
    }

    #[test]
    fn defaults_map_chords_to_commands() {
        let map = build_keymap(&none());
        assert_eq!(map.get("Ctrl+J"), Some(&"downloads"));
        assert_eq!(map.get("Ctrl+Shift+R"), Some(&"reload"));
        assert_eq!(map.get("J"), None);
    }

    #[test]
    fn chords_are_normalized_and_validated() {
        assert_eq!(
            normalize_chord("shift+ctrl+t").as_deref(),
            Some("Ctrl+Shift+T")
        );
        assert_eq!(normalize_chord("F5").as_deref(), Some("F5"));
        assert_eq!(normalize_chord("T"), None); // plain letters would break typing
        assert_eq!(normalize_chord("Ctrl+C"), None); // reserved
        assert_eq!(normalize_chord("Ctrl+Nope"), None);
        assert_eq!(
            normalize_chord("Ctrl+Alt+Left").as_deref(),
            Some("Ctrl+Alt+Left")
        );
    }

    #[test]
    fn assigning_a_taken_chord_moves_it() {
        let mut overrides = none();
        let result = assign(&mut overrides, "history", Some("Ctrl+J"));
        assert!(matches!(result, Assigned::Moved("Downloads")));
        let map = build_keymap(&overrides);
        assert_eq!(map.get("Ctrl+J"), Some(&"history"));
        assert!(!map.values().any(|c| *c == "downloads"));
    }

    #[test]
    fn removing_and_restoring_a_shortcut() {
        let mut overrides = none();
        assign(&mut overrides, "print", Some(""));
        assert_eq!(build_keymap(&overrides).get("Ctrl+P"), None);
        assign(&mut overrides, "print", None);
        assert!(overrides.is_empty());
        assert_eq!(build_keymap(&overrides).get("Ctrl+P"), Some(&"print"));
    }

    #[test]
    fn a_user_chord_beats_a_hidden_alias() {
        let mut overrides = none();
        assign(&mut overrides, "print", Some("Ctrl+Shift+R"));
        assert_eq!(build_keymap(&overrides).get("Ctrl+Shift+R"), Some(&"print"));
    }

    #[test]
    fn ctrl_j_opens_downloads_and_plain_j_does_not() {
        apply_keymap(&none());
        assert_eq!(
            browser_shortcut(0x4A, true, false, false),
            Some("downloads")
        );
        assert_eq!(browser_shortcut(0x4A, false, false, false), None);
        assert_eq!(browser_shortcut(0x4A, true, false, true), None);
    }
}
