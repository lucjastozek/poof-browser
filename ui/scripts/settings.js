/* ---------- extensions ---------- */
window.__setExts = (l) => {
  EXTS = l;
  if (ov === "ext") paintExts();
};
function paintExts() {
  const box = $("extlist");
  if (!box) return;
  if (!EXTS.length) {
    const e = el("div", "empty-st");
    e.append(ic("puzzle", 26), el("span", null, "No extensions yet"));
    box.replaceChildren(e);
    return;
  }
  box.replaceChildren(
    ...EXTS.map((x) => {
      const r = el("div", "ext");
      r.append(icon({ favicon: x.icon, title: x.name }));
      const m = el("div", "meta");
      m.append(
        el("b", null, x.name),
        el("small", null, "Version " + x.version),
      );
      r.append(m);
      r.append(
        btn("sm ghost danger", "Remove", () =>
          send({ cmd: "remove_ext", dir: x.dir }),
        ),
      );
      return r;
    }),
  );
}
function buildExt(v) {
  const id = el("input", "inp");
  id.placeholder = "Chrome Web Store link or extension ID";
  id.setAttribute("aria-label", "Chrome Web Store link or extension ID");
  const add = btn("primary", "Install", () => {
    if (id.value.trim()) {
      send({ cmd: "install_ext", input: id.value });
      id.value = "";
    }
  });
  id.addEventListener("keydown", (e) => {
    if (e.key === "Enter") add.click();
  });
  const inst = el("div", "row2");
  inst.append(id, add);
  const browse = btn("sm ghost", "Browse the Chrome Web Store", () =>
    go({ cmd: "new_tab", input: "https://chromewebstore.google.com/" }),
  );
  browse.prepend(ic("external", 14));
  const bw = el("div", "row2");
  bw.append(browse);
  const list = el("div");
  list.id = "extlist";
  dialog(v, "Extensions", [
    field(null, inst, bw),
    list,
    el(
      "div",
      "note",
      "On any Web Store page an “Add to poof” button appears in the address bar. Extensions run in every profile. Toolbar icons and popups are not available in this engine (WebView2), but background logic and content scripts, like ad blocking, work.",
    ),
  ]);
  paintExts();
  send({ cmd: "list_ext" });
  setTimeout(() => id.focus(), 30);
}

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
  const keys = el("div", "keys");
  [
    ["New tab", ["Ctrl", "T"]],
    ["Address bar", ["Ctrl", "L"]],
    ["Close tab", ["Ctrl", "W"]],
    ["Switch space", ["Ctrl", "Alt", "←", "→"]],
  ].forEach(([n, k]) => {
    const r = el("div");
    const kk = el("span");
    k.forEach((x) => kk.append(el("kbd", null, x)));
    r.append(el("span", null, n), kk);
    keys.append(r);
  });
  const retention = S.profile_cleanup || {};
  const choices = [["24", "24 hours"], ["48", "48 hours"], ["72", "72 hours"], ["168", "1 week"], ["336", "2 weeks"]];
  const openMenus = [];
  const makeRetention = (key, allowOff, label, description) => {
    const value = String(retention[key] ?? (allowOff ? 0 : 48));
    const wrap = el("div", "retention-control");
    const trigger = el("button", "retention-trigger");
    trigger.type = "button";
    trigger.setAttribute("aria-haspopup", "listbox");
    trigger.setAttribute("aria-expanded", "false");
    const valueLabel = () => (allowOff && value === "0" ? "Off" : (choices.find(([v]) => v === value)?.[1] || "48 hours"));
    const paint = () => {
      trigger.replaceChildren(el("span", null, valueLabel()), ic("chevronDown", 14));
      trigger.setAttribute("aria-label", label + ": " + valueLabel());
    };
    let current = value;
    const menu = el("div", "retention-menu");
    menu.setAttribute("role", "listbox");
    menu.setAttribute("aria-label", label);
    const opts = allowOff ? [["0", "Off"], ...choices] : choices;
    const close = () => {
      menu.remove();
      trigger.setAttribute("aria-expanded", "false");
      document.removeEventListener("pointerdown", outside, true);
      document.removeEventListener("keydown", keys, true);
    };
    const outside = (e) => { if (!menu.contains(e.target) && !trigger.contains(e.target)) close(); };
    const choose = (v) => {
      current = v;
      paint();
      close();
      saveRetention();
    };
    const keys = (e) => {
      if (e.key === "Escape") { e.preventDefault(); close(); trigger.focus(); }
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        const active = Math.max(0, opts.findIndex(([v]) => v === current));
        const next = (active + (e.key === "ArrowDown" ? 1 : opts.length - 1)) % opts.length;
        menu.children[next]?.focus();
      }
    };
    const show = () => {
      document.querySelectorAll(".retention-menu").forEach((m) => m.remove());
      menu.replaceChildren(...opts.map(([v, text], i) => {
        const option = el("button", "retention-option" + (v === current ? " selected" : ""));
        option.type = "button";
        option.setAttribute("role", "option");
        option.setAttribute("aria-selected", String(v === current));
        option.append(el("span", null, text));
        if (v === current) option.append(ic("check", 14));
        option.onclick = () => choose(v);
        return option;
      }));
      document.body.append(menu);
      const r = trigger.getBoundingClientRect();
      const menuHeight = Math.min(opts.length * 36 + 8, 240);
      menu.style.left = Math.max(8, Math.min(r.right - 168, innerWidth - 176)) + "px";
      menu.style.width = "168px";
      menu.style.top = (r.bottom + menuHeight > innerHeight - 8 ? Math.max(8, r.top - menuHeight - 5) : r.bottom + 5) + "px";
      trigger.setAttribute("aria-expanded", "true");
      document.addEventListener("pointerdown", outside, true);
      document.addEventListener("keydown", keys, true);
      menu.querySelector(".selected")?.focus();
      if (!menu.querySelector(".selected")) menu.firstElementChild?.focus();
    };
    trigger.onclick = show;
    wrap.append(trigger);
    const copy = el("div", "retention-copy");
    copy.append(el("span", "retention-name", label), el("small", null, description));
    const row = el("div", "retention-row");
    row.append(copy, wrap);
    return { row, get value() { return current; } };
  };
  const tabAge = makeRetention("tab_retention_hours", false, "Inactive tabs", "Close tabs after they have been unused");
  const cookieAge = makeRetention("cookie_retention_hours", true, "Cookies", "May sign you out of websites");
  const cacheAge = makeRetention("cache_retention_hours", true, "Cache", "Remove older cached files");
  const historyAge = makeRetention("history_retention_hours", true, "Browsing history", "Remove older history entries");
  const cleanup = el("div", "cleanup-section");
  cleanup.append(
    el("p", "note", "Choose how long data stays in this profile before it is cleared."),
    tabAge.row, cookieAge.row, cacheAge.row, historyAge.row,
  );
  const saveRetention = () => send({
    cmd: "update_profile_cleanup",
    tab_retention_hours: Number(tabAge.value),
    cookie_retention_hours: Number(cookieAge.value),
    cache_retention_hours: Number(cacheAge.value),
    history_retention_hours: Number(historyAge.value),
  });
  dialog(v, "Settings", [
    field("Appearance", seg),
    field("Browsing", sw),
    field("Sidebar", compact),
    field("Automatic cleanup", cleanup),
    field(null, el("div", "note", "Settings are shared by every space using this profile. Pinned tabs and favorites are exempt from automatic closing.")),
    field("Shortcuts", keys),
  ]);
}

function build(name) {
  const v = $("stage");
  if (name === "newtab") buildNewTab(v);
  else if (name === "ext") buildExt(v);
  else if (name === "settings") buildSettings(v);
  else if (name.startsWith("space")) buildSpace(v, name.split(":")[1]);
}

window.__toast = (m) => {
  const t = $("toast");
  t.textContent = m;
  t.classList.add("show");
  clearTimeout(t._h);
  t._h = setTimeout(() => t.classList.remove("show"), 4200);
};
