/* ---------- start page ---------- */
const looksUrl = (q) =>
  /^https?:\/\//i.test(q) ||
  /^localhost/.test(q) ||
  /^[\w-]+(\.[\w-]+)+(\/\S*)?$/.test(q);
window.__openNew = () => openView("newtab");
function siteTile(s) {
  const d = el("div", "site");
  d.append(
    icon(s, true),
    el("span", "n", s.label.replace(/\.(com|org|net|io|dev|app|co)$/, "")),
  );
  d.title = s.url;
  d.onclick = () => go({ cmd: "new_tab", input: s.url });
  return tileKeys(d);
}
/* Opens the built-in PDF viewer and tools. */
function pdfTile() {
  const d = el("div", "site");
  d.append(ic("external-link", 18), el("span", "n", "PDF tools"));
  d.title = "Open or edit PDF files";
  d.onclick = () => go({ cmd: "new_tab", input: "poof://pdf" });
  return tileKeys(d);
}
/* Tiles are divs: make them reachable and activatable from the keyboard. */
function tileKeys(d) {
  d.tabIndex = 0;
  d.setAttribute("role", "link");
  d.onkeydown = (e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      d.click();
    }
  };
  return d;
}
function buildNewTab(v) {
  const wrap = el("div", "center"),
    box = el("div", "w nt");
  const mark = el("div", "mark", "poof");
  mark.append(el("i"));
  box.append(mark);
  const f = el("div", "field");
  f.append(ic("search", 18));
  const q = el("input");
  q.placeholder = "Search tabs, history, bookmarks and commands";
  q.spellcheck = false;
  q.setAttribute("aria-label", "Search tabs, history, bookmarks and commands");
  q.setAttribute("role", "combobox");
  q.setAttribute("aria-autocomplete", "list");
  q.setAttribute("aria-expanded", "false");
  f.append(q);
  box.append(f);
  const body = el("div");
  box.append(body);
  wrap.append(box);
  v.replaceChildren(wrap);
  let items = [],
    sel = 0;
  const run = (it) => it && it.run();
  function showHome() {
    body.replaceChildren();
    items = [];
    const list = [
      ...S.suggestions.filter((x) => !x.popular),
      ...S.suggestions.filter((x) => x.popular),
    ].slice(0, 8);
    const g = el("div", "sites");
    list.forEach((x) => g.append(siteTile(x)));
    g.append(pdfTile());
    body.append(g);
  }
  /* Browser commands the palette can run (">" shows only these). */
  function commands() {
    const act = (cmd) => () => go(cmd);
    const list = [
      ["New space", "add", () => openView("space:add"), "create"],
      ["Edit current space", "pencil", () => openView("space:edit"), "rename"],
      ["Settings", "settings-3", () => openView("settings"), "preferences"],
      ["Extensions", "plugin-2", () => openView("ext"), "addons"],
      [
        "History",
        "time",
        () => {
          closeView();
          window.__toggleHistory && window.__toggleHistory();
        },
        "visited",
      ],
      [
        "Downloads",
        "download-2",
        () => {
          closeView();
          window.__toggleDownloads && window.__toggleDownloads();
        },
        "files",
      ],
      ["Go back", "arrow-left", act({ cmd: "back" }), "previous"],
      ["Go forward", "arrow-right", act({ cmd: "forward" }), "next"],
      ["Reload page", "refresh-1", act({ cmd: "reload" }), "refresh"],
      ["Take a screenshot", "camera", act({ cmd: "capture" }), "capture"],
      ["Developer tools", "code", act({ cmd: "devtools_toggle" }), "inspect"],
      [
        "Zap an element on this page",
        "magic-2",
        act({ cmd: "start_zap" }),
        "boost hide",
      ],
      [
        S.sidebar_collapsed ? "Show sidebar" : "Hide sidebar",
        "sidebar-left-close",
        act({ cmd: "sidebar_collapsed", collapsed: !S.sidebar_collapsed }),
        "collapse",
      ],
      [
        "Theme: light",
        "sun",
        act({ cmd: "set_theme", theme: "light" }),
        "appearance",
      ],
      [
        "Theme: dark",
        "sun",
        act({ cmd: "set_theme", theme: "dark" }),
        "appearance",
      ],
      [
        "Theme: system",
        "sun",
        act({ cmd: "set_theme", theme: "system" }),
        "appearance",
      ],
    ];
    SETTINGS_TABS.forEach(([id, icon, label]) =>
      list.push([
        "Settings: " + label,
        icon,
        () => {
          settingsTab = id;
          openView("settings");
        },
        "",
      ]),
    );
    (S.spaces || []).forEach((sp) => {
      if (sp.id !== S.current)
        list.push([
          "Switch to space: " + sp.name,
          "arrow-right",
          act({ cmd: "switch_space", id: sp.id, keep_focus: false }),
          "",
        ]);
    });
    if (S.mini_window)
      list.push([
        "Quick search window",
        "search",
        act({ cmd: "open_mini" }),
        "mini little arc",
      ]);
    list.push([
      "Undo last close or delete",
      "undo",
      act({ cmd: "undo" }),
      "restore reopen",
    ]);
    list.push([
      "Show empty-list tips again",
      "star",
      act({ cmd: "reset_hints" }),
      "hints favorites pinned reset",
    ]);
    if (S.active) {
      list.push(
        ["Close tab", "close", act({ cmd: "close_tab", id: S.active }), ""],
        [
          "Add tab to favorites",
          "star",
          act({ cmd: "fav_tab", id: S.active }),
          "bookmark",
        ],
      );
    }
    return list.map(([t, i, run, kw]) => ({
      ic: i,
      t,
      sub: "Command",
      kw: t + " " + kw,
      run,
    }));
  }
  /* Every search word must appear; earlier and prefix matches rank higher. */
  function rank(text, words) {
    const low = text.toLowerCase();
    let score = 0;
    for (const w of words) {
      const at = low.indexOf(w);
      if (at < 0) return -1;
      score += at === 0 ? 0 : at < 12 ? 1 : 2;
    }
    return score;
  }
  function pick(list, textOf, words, max) {
    return list
      .map((x, i) => [rank(textOf(x), words), i, x])
      .filter((r) => r[0] >= 0)
      .sort((p, q) => p[0] - q[0] || p[1] - q[1])
      .slice(0, max)
      .map((r) => r[2]);
  }
  let historyRows = [],
    historyTimer = 0;
  window.__paletteHistory = (rows) => {
    historyRows = rows;
    if (q.isConnected && q.value.trim()) showResults(q.value.trim(), true);
  };
  function showResults(txt, keepSel) {
    const only = txt.startsWith(">");
    const term = (only ? txt.slice(1) : txt).trim();
    const words = term.toLowerCase().split(/\s+/).filter(Boolean);
    const prev = items[sel];
    items = [];
    if (!only) {
      items.push({
        ic: looksUrl(txt) ? "external-link" : "search",
        t: looksUrl(txt) ? "Go to " + txt : "Search for “" + txt + "”",
        sub: "",
        run: () => go({ cmd: "new_tab", input: txt }),
      });
      const tabItem = (t, label) => ({
        fav: t,
        t: t.title || pretty(t.url),
        sub: label,
        run: () => go({ cmd: "select_tab", id: t.id }),
      });
      const text = (t) => (t.title || "") + " " + t.url;
      pick(S.tabs, text, words, 4).forEach((t) =>
        items.push(tabItem(t, "Tab")),
      );
      pick([...S.favorites, ...S.pinned], text, words, 4).forEach((t) =>
        items.push(tabItem(t, "Bookmark")),
      );
    }
    pick(commands(), (c) => c.kw, words, only ? 12 : 4).forEach((c) =>
      items.push(c),
    );
    if (!only) {
      const seen = new Set(items.map((i) => i.fav && i.fav.url));
      const rows = historyRows.filter(
        (h) => !seen.has(h.url) && seen.add(h.url),
      );
      pick(rows, (h) => (h.title || "") + " " + h.url, words, 5).forEach((h) =>
        items.push({
          fav: h,
          t: h.title || pretty(h.url),
          sub: "History · " + pretty(h.url),
          run: () => go({ cmd: "new_tab", input: h.url }),
        }),
      );
      pick(S.suggestions, (x) => x.label + " " + x.url, words, 3).forEach((x) =>
        items.push({
          fav: x,
          t: x.label,
          sub: x.url.replace(/^https?:\/\//, "").replace(/\/$/, ""),
          run: () => go({ cmd: "new_tab", input: x.url }),
        }),
      );
    }
    sel =
      keepSel && prev
        ? Math.max(
            0,
            items.findIndex((i) => i.t === prev.t),
          )
        : 0;
    paint();
  }
  function paint() {
    const list = el("div", "res");
    list.id = "nt-res";
    list.setAttribute("role", "listbox");
    list.setAttribute("aria-label", "Results");
    items.forEach((it, i) => {
      const r = el("div", "row" + (i === sel ? " sel" : ""));
      r.id = "nt-opt-" + i;
      r.setAttribute("role", "option");
      r.setAttribute("aria-selected", String(i === sel));
      if (it.ic) {
        const w = el("span", "ico");
        w.append(ic(it.ic, 14));
        r.append(w);
      } else r.append(icon(it.fav, true));
      r.append(el("span", "ttl", it.t), el("span", "sub", it.sub));
      if (i === sel) r.append(el("kbd", null, "Enter"));
      r.onclick = () => run(it);
      r.onmouseenter = () => {
        if (sel !== i) {
          sel = i;
          [...list.children].forEach((c, j) => {
            c.classList.toggle("sel", j === i);
            c.setAttribute("aria-selected", String(j === i));
          });
          q.setAttribute("aria-activedescendant", "nt-opt-" + i);
        }
      };
      list.append(r);
    });
    body.replaceChildren(list);
    q.setAttribute("aria-expanded", String(items.length > 0));
    q.setAttribute("aria-controls", "nt-res");
    q.setAttribute("aria-activedescendant", "nt-opt-" + sel);
    const cur = list.children[sel];
    if (cur) cur.scrollIntoView({ block: "nearest" });
  }
  q.addEventListener("input", () => {
    const text = q.value.trim();
    text ? showResults(text) : showHome();
    clearTimeout(historyTimer);
    historyTimer = setTimeout(
      () =>
        send({
          cmd: "list_history",
          query: text.startsWith(">") ? "" : text,
        }),
      80,
    );
  });
  q.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown" && items.length) {
      e.preventDefault();
      sel = (sel + 1) % items.length;
      paint();
    } else if (e.key === "ArrowUp" && items.length) {
      e.preventDefault();
      sel = (sel - 1 + items.length) % items.length;
      paint();
    } else if (e.key === "Enter" && q.value.trim()) run(items[sel]);
    else if (e.key === "Escape" && S && S.active) closeView();
  });
  showHome();
  q.focus();
  requestAnimationFrame(() => q.focus());
  window.__focusNewTab = () => q.focus();
}
