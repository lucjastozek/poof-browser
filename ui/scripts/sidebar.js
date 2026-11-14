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

// Drag the sidebar's right edge to resize it. The native content view is
// relaid out continuously; only the final width is written to disk.
const resizeHandle = $("side-resize");
let sidebarWidth = 280;
let resizingSidebar = false;
let resizePointerId = null;
function applySidebarWidth(width, persist = false) {
  sidebarWidth = Math.max(180, Math.min(480, width));
  document.documentElement.style.setProperty("--sidebar-width", `${sidebarWidth}px`);
  send({ cmd: "sidebar_width", width: sidebarWidth, persist });
}
resizeHandle.addEventListener("pointerdown", (e) => {
  if (e.button !== 0 || document.body.classList.contains("compact") ||
      document.body.classList.contains("collapsed")) return;
  e.preventDefault();
  resizingSidebar = true;
  resizePointerId = e.pointerId;
  resizeHandle.setPointerCapture(e.pointerId);
  document.body.classList.add("resizing-sidebar");
});
resizeHandle.addEventListener("pointermove", (e) => {
  if (!resizingSidebar || e.pointerId !== resizePointerId) return;
  applySidebarWidth(e.clientX);
});
function finishSidebarResize(e) {
  if (!resizingSidebar || (e && e.pointerId !== resizePointerId)) return;
  resizingSidebar = false;
  document.body.classList.remove("resizing-sidebar");
  resizePointerId = null;
  applySidebarWidth(sidebarWidth, true);
}
resizeHandle.addEventListener("pointerup", finishSidebarResize);
resizeHandle.addEventListener("pointercancel", finishSidebarResize);
resizeHandle.addEventListener("keydown", (e) => {
  if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
  e.preventDefault();
  applySidebarWidth(sidebarWidth + (e.key === "ArrowRight" ? 12 : -12), true);
});
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
const folderMenu = el("div", "folder-context-menu");
folderMenu.hidden = true;
document.body.append(folderMenu);
function hideFolderMenu() { folderMenu.hidden = true; folderMenu.replaceChildren(); }
function folderMenuItem(label, action) {
  const b = el("button", "folder-menu-item", label);
  b.type = "button";
  b.onclick = () => { hideFolderMenu(); action(); };
  folderMenu.append(b);
}
function showFolderMenu(e, folder = null) {
  e.preventDefault();
  e.stopPropagation();
  hideFolderMenu();
  folderMenuItem("New folder", () => send({ cmd: "create_folder", name: "New folder" }));
  if (folder) {
    folderMenu.append(el("div", "folder-menu-sep"));
    folderMenuItem("Rename folder", () => {
      const name = prompt("Rename folder", folder.name);
      if (name && name.trim()) send({ cmd: "rename_folder", id: folder.id, name: name.trim() });
    });
    folderMenuItem(folder.collapsed ? "Expand folder" : "Collapse folder", () => send({ cmd: "toggle_folder_collapsed", id: folder.id }));
    folderMenuItem(folder.pinned ? "Unpin folder" : "Pin folder", () => send({ cmd: "toggle_folder_pinned", id: folder.id }));
    folderMenuItem("Delete folder", () => {
      if (confirm("Delete this folder? Its tabs will remain open and become ungrouped.")) send({ cmd: "delete_folder", id: folder.id });
    });
  }
  folderMenu.hidden = false;
  const x = Math.min(e.clientX, innerWidth - 190);
  const y = Math.min(e.clientY, innerHeight - folderMenu.childElementCount * 34 - 12);
  folderMenu.style.left = Math.max(4, x) + "px";
  folderMenu.style.top = Math.max(4, y) + "px";
}
$("side").addEventListener("contextmenu", (e) => {
  const head = e.target.closest(".folder-head");
  const folder = head && (S.folders || []).find((f) => f.id === head.dataset.folderId);
  showFolderMenu(e, folder || null);
});
addEventListener("pointerdown", (e) => {
  if (!folderMenu.hidden && !folderMenu.contains(e.target)) hideFolderMenu();
});
addEventListener("keydown", (e) => { if (e.key === "Escape") hideFolderMenu(); });
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
    ib("folder", "Move to folder", () => {
      const folders = (S.folders || []).filter((f) => f.space_id === S.current);
      const current = folders.find((f) => f.id === t.folder_id);
      const choice = prompt("Enter a folder name, or leave blank to ungroup this tab:", current ? current.name : "");
      if (choice === null) return;
      const folder = folders.find((f) => f.name.toLowerCase() === choice.trim().toLowerCase());
      if (choice.trim() && !folder) { window.__toast("Folder not found"); return; }
      send({ cmd: "move_tab_to_folder", id: t.id, folder_id: folder ? folder.id : null });
    }),
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
  if (Number.isFinite(s.sidebar_width)) {
    sidebarWidth = Math.max(180, Math.min(480, s.sidebar_width));
    document.documentElement.style.setProperty("--sidebar-width", `${sidebarWidth}px`);
  }
  document.body.classList.toggle("has-page", !!s.active);
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
  const folders = [...(s.folders || [])].sort((a, b) => Number(b.pinned) - Number(a.pinned));
  const folderNodes = [];
  for (const f of folders) {
    const members = s.tabs.filter((t) => t.folder_id === f.id);
    const group = el("div", "tab-folder" + (f.pinned ? " is-pinned" : ""));
    const head = el("div", "folder-head");
    head.dataset.folderId = f.id;
    const toggle = el("button", "folder-toggle");
    toggle.type = "button";
    toggle.setAttribute("aria-expanded", String(!f.collapsed));
    toggle.setAttribute("aria-label", (f.collapsed ? "Expand " : "Collapse ") + f.name);
    toggle.append(ic(f.collapsed ? "chevronRight" : "chevronDown", 14));
    toggle.onclick = () => send({ cmd: "toggle_folder_collapsed", id: f.id });
    const label = el("span", "folder-name", f.name);
    label.title = f.name;
    label.ondblclick = (e) => {
      e.stopPropagation();
      const name = prompt("Rename folder", f.name);
      if (name && name.trim()) send({ cmd: "rename_folder", id: f.id, name: name.trim() });
    };
    const count = el("span", "folder-count", String(members.length));
    const actions = el("span", "folder-actions");
    actions.append(
      ib(f.pinned ? "pinOff" : "pin", f.pinned ? "Unpin folder" : "Pin folder", () => send({ cmd: "toggle_folder_pinned", id: f.id })),
      ib("pencil", "Rename folder", () => {
        const name = prompt("Rename folder", f.name);
        if (name && name.trim()) send({ cmd: "rename_folder", id: f.id, name: name.trim() });
      }),
      ib("trash", "Delete folder (tabs stay open)", () => {
        if (confirm("Delete this folder? Its tabs will remain open and become ungrouped.")) send({ cmd: "delete_folder", id: f.id });
      })
    );
    head.append(toggle, label, count, actions);
    group.append(head);
    if (!f.collapsed) group.append(...members.map((t) => row(t, "tab")));
    folderNodes.push(group);
  }
  const ungrouped = s.tabs.filter((t) => !folders.some((f) => f.id === t.folder_id));
  $("tabs").replaceChildren(...folderNodes, ...ungrouped.map((t) => row(t, "tab")));
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
  const sideBounds = $("side").getBoundingClientRect();
  if (x > sideBounds.right || x < sideBounds.left || y < sideBounds.top || y > sideBounds.bottom)
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
  const folderTarget = document.elementFromPoint(x, y)?.closest(".folder-head");
  if (folderTarget?.dataset.folderId) zone = "folder:" + folderTarget.dataset.folderId;
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
  document.querySelectorAll(".folder-head").forEach((h) => h.classList.toggle("drop-target", zone === "folder:" + h.dataset.folderId));
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
  document.querySelectorAll(".folder-head").forEach((h) => h.classList.remove("drop-target"));
  if (drop && d.zone) {
    if (d.zone.startsWith("folder:")) {
      send({ cmd: "move_tab_to_folder", id: d.id, folder_id: d.zone.slice(7) });
    } else {
      const to = d.zone === "pinned" ? "pinned" : d.zone === "fav" ? "fav" : "tab";
      send({ cmd: "move_tab", id: d.id, to, index: d.idx });
    }
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


// Ask the native side to restore the current space only after this UI is ready
// to receive its first state snapshot.
window.addEventListener("load", () => send({ cmd: "ready" }), { once: true });
