/* ---------- static sidebar chrome ---------- */
const collapseBtn = ib("panelLeftClose", "Collapse sidebar", () => {
  const collapsed = !document.body.classList.contains("collapsed");
  document.body.classList.toggle("collapsed", collapsed);
  collapseBtn.title = collapsed ? "Expand sidebar" : "Collapse sidebar";
  collapseBtn.setAttribute("aria-label", collapseBtn.title);
  collapseBtn.replaceChildren(
    ic(collapsed ? "panelLeftOpen" : "panelLeftClose"),
  );
  send({ cmd: "sidebar_collapsed", collapsed });
});
$("topNav").append(collapseBtn);
$("pageNav").append(
  ib("back", "Back", () => send({ cmd: "back" })),
  ib("fwd", "Forward", () => send({ cmd: "forward" })),
  ib("reload", "Reload", () => send({ cmd: "reload" })),
);
$("copyLink").append(ic("link", 15));
$("copyLink").onclick = async () => {
  const value = S && S.url ? S.url : "";
  if (!value) return;
  try {
    await navigator.clipboard.writeText(value);
    window.__toast("Link copied");
  } catch (_) {
    const tmp = el("textarea");
    tmp.value = value;
    tmp.style.position = "fixed";
    tmp.style.opacity = "0";
    document.body.append(tmp);
    tmp.select();
    try {
      document.execCommand("copy");
      window.__toast("Link copied");
    } finally {
      tmp.remove();
    }
  }
};
$("pageToolbar").append(ib("puzzle", "Extensions", () => openView("ext")));
$("winMin").onclick = () => send({ cmd: "window_minimize" });
$("winMax").onclick = () => send({ cmd: "window_toggle_maximize" });
$("winClose").onclick = () => send({ cmd: "window_close" });
$("titlebar").addEventListener("dblclick", (e) => {
  if (e.target.closest(".window-controls")) return;
  send({ cmd: "window_toggle_maximize" });
});
$("window-drag").addEventListener("pointerdown", (e) => {
  if (e.button === 0) send({ cmd: "window_drag" });
});
{
  const w = el("span", "ico");
  w.append(ic("plus", 15));
  $("newtab").append(w, el("span", "ttl", "New tab"));
}
$("newtab").onclick = () => openView("newtab");
const settingsButton = ib("settings", "Settings", () => openView("settings"));
const spaces = el("div");
spaces.id = "dots";
const newSpaceButton = ib("plus", "New space", () => openView("space:add"));
$("foot").append(settingsButton, spaces, newSpaceButton);
$("hdr").onclick = () => openView("space:edit");

function row(t, kind) {
  const r = el(
    "div",
    "row" + (S.active === t.id ? " on" : "") + (t.loaded ? "" : " idle"),
  );
  r.dataset.id = t.id;
  r.append(icon(t), el("span", "ttl", t.title || pretty(t.url) || "New tab"));
  const a = el("span", "acts");
  a.append(
    ib(kind === "pin" ? "unpin" : "pin", kind === "pin" ? "Unpin" : "Pin", () =>
      send({ cmd: kind === "pin" ? "unpin_tab" : "pin_tab", id: t.id }),
    ),
    ib("star", "Add to favorites", () => send({ cmd: "fav_tab", id: t.id })),
    ib("x", kind === "tab" ? "Close" : "Unload", () =>
      send({ cmd: "close_tab", id: t.id }),
    ),
  );
  r.append(a);
  r.onclick = () => {
    if (noClick) return;
    go({ cmd: "select_tab", id: t.id });
  };
  r.onauxclick = (e) => {
    if (e.button === 1) send({ cmd: "close_tab", id: t.id });
  };
  r.onpointerdown = (e) => watchDrag(e, r, t, kind);
  return r;
}

