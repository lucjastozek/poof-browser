//! Air Traffic Control: sends links from other apps to the space a rule picks, and edits the rules.

use super::*;
use crate::state::{
    clean_pattern, first_match, resolve_space, target_exists, LinkRule, LINK_RULES_MAX,
};

impl Browser {
    /// The space a link from another app should open in, or `None` to keep the current one.
    /// Only web links are routed; a local file always opens where you are.
    fn space_for_link(&self, url: &str) -> Option<&Space> {
        let web = url
            .get(..4)
            .is_some_and(|head| head.eq_ignore_ascii_case("http"));
        if !web {
            return None;
        }
        let target = first_match(&self.p.link_rules, url, &self.p.spaces, &self.p.profiles)
            .or(self.p.link_fallback.as_ref())?;
        resolve_space(target, &self.p.spaces, &self.p.current_space)
    }

    /// Opens a link from outside in a new tab of the space the rules choose. Switching space
    /// here skips restoring that space's last tab: the new tab is about to take its place, and
    /// loading the old one first would cost a page load and a flash.
    pub(super) fn open_routed(&mut self, url: &str) -> Result<()> {
        let destination = self
            .space_for_link(url)
            .filter(|space| space.id != self.p.current_space)
            .map(|space| (space.id.clone(), space.name.clone()));
        if let Some((id, name)) = destination {
            self.hide_visible();
            self.p.current_space = id;
            self.new_tab(url)?;
            self.toast(&format!("Opened in {name}"));
            return Ok(());
        }
        self.new_tab(url)
    }

    /// Adds a rule, or replaces the one with `id`. Bad input is ignored with a message.
    pub(super) fn save_link_rule(
        &mut self,
        id: Option<String>,
        matches: RuleMatch,
        pattern: &str,
        target: RuleTarget,
    ) {
        let Some(pattern) = clean_pattern(pattern, matches) else {
            self.toast("That doesn’t look like a site or text to match.");
            return;
        };
        if !target_exists(&target, &self.p.spaces, &self.p.profiles) {
            return;
        }
        let existing = id.and_then(|id| self.p.link_rules.iter().position(|r| r.id == id));
        match existing {
            Some(index) => {
                let rule = &mut self.p.link_rules[index];
                (rule.matches, rule.pattern, rule.target) = (matches, pattern, target);
            }
            None if self.p.link_rules.len() >= LINK_RULES_MAX => {
                self.toast("That is the most rules poof keeps. Remove one first.");
                return;
            }
            None => self.p.link_rules.push(LinkRule {
                id: new_id(),
                matches,
                pattern,
                target,
            }),
        }
        self.save();
    }

    pub(super) fn delete_link_rule(&mut self, id: &str) {
        self.p.link_rules.retain(|rule| rule.id != id);
        self.save();
    }

    /// Moves a rule up (`-1`) or down (`1`): earlier rules win.
    pub(super) fn move_link_rule(&mut self, id: &str, delta: i32) {
        let Some(from) = self.p.link_rules.iter().position(|r| r.id == id) else {
            return;
        };
        let to = from
            .saturating_add_signed(delta.signum() as isize)
            .min(self.p.link_rules.len() - 1);
        if to != from {
            self.p.link_rules.swap(from, to);
            self.save();
        }
    }

    pub(super) fn set_link_fallback(&mut self, target: Option<RuleTarget>) {
        let target = target.filter(|t| target_exists(t, &self.p.spaces, &self.p.profiles));
        self.p.link_fallback = target;
        self.save();
    }

    /// Forgets rules that point at a space that was just deleted.
    pub(super) fn prune_link_rules(&mut self) {
        self.p.prune_link_rules();
    }
}
