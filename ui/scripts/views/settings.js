/* ---------- settings: a tabbed panel over a picture of the page ---------- */
const SETTINGS_TABS = [
  ["general", "settings-3", "General"],
  ["appearance", "sun", "Appearance"],
  ["privacy", "lock", "Privacy & data"],
  ["links", "airplane", "Air Traffic Control"],
  ["boosts", "magic-2", "Boosts"],
  ["import", "download-2", "Import"],
  ["shortcuts", "key-2", "Shortcuts"],
  ["developer", "code", "Developer"],
];
let settingsTab = "general";

/* A labelled on/off row. `change(on)` runs after the switch flips. */
function switchRow(label, hint, on, change) {
  const sw = el("button", "switch");
  sw.type = "button";
  sw.setAttribute("role", "switch");
  sw.setAttribute("aria-checked", String(!!on));
  const text = el("span", "t");
  text.append(el("span", null, label), el("small", null, hint));
  sw.append(text, el("span", "track"));
  sw.onclick = () => {
    const next = sw.getAttribute("aria-checked") !== "true";
    sw.setAttribute("aria-checked", String(next));
    change(next);
  };
  return sw;
}

/* A segmented control; `pick(value)` runs on click. */
function segmented(options, current, pick, label) {
  const seg = el("div", "seg");
  seg.setAttribute("role", "group");
  seg.setAttribute("aria-label", label);
  options.forEach(([value, text, icon]) => {
    const b = el("button", value === current ? "on" : "");
    b.type = "button";
    b.setAttribute("aria-pressed", String(value === current));
    if (icon) b.append(ic(icon, 15));
    b.append(el("span", null, text));
    b.onclick = () => {
      pick(value);
      [...seg.children].forEach((c, i) => {
        const on = options[i][0] === value;
        c.classList.toggle("on", on);
        c.setAttribute("aria-pressed", String(on));
      });
    };
    seg.append(b);
  });
  return seg;
}

const RETENTION_CHOICES = [
  ["24", "1d", "24 hours"],
  ["48", "2d", "48 hours"],
  ["72", "3d", "72 hours"],
  ["168", "1w", "1 week"],
  ["336", "2w", "2 weeks"],
];

function retentionRow(label, description, current, allowOff, change) {
  const options = allowOff
    ? [["0", "Off", "Off"], ...RETENTION_CHOICES]
    : RETENTION_CHOICES;
  const group = el("div", "retention-options");
  group.setAttribute("role", "group");
  group.setAttribute("aria-label", label);
  options.forEach(([value, short, full]) => {
    const b = el("button", "retention-pill", short);
    b.type = "button";
    b.title = full;
    b.setAttribute("aria-label", label + ": " + full);
    b.classList.toggle("selected", value === current);
    b.setAttribute("aria-pressed", String(value === current));
    b.onclick = () => {
      [...group.children].forEach((c) => {
        const on = c === b;
        c.classList.toggle("selected", on);
        c.setAttribute("aria-pressed", String(on));
      });
      change(value);
    };
    group.append(b);
  });
  const copy = el("div", "retention-copy");
  copy.append(
    el("span", "retention-name", label),
    el("small", null, description),
  );
  const row = el("div", "retention-row");
  row.append(copy, el("div", "retention-control"));
  row.lastChild.append(group);
  return row;
}

/* The search engine for text typed in the address bar. */
function searchEngineRow() {
  const select = el("select", "engine-select");
  select.setAttribute("aria-label", "Search engine");
  (S.search_engines || []).forEach(({ id, name }) => {
    const o = el("option", null, name);
    o.value = id;
    o.selected = id === S.search_engine;
    select.append(o);
  });
  select.onchange = () => {
    S.search_engine = select.value;
    send({ cmd: "set_search_engine", id: select.value });
  };
  const copy = el("div", "retention-copy");
  copy.append(
    el("span", "retention-name", "Search engine"),
    el("small", null, "Used when you type something that is not an address"),
  );
  const row = el("div", "retention-row");
  row.append(copy, el("div", "retention-control"));
  row.lastChild.append(select);
  return row;
}

