use super::{model::Site, *};

#[test]
fn devtools_preferences_default_to_a_right_docked_panel() {
    let state = Persisted::default();
    assert_eq!(state.devtools_position, "right");
    assert_eq!(state.devtools_size, 420.0);
}

#[test]
fn invalid_devtools_preferences_are_sanitized() {
    let mut state = Persisted {
        devtools_position: "floating".into(),
        devtools_size: f64::NAN,
        ..Persisted::default()
    };
    state.sanitize_devtools_preferences();
    assert_eq!(state.devtools_position, "right");
    assert_eq!(state.devtools_size, 420.0);

    state.devtools_size = 2_000.0;
    state.sanitize_devtools_preferences();
    assert_eq!(state.devtools_size, 800.0);
}

#[test]
fn restore_tabs_is_off_by_default() {
    let state = Persisted::default();
    assert!(!state.restore_tabs);
    assert!(state.active_tabs.is_empty());
}

#[test]
fn background_tabs_sleep_by_default() {
    assert_eq!(Persisted::default().tab_sleep_minutes, 10);
}

#[test]
fn prune_sites_keeps_the_most_visited() {
    let mut state = Persisted::default();
    for i in 0..450u32 {
        state.sites.insert(
            format!("h{i}.test"),
            Site {
                url: String::new(),
                favicon: None,
                count: i,
            },
        );
    }
    state.prune_sites();
    assert_eq!(state.sites.len(), 400);
    assert!(state.sites.contains_key("h449.test"));
    assert!(!state.sites.contains_key("h0.test"));
}

#[test]
fn folders_saved_before_live_folders_load_as_ordinary_folders() {
    let folder: TabFolder =
        serde_json::from_str(r#"{"id":"f","space_id":"s","name":"Docs","collapsed":true}"#)
            .unwrap();
    assert!(folder.live.is_none());
}

#[test]
fn a_live_folder_keeps_its_source_and_links_across_a_save() {
    let mut live = LiveFolder::new(
        LiveSource::Feed {
            url: "https://example.com/feed.xml".into(),
        },
        30,
    );
    live.items.push(LiveItem {
        title: "Hello".into(),
        url: "https://example.com/hello".into(),
    });
    live.checked_at = 1_700_000_000;
    let folder = TabFolder {
        id: "f".into(),
        space_id: "s".into(),
        name: "News".into(),
        collapsed: false,
        pinned: false,
        order: 0,
        live: Some(live),
    };
    let json = serde_json::to_string(&folder).unwrap();
    let back: TabFolder = serde_json::from_str(&json).unwrap();
    let live = back.live.unwrap();
    assert_eq!(live.items.len(), 1);
    assert_eq!(live.checked_at, 1_700_000_000);
    assert!(json.contains(r#""kind":"feed""#));
}
