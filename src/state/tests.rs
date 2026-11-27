use super::link_rules::{
    clean_pattern, first_match, resolve_space, LinkRule, RuleMatch, RuleTarget,
};
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

fn rule(matches: RuleMatch, pattern: &str, target: RuleTarget) -> LinkRule {
    LinkRule {
        id: new_id(),
        matches,
        pattern: pattern.into(),
        target,
    }
}

fn space_of(profile: &Profile, name: &str) -> Space {
    Space {
        id: new_id(),
        profile_id: profile.id.clone(),
        name: name.into(),
        icon: "house".into(),
        color: "#6dc5e2".into(),
    }
}

#[test]
fn domain_patterns_reduce_to_a_bare_host() {
    let clean = |s: &str| clean_pattern(s, RuleMatch::Domain);
    assert_eq!(clean("GitHub.com"), Some("github.com".into()));
    assert_eq!(
        clean("https://www.github.com/a/b?x=1"),
        Some("github.com".into())
    );
    assert_eq!(clean("*.example.org"), Some("example.org".into()));
    assert_eq!(clean("localhost:3000"), Some("localhost".into()));
    assert_eq!(clean("user@host.dev/path"), Some("host.dev".into()));
    assert_eq!(clean("  "), None);
    assert_eq!(clean("two words.com"), None);
    assert_eq!(clean("-bad.com"), None);
    assert_eq!(clean("a\u{0}b.com"), None);
}

#[test]
fn contains_patterns_are_lowercased_and_kept_whole() {
    assert_eq!(
        clean_pattern(" /Pull/ ", RuleMatch::Contains),
        Some("/pull/".into())
    );
    assert_eq!(clean_pattern("", RuleMatch::Contains), None);
}

#[test]
fn a_domain_rule_covers_subdomains_but_not_lookalikes() {
    let r = rule(
        RuleMatch::Domain,
        "github.com",
        RuleTarget::Space("s".into()),
    );
    assert!(r.applies_to("https://github.com/x"));
    assert!(r.applies_to("https://www.github.com/x"));
    assert!(r.applies_to("http://gist.github.com/"));
    assert!(!r.applies_to("https://notgithub.com/"));
    assert!(!r.applies_to("https://github.com.evil.io/"));
    assert!(!r.applies_to("https://example.com/github.com"));
    assert!(!r.applies_to("file:///github.com"));
}

#[test]
fn a_contains_rule_looks_at_the_whole_address() {
    let r = rule(RuleMatch::Contains, "/pull/", RuleTarget::Space("s".into()));
    assert!(r.applies_to("https://github.com/a/b/PULL/3"));
    assert!(!r.applies_to("https://github.com/a/b"));
}

#[test]
fn the_first_rule_with_a_live_target_wins() {
    let profile = Profile::new("Work");
    let (a, b) = (space_of(&profile, "A"), space_of(&profile, "B"));
    let rules = vec![
        rule(RuleMatch::Domain, "x.com", RuleTarget::Space("gone".into())),
        rule(RuleMatch::Domain, "x.com", RuleTarget::Space(b.id.clone())),
        rule(RuleMatch::Domain, "x.com", RuleTarget::Space(a.id.clone())),
    ];
    let spaces = [a, b.clone()];
    let hit = first_match(&rules, "https://x.com/", &spaces, &[profile]);
    assert_eq!(hit, Some(&RuleTarget::Space(b.id)));
    assert_eq!(first_match(&rules, "https://y.com/", &spaces, &[]), None);
}

#[test]
fn a_profile_target_keeps_the_current_space_when_it_belongs_to_it() {
    let (work, home) = (Profile::new("Work"), Profile::new("Home"));
    let (w1, w2, h1) = (
        space_of(&work, "w1"),
        space_of(&work, "w2"),
        space_of(&home, "h1"),
    );
    let spaces = [w1.clone(), w2.clone(), h1.clone()];
    let target = RuleTarget::Profile(work.id.clone());
    assert_eq!(
        resolve_space(&target, &spaces, &w2.id).map(|s| &s.id),
        Some(&w2.id)
    );
    assert_eq!(
        resolve_space(&target, &spaces, &h1.id).map(|s| &s.id),
        Some(&w1.id)
    );
    let none = RuleTarget::Profile("gone".into());
    assert!(resolve_space(&none, &spaces, &h1.id).is_none());
}

#[test]
fn loading_drops_rules_whose_target_was_deleted_and_old_files_have_none() {
    let mut state = Persisted::default();
    let live = state.spaces[0].id.clone();
    state.link_rules = vec![
        rule(RuleMatch::Domain, "a.com", RuleTarget::Space(live.clone())),
        rule(RuleMatch::Domain, "b.com", RuleTarget::Space("gone".into())),
    ];
    state.link_fallback = Some(RuleTarget::Space("gone".into()));
    state.prune_link_rules();
    assert_eq!(state.link_rules.len(), 1);
    assert!(state.link_fallback.is_none());

    let mut json: serde_json::Value = serde_json::to_value(Persisted::default()).unwrap();
    json.as_object_mut().unwrap().remove("link_rules");
    json.as_object_mut().unwrap().remove("link_fallback");
    let old: Persisted = serde_json::from_value(json).unwrap();
    assert!(old.link_rules.is_empty() && old.link_fallback.is_none());
}

#[test]
fn rule_targets_serialize_flat_for_the_sidebar() {
    let json = serde_json::to_string(&RuleTarget::Space("s1".into())).unwrap();
    assert_eq!(json, r#"{"kind":"space","id":"s1"}"#);
}