function generalPanel() {
  return [
    field("Search", searchEngineRow()),
    field(
      "Browsing",
      switchRow(
        "Picture in picture",
        "Pop out a playing video when you leave its tab",
        S.auto_pip,
        (on) => {
          S.auto_pip = on;
          send({ cmd: "set_auto_pip", on });
        },
      ),
      switchRow(
        "Restore tabs on startup",
        "Reopen the tabs you had open when you last closed poof",
        S.restore_tabs,
        (on) => {
          S.restore_tabs = on;
          send({ cmd: "set_restore_tabs", on });
        },
      ),
    ),
    field(
      "Experimental",
      switchRow(
        "Mini window",
        "Open links from other apps in a small window, and add a quick search window to the command bar",
        S.mini_window,
        (on) => {
          S.mini_window = on;
          send({ cmd: "set_mini_window", on });
        },
      ),
      switchRow(
        "Peek links",
        "Alt+click a link to preview it over the page",
        S.peek_links,
        (on) => {
          S.peek_links = on;
          send({ cmd: "set_peek_links", on });
        },
      ),
    ),
    field(
      "Feedback",
      switchRow(
        "Sound",
        "A very quiet tick when you drop a tab onto Pinned",
        S.feedback_sound,
        (on) => {
          S.feedback_sound = on;
          send({ cmd: "set_feedback", sound: on });
          if (on) window.__feedback();
        },
      ),
    ),
    scheduleField(),
    defaultBrowserSettingsField(),
  ];
}

/* "At [time range], open [space]": checked once, when poof opens. */
function scheduleField() {
  const rules = (S.schedules || []).map((r) => ({ ...r }));
  const list = el("div", "sched-list");
  const commit = () => {
    S.schedules = rules.map((r) => ({ ...r }));
    send({ cmd: "set_schedules", rules });
  };
  const time = (value, label, set) => {
    const i = el("input", "inp sched-time");
    i.type = "time";
    i.value = value;
    i.setAttribute("aria-label", label);
    i.onchange = () => {
      if (i.value) set(i.value);
      else i.value = value;
    };
    return i;
  };
  const paint = () => {
    list.replaceChildren(
      ...rules.map((r, i) => {
        const row = el("div", "sched-row");
        const sel = el("select", "engine-select");
        sel.setAttribute("aria-label", "Space to open");
        (S.spaces || []).forEach((sp) => {
          const o = el("option", null, sp.name);
          o.value = sp.id;
          sel.append(o);
        });
        sel.value = r.space;
        sel.onchange = () => {
          r.space = sel.value;
          commit();
        };
        row.append(
          el("span", null, "At"),
          time(r.start, "Start time", (v) => {
            r.start = v;
            commit();
          }),
          el("span", null, "to"),
          time(r.end, "End time", (v) => {
            r.end = v;
            commit();
          }),
          el("span", null, "open"),
          sel,
          ib("close", "Remove schedule", () => {
            rules.splice(i, 1);
            commit();
            paint();
          }),
        );
        return row;
      }),
    );
  };
  const add = el("button", "btn", "Add a schedule");
  add.type = "button";
  add.onclick = () => {
    if (rules.length >= 20 || !(S.spaces || []).length) return;
    rules.push({ start: "09:00", end: "17:00", space: S.current });
    commit();
    paint();
  };
  paint();
  return field(
    "Space schedule",
    el(
      "p",
      "note",
      "When you open poof during a time range, it opens that space instead of the last one. The first matching rule wins.",
    ),
    list,
    add,
  );
}

function appearancePanel() {
  const theme = segmented(
    [
      ["system", "System", "computer"],
      ["light", "Light", "sun"],
      ["dark", "Dark", "moon"],
    ],
    S.theme,
    (k) => {
      S.theme = k;
      lastKey = "";
      applyMode(k);
      send({ cmd: "set_theme", theme: k });
    },
    "Theme",
  );
  const compact = switchRow(
    "Compact sidebar",
    "Keep an icon-only sidebar when it is open",
    S.sidebar_compact,
    (on) => {
      S.sidebar_compact = on;
      document.body.classList.toggle(
        "compact",
        on && !document.body.classList.contains("collapsed"),
      );
      send({ cmd: "sidebar_compact", compact: on });
    },
  );
  return [field("Theme", theme), field("Sidebar", compact)];
}

