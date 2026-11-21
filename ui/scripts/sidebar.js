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
// relaid out continuously (at most once per frame); only the final width is
// written to disk.
const resizeHandle = $("side-resize");
let sidebarWidth = 280;
let resizingSidebar = false;
let resizePointerId = null;
let resizeFrame = 0;
function applySidebarWidth(width, persist = false) {
  sidebarWidth = Math.max(180, Math.min(480, width));
  document.documentElement.style.setProperty(
    "--sidebar-width",
    `${sidebarWidth}px`,
  );
  if (persist) {
    if (resizeFrame) {
      cancelAnimationFrame(resizeFrame);
      resizeFrame = 0;
    }
    send({ cmd: "sidebar_width", width: sidebarWidth, persist: true });
    return;
  }
  if (!resizeFrame) {
    resizeFrame = requestAnimationFrame(() => {
      resizeFrame = 0;
      send({ cmd: "sidebar_width", width: sidebarWidth, persist: false });
    });
  }
}
resizeHandle.addEventListener("pointerdown", (e) => {
  if (
    e.button !== 0 ||
    document.body.classList.contains("compact") ||
    document.body.classList.contains("collapsed")
  )
    return;
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
$("pageToolbar").append(
  ib("code", "Developer tools (F12)", () => send({ cmd: "devtools_toggle" })),
);
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
// Backup: drag from any empty part of the topbar, in case #window-drag is covered or collapsed.
$("titlebar").addEventListener("pointerdown", (e) => {
  if (e.button !== 0 || e.target.closest("#window-drag")) return;
  if (
    e.target.closest(
      "button,input,textarea,select,a,[role=button],.window-controls",
    )
  )
    return;
  send({ cmd: "window_drag" });
});
{
  const w = el("span", "ico");
  w.append(ic("plus", 15));
  $("newtab").append(w, el("span", "ttl", "New tab"));
}
$("newtab").onclick = () => openView("newtab");
function renameFolderInline(label, folder) {
  if (label.dataset.editing === "true") return;
  label.dataset.editing = "true";
  const input = el("input", "folder-rename");
  input.value = folder.name;
  label.replaceWith(input);
  input.focus();
  input.select();
  let cancel = false;
  input.onkeydown = (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      input.blur();
    }
    if (e.key === "Escape") {
      e.preventDefault();
      cancel = true;
      input.blur();
    }
  };
  input.onblur = () => {
    const name = input.value.trim();
    delete label.dataset.editing;
    input.replaceWith(label);
    if (!cancel && name && name !== folder.name)
      send({ cmd: "rename_folder", id: folder.id, name });
  };
}
function folderNode(s, folder, pinned) {
  const members = (pinned ? s.pinned : s.tabs).filter(
    (t) => t.folder_id === folder.id,
  );
  const group = el(
    "div",
    "tab-folder" + (folder.collapsed ? " collapsed" : ""),
  );
  group.dataset.folderId = folder.id;
  const head = el("div", "folder-head");
  head.dataset.folderId = folder.id;
  const toggle = el("span", "folder-chevron");
  toggle.setAttribute("aria-hidden", "true");
  toggle.append(ic(folder.collapsed ? "chevron-right" : "chevron-down", 13));
  const glyph = el("span", "folder-glyph");
  glyph.append(ic(folder.collapsed ? "folder" : "folder-open", 18));
  const label = el("span", "folder-name", folder.name);
  label.title = "Double-click to rename";
  label.ondblclick = (e) => {
    e.stopPropagation();
    renameFolderInline(label, folder);
  };
  const actions = el("span", "folder-actions");
  actions.append(
    ib("pencil", "Rename folder", () => renameFolderInline(label, folder)),
    ib("trash", "Delete folder", () =>
      send({ cmd: "delete_folder", id: folder.id }),
    ),
  );
  head.append(toggle, glyph, label, actions);
  head.setAttribute("role", "button");
  head.tabIndex = 0;
  head.setAttribute("aria-expanded", String(!folder.collapsed));
  head.setAttribute(
    "aria-label",
    (folder.collapsed ? "Expand " : "Collapse ") + folder.name,
  );
  head.onclick = (e) => {
    if (e.target.closest(".folder-actions,.folder-rename") || noClick) return;
    send({ cmd: "toggle_folder_collapsed", id: folder.id });
  };
  head.onkeydown = (e) => {
    if (
      (e.key === "Enter" || e.key === " ") &&
      !e.target.closest(".folder-actions,.folder-rename")
    ) {
      e.preventDefault();
      send({ cmd: "toggle_folder_collapsed", id: folder.id });
    }
  };
  head.onpointerdown = (e) => watchFolderDrag(e, head, folder);
  group.append(head);
  const contents = el("div", "folder-contents");
  // A collapsed folder's rows are never visible, so they are not built at all.
  if (!folder.collapsed)
    contents.append(...members.map((t) => row(t, pinned ? "pin" : "tab")));
  group.append(contents);
  return group;
}

// Tabs that are split together are shown as one row with two halves.
function groupSplits(nodes) {
  const pairs = S.splits || [];
  const byId = new Map(
    nodes
      .filter((n) => n.classList.contains("row"))
      .map((n) => [n.dataset.id, n]),
  );
  const used = new Set();
  const out = [];
  for (const n of nodes) {
    if (used.has(n)) continue;
    const id = n.classList.contains("row") ? n.dataset.id : null;
    const sp = id && pairs.find((p) => p.a === id || p.b === id);
    const a = sp && byId.get(sp.a),
      b = sp && byId.get(sp.b);
    if (a && b && !used.has(a) && !used.has(b)) {
      used.add(a);
      used.add(b);
      const g = el("div", "split-pair");
      g.append(a, b);
      out.push(g);
    } else out.push(n);
  }
  return out;
}

function folderArea(s, pinned) {
  const folders = (s.folders || []).filter((f) => !!f.pinned === pinned);
  const allFolders = s.folders || [];
  const loose = (pinned ? s.pinned : s.tabs).filter(
    (t) => !allFolders.some((f) => f.id === t.folder_id),
  );
  const nodes = loose.map((t) => row(t, pinned ? "pin" : "tab"));
  // Folder.order is an index in the combined tab + folder sequence, not a
  // folder-only index. This lets a folder sit between any two ordinary tabs.
  folders.sort(
    (a, b) =>
      (a.order ?? Number.MAX_SAFE_INTEGER) -
      (b.order ?? Number.MAX_SAFE_INTEGER),
  );
  for (const folder of folders) {
    const at = Math.max(
      0,
      Math.min(
        nodes.length,
        Number.isFinite(folder.order) ? folder.order : nodes.length,
      ),
    );
    nodes.splice(at, 0, folderNode(s, folder, pinned));
  }
  return groupSplits(nodes);
}
const settingsButton = ib("settings", "Settings", () => openView("settings"));
const spaces = el("div");
spaces.id = "dots";
const newSpaceButton = ib("plus", "New space", () => openView("space:add"));
$("foot").append(settingsButton, spaces, newSpaceButton);
const clearTabsBtn = el("button", "clear-tabs", "Clear");
clearTabsBtn.type = "button";
clearTabsBtn.title = "Close all unpinned tabs";
clearTabsBtn.onpointerdown = (e) => e.stopPropagation();
clearTabsBtn.onclick = (e) => {
  e.stopPropagation();
  send({ cmd: "clear_tabs" });
};
{
  const line = $("line");
  if (getComputedStyle(line).position === "static")
    line.style.position = "relative";
  line.append(clearTabsBtn);
  const st = document.createElement("style");
  st.textContent = `
  #line .clear-tabs{position:absolute;right:4px;top:50%;transform:translateY(-50%);
    padding:2px 8px;border:0;border-radius:var(--r-sm);background:var(--chrome);
    color:var(--ink-2);font:600 11px var(--f-display);cursor:pointer;
    opacity:0;pointer-events:none;transition:opacity .12s,background .12s,color .12s}
  #line:hover .clear-tabs{opacity:1;pointer-events:auto}
  #line .clear-tabs:hover{background:var(--hover);color:var(--ink)}
  #line .clear-tabs:active{background:var(--pressed)}
  #line .clear-tabs[hidden],body.dragging #line .clear-tabs{display:none}`;
  document.head.append(st);
}
$("hdr").onclick = () => openView("space:edit");

// Right-clicking open sidebar space offers folder creation without doing it immediately.
const folderContextMenu = el("div", "folder-context-menu");
const createFolderItem = el("button", "folder-context-item", "New folder");
createFolderItem.type = "button";
createFolderItem.onclick = () => {
  folderContextMenu.remove();
  send({ cmd: "create_folder", name: "New folder" });
};
folderContextMenu.append(createFolderItem);

// Right-clicking a tab row: split view actions.
const rowMenu = el("div", "folder-context-menu");
function showRowMenu(e, t) {
  e.preventDefault();
  e.stopPropagation();
  folderContextMenu.remove();
  rowMenu.remove();
  const item = (label, cmd) => {
    const b = el("button", "folder-context-item", label);
    b.type = "button";
    b.onclick = () => {
      rowMenu.remove();
      send(cmd);
    };
    return b;
  };
  const pair = (S.splits || []).find((p) => p.a === t.id || p.b === t.id);
  const live = S.split && (S.split.a === t.id || S.split.b === t.id);
  const items = [];
  if (pair) {
    items.push(item("Close split view", { cmd: "split_close", id: t.id }));
    if (live)
      items.push(
        item(S.split.stacked ? "Side by side" : "Top and bottom", {
          cmd: "split_orientation",
        }),
      );
  } else if (S.active && S.active !== t.id) {
    items.push(
      item("Open in split view (side by side)", {
        cmd: "split_tab",
        id: t.id,
        stacked: false,
      }),
      item("Open in split view (top and bottom)", {
        cmd: "split_tab",
        id: t.id,
        stacked: true,
      }),
    );
  }
  if (!items.length) return;
  rowMenu.replaceChildren(...items);
  $("side").append(rowMenu);
  const bounds = $("side").getBoundingClientRect();
  rowMenu.style.left =
    Math.max(8, Math.min(e.clientX - bounds.left, bounds.width - 220)) + "px";
  rowMenu.style.top = Math.max(8, e.clientY - bounds.top) + "px";
}

$("side").addEventListener("contextmenu", (e) => {
  if (
    e.target.closest(
      ".row,.tile,.tab-folder,.folder-actions,.folder-rename,input,button,a",
    )
  )
    return;
  e.preventDefault();
  rowMenu.remove();
  folderContextMenu.remove();
  $("side").append(folderContextMenu);
  const bounds = $("side").getBoundingClientRect();
  folderContextMenu.style.left =
    Math.max(8, Math.min(e.clientX - bounds.left, bounds.width - 148)) + "px";
  folderContextMenu.style.top = Math.max(8, e.clientY - bounds.top) + "px";
});
document.addEventListener("pointerdown", (e) => {
  if (!folderContextMenu.contains(e.target)) folderContextMenu.remove();
  if (!rowMenu.contains(e.target)) rowMenu.remove();
});
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    folderContextMenu.remove();
    rowMenu.remove();
  }
});