window.__setState = (s) => {
  if (drag) {
    pending = s;
    return;
  }
  const key = JSON.stringify(s);
  if (key === lastKey) return;
  lastKey = key;
  S = s;
  document.body.classList.toggle("collapsed", !!s.sidebar_collapsed);
  document.body.classList.toggle(
    "compact",
    !!s.sidebar_compact && !s.sidebar_collapsed,
  );
  const isCollapsed = !!s.sidebar_collapsed;
  collapseBtn.title = isCollapsed ? "Expand sidebar" : "Collapse sidebar";
  collapseBtn.setAttribute("aria-label", collapseBtn.title);
  collapseBtn.replaceChildren(
    ic(isCollapsed ? "panelLeftOpen" : "panelLeftClose"),
  );
  const idx = s.spaces.findIndex((x) => x.id === s.current),
    sp = s.spaces[idx];
  if (pref !== s.theme) applyMode(s.theme || "system");
  if (!ov || !ov.startsWith("space")) applyColor(sp.color);
  const prof = s.profiles.find((p) => p.id === sp.profile_id);
  const chipi = el("span", "chipi");
  chipi.append(sic(sp.icon, 15));
  const pen = el("span", "pen");
  pen.append(ic("pencil", 14));
  $("hdr").title = "Edit space";
  $("hdr").replaceChildren(chipi, el("b", null, sp.name), pen);
  $("favs").replaceChildren(
    ...s.favorites.map((t) => {
      const d = el(
        "div",
        "tile" + (s.active === t.id ? " on" : "") + (t.loaded ? "" : " idle"),
      );
      d.dataset.id = t.id;
      d.title = (t.title || t.url) + "\nRight-click to remove";
      d.append(icon(t));
      d.onclick = () => go({ cmd: "select_tab", id: t.id });
      d.oncontextmenu = (e) => {
        e.preventDefault();
        send({ cmd: "unfav_tab", id: t.id });
      };
      return d;
    }),
  );
  $("favs").classList.toggle("empty", !s.favorites.length);
  $("pinned").replaceChildren(...s.pinned.map((t) => row(t, "pin")));
  $("tabs").replaceChildren(...s.tabs.map((t) => row(t, "tab")));
  const dots = [];
  let lastP = null;
  s.spaces.forEach((x) => {
    if (lastP && lastP !== x.profile_id) dots.push(el("i", "vsep"));
    const b = el("button", "sp" + (x.id === s.current ? " on" : ""));
    b.append(sic(x.icon, 17));
    const pn = (s.profiles.find((p) => p.id === x.profile_id) || {}).name || "";
    b.title = x.name + (pn ? " · " + pn : "");
    b.setAttribute("aria-label", x.name);
    b.onclick = () => go({ cmd: "switch_space", id: x.id });
    lastP = x.profile_id;
    dots.push(b);
  });
  $("dots").replaceChildren(...dots);
  if (prevIdx >= 0 && idx !== prevIdx) {
    const pane = $("pane");
    pane.classList.remove("l", "r");
    void pane.offsetWidth;
    pane.classList.add(idx > prevIdx ? "l" : "r");
  }
  prevIdx = idx;
  $("addExt").hidden = !s.store_ext;
  syncUrl();
  if (!s.active && !ov)
    openView("newtab"); /* no page open: show the start page */
};
$("addExt").onclick = () => send({ cmd: "install_ext", input: S.store_ext });

/* ---------- address bar: shows the site name, the full address while editing ---------- */
const url = $("url");
let urlDraft = null;
let lastSyncedKey = null;
function syncUrl() {
  const currentKey = S ? (S.active || "") + "\n" + (S.url || "") : "";
  if (currentKey !== lastSyncedKey) {
    lastSyncedKey = currentKey;
    urlDraft = null;
  }
  if (document.activeElement !== url && urlDraft === null) {
    url.value = S ? pretty(S.url) : "";
  }
}
window.__focusUrl = () => {
  url.focus();
};
url.addEventListener("focus", () => {
  if (urlDraft === null) {
    url.value = S ? S.url || "" : "";
    url.select();
  }
});
url.addEventListener("input", () => {
  urlDraft = url.value;
});
url.addEventListener("blur", syncUrl);
url.addEventListener("keydown", (e) => {
  if (e.key === "Enter" && url.value.trim()) {
    urlDraft = url.value;
    go({ cmd: "navigate", input: url.value });
    url.blur();
  }
  if (e.key === "Escape") {
    url.blur();
  }
});