function privacyPanel() {
  const retention = S.profile_cleanup || {};
  const values = {
    tab_retention_hours: String(retention.tab_retention_hours ?? 48),
    cookie_retention_hours: String(retention.cookie_retention_hours ?? 0),
    cache_retention_hours: String(retention.cache_retention_hours ?? 0),
    history_retention_hours: String(retention.history_retention_hours ?? 0),
  };
  const save = () =>
    send({
      cmd: "update_profile_cleanup",
      ...Object.fromEntries(
        Object.entries(values).map(([k, v]) => [k, Number(v)]),
      ),
    });
  const row = (key, allowOff, label, description) =>
    retentionRow(label, description, values[key], allowOff, (value) => {
      values[key] = value;
      save();
    });
  const cleanup = el("div", "cleanup-section");
  cleanup.append(
    el(
      "p",
      "note",
      "Choose how long data stays in this profile before it is cleared.",
    ),
    row(
      "tab_retention_hours",
      false,
      "Inactive tabs",
      "Close tabs after they have been unused",
    ),
    row(
      "cookie_retention_hours",
      true,
      "Cookies",
      "May sign you out of websites",
    ),
    row("cache_retention_hours", true, "Cache", "Remove older cached files"),
    row(
      "history_retention_hours",
      true,
      "Browsing history",
      "Remove older history entries",
    ),
  );
  return [
    passwordSettingsField(),
    field("Automatic cleanup", cleanup),
    field(
      null,
      el(
        "div",
        "note",
        "Settings are shared by every space using this profile. Pinned tabs and favorites are exempt from automatic closing.",
      ),
    ),
  ];
}

/* ---- import from other browsers ---- */
let importBusy = false;
let importUi = null; // the button and status line of the panel that is open

/* The native side reports when it is done (also if the panel was closed meanwhile). */
window.__importFinished = ({ ok, message }) => {
  importBusy = false;
  if (importUi && importUi.status.isConnected) {
    importUi.run.disabled = false;
    importUi.status.textContent = message;
    importUi.status.classList.toggle("error", !ok);
  }
  if (window.__toast) window.__toast(message);
};

function importPanel() {
  const choice = { spaces: true, passwords: true };
  const status = el("div", "note", importBusy ? "Importing…" : "");
  status.setAttribute("role", "status");
  const run = btn("primary", "Import from Arc", () => {
    if (importBusy) return;
    importBusy = true;
    run.disabled = true;
    status.classList.remove("error");
    status.textContent = "Importing…";
    send({ cmd: "import_arc", ...choice });
  });
  run.disabled = importBusy;
  importUi = { run, status };
  return [
    field(
      "Arc",
      el(
        "div",
        "note",
        "Quit Arc first so its saved passwords can be read. Nothing here is replaced: spaces, profiles and passwords you already have are kept.",
      ),
      switchRow(
        "Profiles, spaces and tabs",
        "Spaces with their pinned tabs and folders, favorites, and the profiles they use",
        choice.spaces,
        (on) => (choice.spaces = on),
      ),
      switchRow(
        "Passwords",
        "Saved logins of each Arc profile (Windows only)",
        choice.passwords,
        (on) => (choice.passwords = on),
      ),
      run,
      status,
    ),
  ];
}

function developerPanel() {
  const dock = segmented(
    [
      ["right", "Right"],
      ["left", "Left"],
      ["bottom", "Bottom"],
      ["top", "Top"],
    ],
    S.devtools_position,
    (value) => {
      S.devtools_position = value;
      send({ cmd: "devtools_position", position: value });
    },
    "Developer tools dock",
  );
  const size = segmented(
    [
      ["320", "Narrow"],
      ["420", "Default"],
      ["560", "Wide"],
    ],
    String(Number(S.devtools_size || 420)),
    (value) => {
      S.devtools_size = Number(value);
      send({ cmd: "devtools_size", size: Number(value), persist: true });
    },
    "Developer tools size",
  );
  return [field("Dock position", dock), field("Panel size", size)];
}