function minusIcon() {
  try {
    const n = ic("minus");
    if (
      n &&
      (n.childElementCount || (n.querySelector && n.querySelector("svg")))
    )
      return n;
  } catch (_) {}
  const NS = "http://www.w3.org/2000/svg";
  const s = document.createElementNS(NS, "svg");
  s.setAttribute("viewBox", "0 0 24 24");
  s.setAttribute("width", "15");
  s.setAttribute("height", "15");
  const p = document.createElementNS(NS, "path");
  p.setAttribute("d", "M5 12h14");
  p.setAttribute("stroke", "currentColor");
  p.setAttribute("stroke-width", "2");
  p.setAttribute("stroke-linecap", "round");
  p.setAttribute("fill", "none");
  s.append(p);
  return s;
}
function unloadBtn(fn) {
  const b = ib("x", "Unload tab", fn);
  b.replaceChildren(minusIcon());
  return b;
}

function row(t, kind) {
  const r = el(
    "div",
    "row" +
      (S.active === t.id ? " on" : "") +
      (t.loaded ? "" : " idle") +
      (S.split && (S.split.a === t.id || S.split.b === t.id) ? " split" : ""),
  );
  r.dataset.id = t.id;
  r.append(icon(t), el("span", "ttl", t.title || pretty(t.url) || "New tab"));
  const a = el("span", "acts");
  a.append(
    ib("star", "Add to favorites", () => send({ cmd: "fav_tab", id: t.id })),
    unloadBtn(() => send({ cmd: "unload_tab", id: t.id })),
    ib("x", kind === "tab" ? "Close" : "Reset to pinned page", () =>
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
  r.oncontextmenu = (e) => showRowMenu(e, t);
  r.onpointerdown = (e) => watchDrag(e, r, t, kind);
  return r;
}

window.__setState = (s) => {
  if (drag) {
    pending = s;
    return;
  }
  // The native side already drops identical states, so no JSON.stringify of the whole
  // snapshot (favicons included) is needed here. `lastKey` is kept only because the
  // drag code resets it.
  lastKey = "";
  S = s;
  if (Number.isFinite(s.sidebar_width) && !resizingSidebar) {
    sidebarWidth = Math.max(180, Math.min(480, s.sidebar_width));
    document.documentElement.style.setProperty(
      "--sidebar-width",
      `${sidebarWidth}px`,
    );
  }
  document.body.classList.toggle("has-page", !!s.active);
  document.body.classList.toggle("collapsed", !!s.sidebar_collapsed);
  document.body.classList.toggle(
    "peek",
    !!s.sidebar_peek && !!s.sidebar_collapsed,
  );
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
      d.onclick = () => {
        if (noClick) return;
        go({ cmd: "select_tab", id: t.id });
      };
      d.onpointerdown = (e) => watchDrag(e, d, t, "fav");
      d.oncontextmenu = (e) => {
        e.preventDefault();
        send({ cmd: "unfav_tab", id: t.id });
      };
      return d;
    }),
  );
  $("favs").classList.toggle("empty", !s.favorites.length);
  $("pinned").replaceChildren(...folderArea(s, true));
  $("tabs").replaceChildren(...folderArea(s, false));
  clearTabsBtn.hidden = !s.tabs.some((t) => !t.folder_id);
  const dots = [];
  let lastP = null;
  s.spaces.forEach((x) => {
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
// Inline autofill: while typing, complete the address with the best matching visited
// (or popular) site and leave the completion selected, so Backspace removes it and
// Enter accepts it.
function autofillFor(typed) {
  if (!S || !typed || /\s/.test(typed)) return null;
  const bare = typed
    .toLowerCase()
    .replace(/^https?:\/\//, "")
    .replace(/^www\./, "");
  if (!bare || bare.includes("/")) return null;
  for (const s of S.suggestions || []) {
    let host;
    try {
      host = new URL(s.url).host.replace(/^www\./, "").toLowerCase();
    } catch (_) {
      continue;
    }
    if (host.length > bare.length && host.startsWith(bare))
      return typed + host.slice(bare.length);
  }
  return null;
}
url.addEventListener("input", (e) => {
  urlDraft = url.value;
  if (
    e.isComposing ||
    e.inputType !== "insertText" ||
    url.selectionStart !== url.value.length ||
    url.selectionEnd !== url.value.length
  )
    return;
  const typed = url.value;
  const full = autofillFor(typed);
  if (full) {
    url.value = full;
    url.setSelectionRange(typed.length, full.length);
    urlDraft = full;
  }
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
function watchFolderDrag(e, head, folder) {
  if (e.button !== 0 || e.target.closest("button,.folder-actions")) return;
  const x0 = e.clientX,
    y0 = e.clientY,
    ox = e.clientX - head.getBoundingClientRect().left;
  let on = false;
  const mv = (ev) => {
    if (!on) {
      if (Math.hypot(ev.clientX - x0, ev.clientY - y0) < 5) return;
      on = true;
      const ghost = el("div", "dghost folder-ghost");
      ghost.style.width = head.offsetWidth + "px";
      ghost.append(ic("folder", 16), el("span", "ttl", folder.name));
      document.body.append(ghost);
      drag = {
        id: folder.id,
        kind: "folder",
        row: head,
        ghost,
        ox,
        zone: null,
        idx: 0,
      };
      head.classList.add("lift");
      document.body.classList.add("dragging");
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
function overRect(r, x, y, pad = 0) {
  return (
    x >= r.left - pad &&
    x <= r.right + pad &&
    y >= r.top - pad &&
    y <= r.bottom + pad
  );
}

/* ---------- drag a tab onto the page to split ---------- */
const splitDrop = el("div", "split-drop");
const splitZone = el("div", "split-drop-zone");
splitDrop.hidden = true;
splitDrop.append(splitZone);
document.body.append(splitDrop);
{
  const st = document.createElement("style");
  st.textContent = `
  .row.split-target{
  outline:2px solid var(--space);outline-offset:-2px;
  background:color-mix(in srgb,var(--space) 22%,transparent)}
  .split-drop{position:fixed;z-index:45;pointer-events:none;border-radius:8px;
    background:color-mix(in srgb,var(--ink) 5%,transparent)}
  .split-drop[hidden]{display:none}
  .split-drop-zone{position:absolute;border-radius:8px;box-sizing:border-box;
    background:color-mix(in srgb,var(--space) 26%,transparent);
    border:2px solid var(--space);transition:left .1s,top .1s,width .1s,height .1s}`;
  document.head.append(st);
}
let splitSide = null;
let pagesHidden = false;
function contentBounds() {
  const left =
    S && S.sidebar_collapsed ? 0 : $("side").getBoundingClientRect().right;
  return {
    x: left + 8,
    y: 40,
    w: innerWidth - left - 16,
    h: innerHeight - 40 - 8,
  };
}
function splitTarget(x, y, d) {
  if (d.kind === "folder" || !S || !S.active || d.id === S.active) return null;
  if (x <= $("side").getBoundingClientRect().right) return null;
  const b = contentBounds();
  if (x < b.x || x > b.x + b.w || y < b.y || y > b.y + b.h) return null;
  const dx = (x - b.x) / b.w - 0.5,
    dy = (y - b.y) / b.h - 0.5;
  if (Math.abs(dx) >= Math.abs(dy)) return dx < 0 ? "left" : "right";
  return dy < 0 ? "top" : "bottom";
}
function setSplitPreview(side) {
  if (side === splitSide) return;
  splitSide = side;
  if (side && !pagesHidden) {
    pagesHidden = true;
    send({ cmd: "tab_drag", active: true }); // hide the native page so the preview is visible
  } else if (!side && pagesHidden) {
    pagesHidden = false;
    send({ cmd: "tab_drag", active: false });
  }
  if (!side) {
    splitDrop.hidden = true;
    return;
  }
  const b = contentBounds(),
    g = 8;
  Object.assign(splitDrop.style, {
    left: b.x + "px",
    top: b.y + "px",
    width: b.w + "px",
    height: b.h + "px",
  });
  const half = (n) => (n - g) / 2;
  const z = {
    left: { left: 0, top: 0, width: half(b.w), height: b.h },
    right: { left: half(b.w) + g, top: 0, width: half(b.w), height: b.h },
    top: { left: 0, top: 0, width: b.w, height: half(b.h) },
    bottom: { left: 0, top: half(b.h) + g, width: b.w, height: half(b.h) },
  }[side];
  for (const k in z) splitZone.style[k] = z[k] + "px";
  splitDrop.hidden = false;
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
  const over = splitTarget(x, y, d);
  setSplitPreview(over);
  if (over) {
    d.zone = "split:" + over;
    d.idx = 0;
    $("ind").hidden = true;
    $("line").classList.remove("hot");
    return;
  }

  // Hovering the middle of another tab row: split with that tab.
  document
    .querySelectorAll(".row.split-target")
    .forEach((r) => r.classList.remove("split-target"));
  if (d.kind !== "folder") {
    const hit = document.elementFromPoint(x, y)?.closest(".row");
    if (hit && hit.dataset.id && hit.dataset.id !== d.id) {
      const rr = hit.getBoundingClientRect();
      const rel = (y - rr.top) / rr.height;
      if (rel > 0.28 && rel < 0.72) {
        hit.classList.add("split-target");
        d.zone = "splittab:" + hit.dataset.id;
        d.idx = 0;
        $("ind").hidden = true;
        $("line").classList.remove("hot");
        $("pinned").classList.remove("hot-zone");
        $("favs").classList.remove("hot-zone");
        $("lineLbl").textContent = "Release to split";
        return;
      }
    }
  }

  const fr = $("favs").getBoundingClientRect(),
    lr = $("line").getBoundingClientRect();
  let zone = null;
  const sideBounds = $("side").getBoundingClientRect();
  if (
    x > sideBounds.right ||
    x < sideBounds.left ||
    y < sideBounds.top ||
    y > sideBounds.bottom
  )
    zone = null; /* left the sidebar: cancel */
  else if (overRect(fr, x, y, 4) && y < pr.top - 4) zone = "fav";
  else if (
    overRect($("pinned").getBoundingClientRect(), x, y, 8) ||
    y < lr.top + lr.height / 2
  )
    zone = "pinned";
  else zone = "tab";
  let idx = 0,
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
    const ref = tiles[idx] || tiles[tiles.length - 1],
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
  } else if (zone === "pinned") {
    const list = $("pinned");
    const items = [...list.children].filter(
      (c) => c !== (d.kind === "folder" ? d.row.parentElement : d.row),
    );
    items.forEach((c) => {
      const r = c.getBoundingClientRect();
      if (r.top + r.height / 2 < y)
        idx += c.classList.contains("split-pair")
          ? c.querySelectorAll(":scope > .row:not(.lift)").length
          : 1;
    });
    const lb = list.getBoundingClientRect();
    box = { left: lb.left + 6, top: y, width: lb.width - 12, height: 2 };
  } else if (zone === "tab") {
    const items = [...$("tabs").children].filter(
      (c) => c !== (d.kind === "folder" ? d.row.parentElement : d.row),
    );
    items.forEach((c) => {
      const r = c.getBoundingClientRect();
      if (r.top + r.height / 2 < y)
        idx += c.classList.contains("split-pair")
          ? c.querySelectorAll(":scope > .row:not(.lift)").length
          : 1;
    });
    const lb = $("tabs").getBoundingClientRect();
    box = { left: lb.left + 6, top: y, width: lb.width - 12, height: 2 };
  }
  const folderGroup =
    d.kind === "folder"
      ? null
      : document.elementFromPoint(x, y)?.closest(".tab-folder");
  const folderTarget = folderGroup?.querySelector(".folder-head");
  if (folderTarget?.dataset.folderId) {
    zone = "folder:" + folderTarget.dataset.folderId;
    const contents = folderGroup.querySelector(".folder-contents");
    const rows = [...contents.children].filter((c) => c !== d.row);
    const headBottom = folderTarget.getBoundingClientRect().bottom;
    idx = 0;
    rows.forEach((c) => {
      const r = c.getBoundingClientRect();
      if (r.top + r.height / 2 < y) idx++;
    });
    if (y <= headBottom || !rows.length) {
      // Over the folder header (or an empty folder): drop at the end.
      idx = rows.length;
      box = null;
    } else {
      // Over the folder's tabs: show the insertion line between them.
      const ref = rows[idx];
      const r = (ref || rows[rows.length - 1]).getBoundingClientRect();
      const cb = contents.getBoundingClientRect();
      box = {
        left: cb.left + 6,
        top: ref ? r.top : r.bottom,
        width: cb.width - 12,
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
  document
    .querySelectorAll(".folder-head")
    .forEach((h) =>
      h.classList.toggle(
        "drop-target",
        zone === "folder:" + h.dataset.folderId,
      ),
    );
  const changing =
    (zone === "pinned" && d.kind !== "pin") ||
    (zone === "tab" && d.kind !== "tab");
  $("line").classList.toggle("hot", changing);
  $("lineLbl").textContent =
    zone && zone.startsWith("folder:")
      ? "Add to folder"
      : d.kind === "folder"
        ? "Move folder"
        : zone === "pinned"
          ? "Release to pin"
          : d.kind === "fav"
            ? "Release to unfavorite"
            : "Release to unpin";
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
  document
    .querySelectorAll(".row.split-target")
    .forEach((r) => r.classList.remove("split-target"));
  document
    .querySelectorAll(".folder-head")
    .forEach((h) => h.classList.remove("drop-target"));
  if (drop && d.zone && d.zone.startsWith("splittab:")) {
    send({
      cmd: "split_tab",
      id: d.id,
      target: d.zone.slice(9),
      stacked: false,
      first: false,
    });
  } else if (drop && d.zone && d.zone.startsWith("split:")) {
    if (d.kind === "folder") {
      if (d.zone === "pinned" || d.zone === "tab")
        send({ cmd: "move_folder", id: d.id, to: d.zone, index: d.idx });
    } else if (d.zone.startsWith("folder:"))
      send({
        cmd: "move_tab_to_folder",
        id: d.id,
        folder_id: d.zone.slice(7),
        index: d.idx,
      });
    else {
      const to =
        d.zone === "pinned" ? "pinned" : d.zone === "fav" ? "fav" : "tab";
      send({ cmd: "move_tab", id: d.id, to, index: d.idx });
    }
  }

  setSplitPreview(null);

  if (pending) {
    const p = pending;
    pending = null;
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

/* ---------- Ctrl+Tab tab switcher ---------- */
{
  const st = document.createElement("style");
  st.textContent = `
  .tabsw{position:fixed;inset:0;z-index:2147483000;display:flex;align-items:center;justify-content:center;
    background:rgba(0,0,0,.38);backdrop-filter:blur(10px);-webkit-backdrop-filter:blur(10px);
    animation:tabsw-fade .12s ease-out}
  .tabsw[hidden]{display:none}
  .tabsw-panel{max-width:min(920px,calc(100vw - 48px));padding:18px 18px 14px;border-radius:20px;
    background:color-mix(in srgb,var(--chrome) 90%,transparent);border:1px solid var(--line);
    box-shadow:0 24px 70px rgba(0,0,0,.35);animation:tabsw-pop .14s cubic-bezier(.2,.8,.2,1)}
  .tabsw-strip{display:flex;gap:12px;overflow-x:auto;padding:8px 6px 10px;scrollbar-width:none}
  .tabsw-strip::-webkit-scrollbar{display:none}
  .tabsw-card{flex:none;width:148px;cursor:pointer;border-radius:14px;padding:6px;
    transition:background .1s,transform .12s;outline:2px solid transparent;outline-offset:0}
  .tabsw-card:hover{background:var(--hover)}
  .tabsw-card.on{background:var(--pressed);outline-color:var(--space);transform:translateY(-2px) scale(1.03)}
  .tabsw-art{position:relative;height:92px;border-radius:10px;display:flex;align-items:center;justify-content:center;
    background:linear-gradient(160deg,color-mix(in srgb,var(--space) 22%,var(--chrome)),
      color-mix(in srgb,var(--space) 6%,var(--chrome)));
    border:1px solid var(--line);overflow:hidden}
  .tabsw-card.idle .tabsw-art{opacity:.6}
  .tabsw-art img{width:40px;height:40px;object-fit:contain;border-radius:9px}
  .tabsw-art .ltr{font:700 30px var(--f-display);color:var(--ink-2);text-transform:uppercase}
  .tabsw-pin{position:absolute;top:6px;right:6px;width:7px;height:7px;border-radius:50%;background:var(--space)}
  .tabsw-ttl{margin:8px 2px 0;font:600 12px var(--f-display);color:var(--ink);
    white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
  .tabsw-host{margin:1px 2px 0;font:500 11px var(--f-display);color:var(--ink-3);
    white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
  .tabsw-hint{margin-top:6px;text-align:center;font:500 11px var(--f-display);color:var(--ink-3)}
  @keyframes tabsw-fade{from{opacity:0}to{opacity:1}}
  @keyframes tabsw-pop{from{opacity:0;transform:scale(.97)}to{opacity:1;transform:none}}`;
  document.head.append(st);

  const layer = el("div", "tabsw");
  layer.hidden = true;
  const panel = el("div", "tabsw-panel");
  const strip = el("div", "tabsw-strip");
  const hint = el(
    "div",
    "tabsw-hint",
    "Keep holding Ctrl and press Tab to cycle · release to switch · Esc to cancel",
  );
  panel.append(strip, hint);
  layer.append(panel);
  document.body.append(layer);

  let timer = 0;
  const hostOf = (u) => {
    try {
      return new URL(u).host.replace(/^www\./, "");
    } catch (_) {
      return u || "";
    }
  };
  const paint = (i) => {
    [...strip.children].forEach((c, n) => c.classList.toggle("on", n === i));
    const c = strip.children[i];
    if (c) c.scrollIntoView({ inline: "center", block: "nearest" });
  };
  window.__switcher = (p) => {
    strip.replaceChildren(
      ...p.cards.map((t) => {
        const c = el("div", "tabsw-card" + (t.loaded ? "" : " idle"));
        const art = el("div", "tabsw-art");
        if (t.favicon) {
          const img = el("img");
          img.src = t.favicon;
          img.draggable = false;
          img.onerror = () => {
            img.replaceWith(el("span", "ltr", hostOf(t.url)[0] || "•"));
          };
          art.append(img);
        } else art.append(el("span", "ltr", hostOf(t.url)[0] || "•"));
        if (t.pinned) art.append(el("span", "tabsw-pin"));
        const title = t.title || pretty(t.url) || "New tab";
        const ttl = el("div", "tabsw-ttl", title);
        ttl.title = title;
        c.append(
          art,
          ttl,
          el(
            "div",
            "tabsw-host",
            hostOf(t.url) + (t.loaded ? "" : " · asleep"),
          ),
        );
        c.onclick = () => send({ cmd: "switcher_pick", id: t.id });
        return c;
      }),
    );
    layer.hidden = false;
    paint(p.index);
    clearInterval(timer);
    timer = setInterval(() => send({ cmd: "switcher_poll" }), 70);
  };
  window.__switcherSelect = paint;
  window.__switcherClose = () => {
    layer.hidden = true;
    clearInterval(timer);
    timer = 0;
    strip.replaceChildren();
  };
  layer.addEventListener("pointerdown", (e) => {
    if (e.target === layer) send({ cmd: "switcher_cancel" });
  });
  addEventListener(
    "keyup",
    (e) => {
      if (e.key === "Control" && !layer.hidden)
        send({ cmd: "switcher_commit" });
    },
    true,
  );
  addEventListener(
    "keydown",
    (e) => {
      if (layer.hidden) return;
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        send({ cmd: "switcher_cancel" });
      } else if (e.key === "Enter") {
        e.preventDefault();
        send({ cmd: "switcher_commit" });
      }
    },
    true,
  );
}

/* ---------- hover the left edge to reveal a collapsed sidebar (peek) ---------- */
{
  const EDGE = 6; // px from the window's left edge that triggers the peek
  const TOP = 40; // keep the title bar out of it
  let closeT = 0;
  let askedAt = 0;
  const ask = (open) => send({ cmd: "sidebar_peek", open });
  const cancelClose = () => {
    clearTimeout(closeT);
    closeT = 0;
  };
  const scheduleClose = () => {
    if (closeT || !S || !S.sidebar_peek || drag) return;
    closeT = setTimeout(() => {
      closeT = 0;
      if (!drag) ask(false);
    }, 220);
  };
  addEventListener(
    "mousemove",
    (e) => {
      if (!S || !S.sidebar_collapsed || resizingSidebar) return;
      if (!S.sidebar_peek) {
        if (
          e.clientX <= EDGE &&
          e.clientY >= TOP &&
          Date.now() - askedAt > 300
        ) {
          askedAt = Date.now();
          ask(true);
        }
        return;
      }
      if (e.clientX <= sidebarWidth + 14) cancelClose();
      else scheduleClose();
    },
    { passive: true },
  );
  document.documentElement.addEventListener("mouseleave", scheduleClose);
}

// Ask the native side to restore the current space only after this UI is ready
// to receive its first state snapshot.
window.addEventListener("load", () => send({ cmd: "ready" }), { once: true });
