/* ---------- settings ---------- */
function buildSettings(v) {
  const seg = el("div", "seg");
  [
    ["system", "monitor", "System"],
    ["light", "sun", "Light"],
    ["dark", "moon", "Dark"],
  ].forEach(([k, i, l]) => {
    const b = el("button", S.theme === k ? "on" : "");
    b.type = "button";
    b.append(ic(i, 15), el("span", null, l));
    b.onclick = () => {
      S.theme = k;
      lastKey = "";
      applyMode(k);
      send({ cmd: "set_theme", theme: k });
      [...seg.children].forEach((c, j) =>
        c.classList.toggle("on", j === ["system", "light", "dark"].indexOf(k)),
      );
    };
    seg.append(b);
  });
  const compact = el("button", "switch");
  compact.type = "button";
  compact.setAttribute("role", "switch");
  compact.setAttribute("aria-checked", String(!!S.sidebar_compact));
  const compactText = el("span", "t");
  compactText.append(
    el("span", null, "Compact sidebar"),
    el("small", null, "Keep an icon-only sidebar when it is open"),
  );
  compact.append(compactText, el("span", "track"));
  compact.onclick = () => {
    const on = compact.getAttribute("aria-checked") !== "true";
    compact.setAttribute("aria-checked", String(on));
    S.sidebar_compact = on;
    document.body.classList.toggle(
      "compact",
      on && !document.body.classList.contains("collapsed"),
    );
    send({ cmd: "sidebar_compact", compact: on });
  };
  const sw = el("button", "switch");
  sw.type = "button";
  sw.setAttribute("role", "switch");
  sw.setAttribute("aria-checked", String(!!S.auto_pip));
  const t = el("span", "t");
  t.append(
    el("span", null, "Picture in picture"),
    el("small", null, "Pop out a playing video when you leave its tab"),
  );
  sw.append(t, el("span", "track"));
  sw.onclick = () => {
    const on = sw.getAttribute("aria-checked") !== "true";
    sw.setAttribute("aria-checked", String(on));
    S.auto_pip = on;
    send({ cmd: "set_auto_pip", on });
  };
  const restore = el("button", "switch");
  restore.type = "button";
  restore.setAttribute("role", "switch");
  restore.setAttribute("aria-checked", String(!!S.restore_tabs));
  const restoreText = el("span", "t");
  restoreText.append(
    el("span", null, "Restore tabs on startup"),
    el("small", null, "Reopen the tabs you had open when you last closed poof"),
  );
  restore.append(restoreText, el("span", "track"));
  restore.onclick = () => {
    const on = restore.getAttribute("aria-checked") !== "true";
    restore.setAttribute("aria-checked", String(on));
    S.restore_tabs = on;
    send({ cmd: "set_restore_tabs", on });
  };
  const keys = el("div", "keys");
  [
    ["New tab", ["Ctrl", "T"]],
    ["Address bar", ["Ctrl", "L"]],
    ["Close tab", ["Ctrl", "W"]],
    ["Switch space", ["Ctrl", "Alt", "←", "→"]],
    ["Downloads", ["Ctrl", "J"]],
    ["Developer tools", ["F12"]],
  ].forEach(([n, k]) => {
    const r = el("div");
    const kk = el("span");
    k.forEach((x) => kk.append(el("kbd", null, x)));
    r.append(el("span", null, n), kk);
    keys.append(r);
  });
  const retention = S.profile_cleanup || {};
  const choices = [
    ["24", "24 hours"],
    ["48", "48 hours"],
    ["72", "72 hours"],
    ["168", "1 week"],
    ["336", "2 weeks"],
  ];
  const makeRetention = (key, allowOff, label, description) => {
    let current = String(retention[key] ?? (allowOff ? 0 : 48));
    const wrap = el("div", "retention-control");
    const group = el("div", "retention-options");
    group.setAttribute("role", "group");
    group.setAttribute("aria-label", label);
    const opts = allowOff
      ? [
          ["0", "Off", "Off"],
          ...choices.map(([v], i) => [
            v,
            ["1d", "2d", "3d", "1w", "2w"][i],
            choices[i][1],
          ]),
        ]
      : choices.map(([v], i) => [
          v,
          ["1d", "2d", "3d", "1w", "2w"][i],
          choices[i][1],
        ]);
    const paint = () => {
      [...group.children].forEach((button) => {
        const selected = button.dataset.value === current;
        button.classList.toggle("selected", selected);
        button.setAttribute("aria-pressed", String(selected));
      });
    };
    opts.forEach(([value, shortLabel, fullLabel]) => {
      const option = el("button", "retention-pill");
      option.type = "button";
      option.dataset.value = value;
      option.textContent = shortLabel;
      option.title = fullLabel;
      option.setAttribute("aria-label", label + ": " + fullLabel);
      option.setAttribute("aria-pressed", String(value === current));
      option.onclick = () => {
        if (current === value) return;
        current = value;
        paint();
        saveRetention();
      };
      group.append(option);
    });
    paint();
    wrap.append(group);
    const copy = el("div", "retention-copy");
    copy.append(
      el("span", "retention-name", label),
      el("small", null, description),
    );
    const row = el("div", "retention-row");
    row.append(copy, wrap);
    return {
      row,
      get value() {
        return current;
      },
    };
  };
  const tabAge = makeRetention(
    "tab_retention_hours",
    false,
    "Inactive tabs",
    "Close tabs after they have been unused",
  );
  const cookieAge = makeRetention(
    "cookie_retention_hours",
    true,
    "Cookies",
    "May sign you out of websites",
  );
  const cacheAge = makeRetention(
    "cache_retention_hours",
    true,
    "Cache",
    "Remove older cached files",
  );
  const historyAge = makeRetention(
    "history_retention_hours",
    true,
    "Browsing history",
    "Remove older history entries",
  );
  const cleanup = el("div", "cleanup-section");
  cleanup.append(
    el(
      "p",
      "note",
      "Choose how long data stays in this profile before it is cleared.",
    ),
    tabAge.row,
    cookieAge.row,
    cacheAge.row,
    historyAge.row,
  );
  const saveRetention = () =>
    send({
      cmd: "update_profile_cleanup",
      tab_retention_hours: Number(tabAge.value),
      cookie_retention_hours: Number(cookieAge.value),
      cache_retention_hours: Number(cacheAge.value),
      history_retention_hours: Number(historyAge.value),
    });
  const devtoolsDock = el("div", "seg");
  [
    ["right", "Right"],
    ["left", "Left"],
    ["bottom", "Bottom"],
    ["top", "Top"],
  ].forEach(([value, label]) => {
    const option = el(
      "button",
      S.devtools_position === value ? "on" : "",
      label,
    );
    option.type = "button";
    option.setAttribute("aria-pressed", String(S.devtools_position === value));
    option.onclick = () => {
      S.devtools_position = value;
      send({ cmd: "devtools_position", position: value });
      [...devtoolsDock.children].forEach((child) => {
        const selected = child === option;
        child.classList.toggle("on", selected);
        child.setAttribute("aria-pressed", String(selected));
      });
    };
    devtoolsDock.append(option);
  });
  const devtoolsSize = el("div", "seg");
  [
    [320, "Narrow"],
    [420, "Default"],
    [560, "Wide"],
  ].forEach(([value, label]) => {
    const option = el(
      "button",
      Number(S.devtools_size || 420) === value ? "on" : "",
      label,
    );
    option.type = "button";
    option.setAttribute(
      "aria-pressed",
      String(Number(S.devtools_size || 420) === value),
    );
    option.onclick = () => {
      S.devtools_size = value;
      send({ cmd: "devtools_size", size: value, persist: true });
      [...devtoolsSize.children].forEach((child) => {
        const selected = child === option;
        child.classList.toggle("on", selected);
        child.setAttribute("aria-pressed", String(selected));
      });
    };
    devtoolsSize.append(option);
  });
  dialog(v, "Settings", [
    field("Appearance", seg),
    field("Developer tools · dock", devtoolsDock),
    field("Developer tools · size", devtoolsSize),
    field("Browsing", sw, restore),
    defaultBrowserSettingsField(),
    passwordSettingsField(),
    field("Sidebar", compact),
    field("Automatic cleanup", cleanup),
    field(
      null,
      el(
        "div",
        "note",
        "Settings are shared by every space using this profile. Pinned tabs and favorites are exempt from automatic closing.",
      ),
    ),
    field("Shortcuts", keys),
  ]);
}