/* ---- keyboard shortcuts ---- */
const CODE_NAMES = {
  Tab: "Tab",
  Enter: "Enter",
  Space: "Space",
  Backspace: "Backspace",
  Delete: "Delete",
  Insert: "Insert",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  ArrowUp: "Up",
  ArrowDown: "Down",
  Semicolon: ";",
  Equal: "=",
  Comma: ",",
  Minus: "-",
  Period: ".",
  Slash: "/",
  Backquote: "`",
  BracketLeft: "[",
  Backslash: "\\",
  BracketRight: "]",
  Quote: "'",
};
/* The chord key for a keyboard event, in the same spelling the browser uses. */
function chordKey(code) {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit\d$/.test(code)) return code.slice(5);
  if (/^F\d{1,2}$/.test(code)) return code;
  return CODE_NAMES[code] || null;
}
const KEY_GLYPHS = { Left: "←", Right: "→", Up: "↑", Down: "↓" };

function chordNode(chord) {
  const box = el("span", "sc-chord");
  if (!chord) {
    box.append(el("span", "sc-none", "Not set"));
    return box;
  }
  chord
    .split("+")
    .forEach((k) => box.append(el("kbd", null, KEY_GLYPHS[k] || k)));
  return box;
}

function shortcutsPanel() {
  const host = el("div", "sc");
  let capturing = null;
  let hint = "";

  const stop = () => {
    if (!capturing) return;
    capturing = null;
    hint = "";
    removeEventListener("keydown", onKey, true);
    removeEventListener("blur", stop);
    send({ cmd: "shortcut_capture", active: false });
    paint();
  };
  const onKey = (e) => {
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") return stop();
    if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) return;
    if (
      (e.key === "Backspace" || e.key === "Delete") &&
      !(e.ctrlKey || e.altKey || e.shiftKey)
    ) {
      const id = capturing;
      stop();
      return send({ cmd: "set_shortcut", id, chord: "" });
    }
    const key = chordKey(e.code);
    if (!key) return;
    if (!(e.ctrlKey || e.altKey || /^F\d+$/.test(key))) {
      hint = "Include Ctrl or Alt, or use a function key.";
      return paint();
    }
    const chord = [
      e.ctrlKey && "Ctrl",
      e.altKey && "Alt",
      e.shiftKey && "Shift",
      key,
    ]
      .filter(Boolean)
      .join("+");
    const id = capturing;
    stop();
    send({ cmd: "set_shortcut", id, chord });
  };
  const start = (id) => {
    if (capturing) stop();
    capturing = id;
    hint = "";
    send({ cmd: "shortcut_capture", active: true });
    addEventListener("keydown", onKey, true);
    addEventListener("blur", stop);
    paint();
  };
  window.__stopShortcutCapture = stop;

  function row(item) {
    const recording = capturing === item.id;
    const r = el("div", "sc-row");
    r.setAttribute("role", "row");
    const key = el("button", "sc-key" + (recording ? " recording" : ""));
    key.type = "button";
    key.setAttribute(
      "aria-label",
      "Shortcut for " +
        item.label +
        ": " +
        (item.chord || "not set") +
        ". Click to change.",
    );
    if (recording) {
      key.append(el("span", "sc-prompt", hint || "Press keys…"));
    } else {
      key.append(chordNode(item.chord), ic("pencil", 14));
      key.lastChild.classList.add("sc-edit");
    }
    key.onclick = () => (recording ? stop() : start(item.id));
    const reset = el("span", "sc-reset");
    if (item.chord !== item.default && !recording) {
      const b = el("button", "ib");
      b.type = "button";
      b.title =
        "Reset to default" +
        (item.default ? " (" + item.default + ")" : " (none)");
      b.setAttribute("aria-label", "Reset " + item.label + " to default");
      b.append(ic("refresh-2", 15));
      b.onclick = () => send({ cmd: "reset_shortcut", id: item.id });
      reset.append(b);
    }
    r.append(el("span", "sc-label", item.label), key, reset);
    return r;
  }

  function paint() {
    const groups = new Map();
    (S.shortcuts || []).forEach((item) => {
      if (!groups.has(item.group)) groups.set(item.group, []);
      groups.get(item.group).push(item);
    });
    const top = el("div", "sc-top");
    top.append(
      el(
        "p",
        "note",
        capturing
          ? "Press the new keys. Backspace removes the shortcut, Esc cancels."
          : "Click a shortcut to change it.",
      ),
      btn("ghost sm", "Reset all", () => send({ cmd: "reset_shortcuts" })),
    );
    host.replaceChildren(top);
    groups.forEach((items, group) => {
      const section = el("section", "sc-group");
      section.append(el("h3", null, group), ...items.map(row));
      host.append(section);
    });
  }
  paint();
  host._repaint = () => {
    if (!capturing) paint();
  };
  return host;
}

