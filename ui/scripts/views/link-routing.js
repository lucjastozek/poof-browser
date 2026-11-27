/* ---------- settings: Air Traffic Control (link routing) ----------
   Rules for links opened from other apps: the first rule that matches decides which space
   (or profile) the link opens in. The native side owns the rules; this panel only edits them
   and repaints when the state it receives has different rules, spaces or profiles. */
const LINK_MATCHES = [
  ["domain", "Domain is"],
  ["contains", "Address contains"],
];
const LINK_PATTERN_HINT = {
  domain: "github.com",
  contains: "/pull/",
};
const LINK_KEEP_CURRENT = "current";

/* "space:<id>" / "profile:<id>" as used by the target menus, and back. */
const targetValue = (t) => (t ? t.kind + ":" + t.id : LINK_KEEP_CURRENT);
function parseTargetValue(value) {
  const at = value.indexOf(":");
  if (at < 0) return null;
  return { kind: value.slice(0, at), id: value.slice(at + 1) };
}

function linkTargetSelect(label, current, withKeep) {
  const select = el("select", "engine-select link-target");
  select.setAttribute("aria-label", label);
  const add = (parent, value, text) => {
    const o = el("option", null, text);
    o.value = value;
    parent.append(o);
  };
  if (withKeep) add(select, LINK_KEEP_CURRENT, "The space that is showing");
  const profiles = S.profiles || [];
  const named = (id) => (profiles.find((p) => p.id === id) || {}).name || "";
  const spaces = el("optgroup");
  spaces.label = "Spaces";
  (S.spaces || []).forEach((sp) => {
    const extra = profiles.length > 1 ? " · " + named(sp.profile_id) : "";
    add(spaces, "space:" + sp.id, sp.name + extra);
  });
  select.append(spaces);
  if (profiles.length > 1) {
    const group = el("optgroup");
    group.label = "Profiles";
    profiles.forEach((p) => add(group, "profile:" + p.id, p.name));
    select.append(group);
  }
  select.value = current;
  if (select.value !== current) select.selectedIndex = 0;
  return select;
}

function linkRuleRow(rule, index, count) {
  const kind = segmentedSelect(LINK_MATCHES, rule.matches, "Match type");
  const pattern = el("input", "inp link-pattern");
  pattern.value = rule.pattern;
  pattern.setAttribute("aria-label", "Text to match");
  pattern.spellcheck = false;
  const target = linkTargetSelect("Open in", targetValue(rule.target), false);
  const save = () => {
    if (!pattern.value.trim()) return (pattern.value = rule.pattern);
    send({
      cmd: "save_link_rule",
      id: rule.id,
      matches: kind.value,
      pattern: pattern.value,
      target: parseTargetValue(target.value),
    });
  };
  kind.onchange = save;
  target.onchange = save;
  pattern.onchange = save;
  pattern.onkeydown = (e) => {
    if (e.key === "Enter") pattern.blur();
  };
  const tools = el("div", "link-tools");
  const up = ib("arrow-up", "Check this rule earlier", () =>
    send({ cmd: "move_link_rule", id: rule.id, delta: -1 }),
  );
  const down = ib("arrow-down", "Check this rule later", () =>
    send({ cmd: "move_link_rule", id: rule.id, delta: 1 }),
  );
  up.disabled = index === 0;
  down.disabled = index === count - 1;
  tools.append(
    up,
    down,
    ib("delete-2", "Delete rule", () =>
      send({ cmd: "delete_link_rule", id: rule.id }),
    ),
  );
  const row = el("div", "link-rule");
  row.append(kind, pattern, el("span", "link-arrow", "→"), target, tools);
  return row;
}

/* A drop-down for the match type; shares the select styling of the search-engine row. */
function segmentedSelect(options, current, label) {
  const select = el("select", "engine-select link-kind");
  select.setAttribute("aria-label", label);
  options.forEach(([value, text]) => {
    const o = el("option", null, text);
    o.value = value;
    select.append(o);
  });
  select.value = current;
  return select;
}

function linkAddForm() {
  const kind = segmentedSelect(LINK_MATCHES, "domain", "Match type");
  const pattern = el("input", "inp link-pattern");
  pattern.placeholder = LINK_PATTERN_HINT.domain;
  pattern.setAttribute("aria-label", "Text to match");
  pattern.spellcheck = false;
  kind.onchange = () => (pattern.placeholder = LINK_PATTERN_HINT[kind.value]);
  const current = S.current || (S.spaces[0] || {}).id;
  const target = linkTargetSelect("Open in", "space:" + current, false);
  const add = () => {
    if (!pattern.value.trim()) return pattern.focus();
    send({
      cmd: "save_link_rule",
      matches: kind.value,
      pattern: pattern.value,
      target: parseTargetValue(target.value),
    });
    pattern.value = "";
  };
  pattern.onkeydown = (e) => {
    if (e.key === "Enter") add();
  };
  const row = el("div", "link-rule link-add");
  row.append(
    kind,
    pattern,
    el("span", "link-arrow", "→"),
    target,
    btn("primary sm", "Add rule", add),
  );
  return row;
}

function linkRoutingPanel() {
  const list = el("div", "link-list");
  const fallback = el("div", "link-fallback");

  const paintRules = () => {
    const rules = S.link_rules || [];
    if (!rules.length) {
      list.replaceChildren(
        el(
          "p",
          "note link-empty",
          "No rules yet. Links from other apps open in the space that is showing.",
        ),
      );
      return;
    }
    list.replaceChildren(
      ...rules.map((r, i) => linkRuleRow(r, i, rules.length)),
    );
  };
  const paintFallback = () => {
    const select = linkTargetSelect(
      "Open other links in",
      targetValue(S.link_fallback),
      true,
    );
    select.onchange = () =>
      send({
        cmd: "set_link_fallback",
        target:
          select.value === LINK_KEEP_CURRENT
            ? null
            : parseTargetValue(select.value),
      });
    const copy = el("div", "retention-copy");
    copy.append(
      el("span", "retention-name", "Everything else"),
      el("small", null, "Links that match none of the rules"),
    );
    fallback.replaceChildren(copy, select);
  };
  const signature = () =>
    JSON.stringify([S.link_rules, S.link_fallback, S.spaces, S.profiles]);
  let painted = "";
  const paint = () => {
    painted = signature();
    paintRules();
    paintFallback();
  };
  paint();
  /* Called after every state push: tab titles change all the time and must not rebuild
     a row the user is typing in, so only different rules or spaces repaint. */
  window.__repaintLinkRules = () => {
    if (list.isConnected && signature() !== painted) paint();
  };

  return [
    field(
      "Link routing",
      el(
        "div",
        "note",
        "Links you open from other apps go to the first rule that matches, checked from the top. A domain rule also covers its subdomains.",
      ),
      list,
    ),
    field("Add a rule", linkAddForm()),
    field(null, fallback),
  ];
}