/* ---------- drag to pin / unpin / favorite / reorder ---------- */
function watchDrag(e, row, t, kind) {
  if (e.button !== 0 || e.target.closest(".acts")) return;
  const x0 = e.clientX,
    y0 = e.clientY,
    ox = e.clientX - row.getBoundingClientRect().left;
  let on = false;
  const mv = (ev) => {
    if (!on) {
      if (Math.hypot(ev.clientX - x0, ev.clientY - y0) < 5) return;
      on = true;
      beginDrag(row, t, kind, ox);
    }
    updateDrag(ev);
  };
  const up = (ev) => {
    removeEventListener("pointermove", mv);
    removeEventListener("pointerup", up);
    removeEventListener("pointercancel", up);
    if (on) {
      noClick = true;
      setTimeout(() => (noClick = false), 0);
      endDrag(ev.type === "pointerup");
    }
  };
  addEventListener("pointermove", mv);
  addEventListener("pointerup", up);
  addEventListener("pointercancel", up);
}
function beginDrag(row, t, kind, ox) {
  const g = row.cloneNode(false);
  g.className = "dghost";
  g.style.width = row.offsetWidth + "px";
  g.append(icon(t), el("span", "ttl", t.title || pretty(t.url) || "New tab"));
  document.body.append(g);
  drag = { id: t.id, kind, row, ghost: g, ox, zone: null, idx: 0 };
  row.classList.add("lift");
  document.body.classList.add("dragging");
}
function overRect(r, x, y, pad = 0) {
  return (
    x >= r.left - pad &&
    x <= r.right + pad &&
    y >= r.top - pad &&
    y <= r.bottom + pad
  );
}
function updateDrag(ev) {
  const d = drag,
    x = ev.clientX,
    y = ev.clientY;
  d.ghost.style.left = x - d.ox + "px";
  d.ghost.style.top = y - 17 + "px";
  const pane = $("pane"),
    pr = pane.getBoundingClientRect();
  if (y < pr.top + 28) pane.scrollTop -= 8;
  else if (y > pr.bottom - 28) pane.scrollTop += 8;
  const fr = $("favs").getBoundingClientRect(),
    lr = $("line").getBoundingClientRect();
  let zone = null;
  if (x > 290 || x < 0 || y < 0 || y > innerHeight)
    zone = null; /* left the sidebar: cancel */
  else if (overRect(fr, x, y, 4) && y < pr.top - 4) zone = "fav";
  else zone = y < lr.top + lr.height / 2 ? "pinned" : "tab";
  let idx = 0,
    ref = null,
    after = false,
    box = null;
  if (zone === "fav") {
    const tiles = [...$("favs").children].filter((c) => c !== d.row);
    tiles.forEach((c) => {
      const r = c.getBoundingClientRect();
      if (
        r.bottom < y ||
        (y <= r.bottom && y >= r.top && r.left + r.width / 2 < x)
      )
        idx++;
    });
    ref = tiles[idx] || tiles[tiles.length - 1];
    after = !tiles[idx];
    if (ref) {
      const r = ref.getBoundingClientRect();
      box = {
        left: after ? r.right + 2 : r.left - 4,
        top: r.top + 4,
        width: 2,
        height: r.height - 8,
      };
    }
  } else if (zone) {
    const list = $(zone === "pinned" ? "pinned" : "tabs");
    const rows = [...list.children].filter((c) => c !== d.row);
    rows.forEach((c) => {
      const r = c.getBoundingClientRect();
      if (r.top + r.height / 2 < y) idx++;
    });
    const lb = list.getBoundingClientRect();
    if (rows.length) {
      const r = (rows[idx] || rows[rows.length - 1]).getBoundingClientRect();
      box = {
        left: lb.left + 6,
        top: rows[idx] ? r.top - 2 : r.bottom + 1,
        width: lb.width - 12,
        height: 2,
      };
    }
  }
  d.zone = zone;
  d.idx = idx;
  const ind = $("ind");
  if (box) {
    ind.hidden = false;
    Object.assign(ind.style, {
      left: box.left + "px",
      top: box.top + "px",
      width: box.width + "px",
      height: box.height + "px",
    });
  } else ind.hidden = true;
  const free = (n) => ![...$(n).children].some((c) => c !== d.row);
  $("pinned").classList.toggle("hot-zone", zone === "pinned" && free("pinned"));
  $("favs").classList.toggle("hot-zone", zone === "fav" && free("favs"));
  const changing =
    (zone === "pinned" && d.kind !== "pin") ||
    (zone === "tab" && d.kind === "pin");
  $("line").classList.toggle("hot", changing);
  $("lineLbl").textContent =
    zone === "pinned" ? "Release to pin" : "Release to unpin";
}
function endDrag(drop) {
  const d = drag;
  drag = null;
  d.ghost.remove();
  d.row.classList.remove("lift");
  document.body.classList.remove("dragging");
  $("ind").hidden = true;
  $("line").classList.remove("hot");
  $("pinned").classList.remove("hot-zone");
  $("favs").classList.remove("hot-zone");
  if (drop && d.zone) {
    const to =
      d.zone === "pinned" ? "pinned" : d.zone === "fav" ? "fav" : "tab";
    send({ cmd: "move_tab", id: d.id, to, index: d.idx });
  }
  if (pending) {
    const p = pending;
    pending = null;
    lastKey = "";
    window.__setState(p);
  }
}
addEventListener(
  "keydown",
  (e) => {
    if (e.key === "Escape" && drag) {
      e.preventDefault();
      endDrag(false);
    }
  },
  true,
);