function buildSettings(v) {
  const wrap = el("div", "settings-scrim");
  const modal = el("div", "settings-modal");
  modal.setAttribute("role", "dialog");
  modal.setAttribute("aria-label", "Settings");
  const nav = el("nav", "settings-nav");
  nav.setAttribute("role", "tablist");
  nav.setAttribute("aria-orientation", "vertical");
  const content = el("div", "settings-content");
  const panels = {
    general: generalPanel,
    appearance: appearancePanel,
    privacy: privacyPanel,
    links: linkRoutingPanel,
    boosts: boostsPanel,
    import: importPanel,
    developer: developerPanel,
  };
  let shortcutsHost = null;

  function show(tab) {
    if (window.__stopShortcutCapture) window.__stopShortcutCapture();
    settingsTab = tab;
    shortcutsHost = null;
    [...nav.children].forEach((b) => {
      const on = b.dataset.tab === tab;
      b.classList.toggle("on", on);
      b.setAttribute("aria-selected", String(on));
      b.tabIndex = on ? 0 : -1;
    });
    const title = SETTINGS_TABS.find((t) => t[0] === tab)[2];
    const head = el("header");
    head.append(
      el("h2", null, title),
      ib("close", "Close settings", closeView),
    );
    const body = el("div", "settings-panel");
    body.setAttribute("role", "tabpanel");
    if (tab === "shortcuts") {
      shortcutsHost = shortcutsPanel();
      body.append(shortcutsHost);
    } else {
      body.append(...panels[tab]());
    }
    content.replaceChildren(head, body);
  }

  SETTINGS_TABS.forEach(([id, icon, label]) => {
    const b = el("button", "settings-tab");
    b.type = "button";
    b.dataset.tab = id;
    b.setAttribute("role", "tab");
    b.append(ic(icon), el("span", null, label));
    b.onclick = () => show(id);
    b.onkeydown = (e) => {
      const step = { ArrowDown: 1, ArrowRight: 1, ArrowUp: -1, ArrowLeft: -1 }[
        e.key
      ];
      if (!step) return;
      e.preventDefault();
      const next =
        (SETTINGS_TABS.findIndex((t) => t[0] === id) +
          step +
          SETTINGS_TABS.length) %
        SETTINGS_TABS.length;
      show(SETTINGS_TABS[next][0]);
      nav.children[next].focus();
    };
    nav.append(b);
  });
  modal.append(nav, content);
  wrap.append(modal);
  wrap.addEventListener("pointerdown", (e) => {
    if (e.target === wrap) closeView();
  });
  v.replaceChildren(wrap);
  show(settingsTab);

  // The shortcut list follows state pushes (a moved chord changes two rows).
  const base = window.__setState;
  if (!base._settingsWrapped) {
    const wrapped = (s) => {
      base(s);
      if (ov !== "settings") return;
      if (window.__repaintShortcuts) window.__repaintShortcuts();
      if (window.__repaintLinkRules) window.__repaintLinkRules();
      if (window.__repaintBoosts) window.__repaintBoosts();
    };
    wrapped._settingsWrapped = true;
    window.__setState = wrapped;
  }
  window.__repaintShortcuts = () => shortcutsHost && shortcutsHost._repaint();
}
