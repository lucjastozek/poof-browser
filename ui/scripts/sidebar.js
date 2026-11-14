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
    if (e.key === "Enter") { e.preventDefault(); input.blur(); }
    if (e.key === "Escape") { e.preventDefault(); cancel = true; input.blur(); }
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
  const members = (pinned ? s.pinned : s.tabs).filter(t => t.folder_id === folder.id);
  const group = el("div", "tab-folder" + (folder.collapsed ? " collapsed" : ""));
  group.dataset.folderId = folder.id;
  const head = el("div", "folder-head");
  head.dataset.folderId = folder.id;
  const toggle = el("span", "folder-chevron");
  toggle.setAttribute("aria-hidden", "true");
  toggle.append(ic(folder.collapsed ? "chevron-right" : "chevron-down", 13));
  const glyph = el("span", "folder-glyph");
  glyph.append(ic("folder", 18));
  const label = el("span", "folder-name", folder.name);
  label.title = "Double-click to rename";
  label.ondblclick = (e) => { e.stopPropagation(); renameFolderInline(label, folder); };
  const count = el("span", "folder-count", String(members.length));
  const actions = el("span", "folder-actions");
  actions.append(
    ib("pencil", "Rename folder", () => renameFolderInline(label, folder)),
    ib("trash", "Delete folder", () => send({ cmd: "delete_folder", id: folder.id })),
  );
  head.append(toggle, glyph, label, count, actions);
  head.setAttribute("role", "button");
  head.tabIndex = 0;
  head.setAttribute("aria-expanded", String(!folder.collapsed));
  head.setAttribute("aria-label", (folder.collapsed ? "Expand " : "Collapse ") + folder.name);
  head.onclick = (e) => {
    if (e.target.closest(".folder-actions,.folder-rename") || noClick) return;
    send({ cmd: "toggle_folder_collapsed", id: folder.id });
  };
  head.onkeydown = (e) => {
    if ((e.key === "Enter" || e.key === " ") && !e.target.closest(".folder-actions,.folder-rename")) {
      e.preventDefault();
      send({ cmd: "toggle_folder_collapsed", id: folder.id });
    }
  };
  head.onpointerdown = (e) => watchFolderDrag(e, head, folder);
  group.append(head);
  const contents = el("div", "folder-contents");
  contents.append(...members.map(t => row(t, pinned ? "pin" : "tab")));
  group.append(contents);
  return group;
}
function folderArea(s, pinned) {
  const folders = (s.folders || []).filter(f => !!f.pinned === pinned);
  const allFolders = s.folders || [];
  const loose = (pinned ? s.pinned : s.tabs).filter(t => !allFolders.some(f => f.id === t.folder_id));
  const nodes = [...folders.map(f => folderNode(s, f, pinned)), ...loose.map(t => row(t, pinned ? "pin" : "tab"))];
  return nodes;
}
const settingsButton = ib("settings", "Settings", () => openView("settings"));
const spaces = el("div");
spaces.id = "dots";
const newSpaceButton = ib("plus", "New space", () => openView("space:add"));
$("foot").append(settingsButton, spaces, newSpaceButton);
$("hdr").onclick = () => openView("space:edit");

// Right-clicking the sidebar canvas creates a folder in-place.
$("pane").addEventListener("contextmenu", (e) => {
  if (e.target.closest(".row,.folder-actions,.folder-rename,input,button")) return;
  e.preventDefault();
  send({ cmd: "create_folder", name: "New folder" });
});

function row(t, kind) {
  const r = el(
    "div",
    "row" + (S.active === t.id ? " on" : "") + (t.loaded ? "" : " idle"),
  );
  r.dataset.id = t.id;
  r.append(icon(t), el("span", "ttl", t.title || pretty(t.url) || "New tab"));
  const a = el("span", "acts");
  a.append(
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
  $("pinned").replaceChildren(...folderArea(s, true));
  $("tabs").replaceChildren(...folderArea(s, false));
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
  const g = row.cloneNode(false); g.className = "dghost"; g.style.width = row.offsetWidth + "px";
  g.append(icon(t), el("span", "ttl", t.title || pretty(t.url) || "New tab")); document.body.append(g);
  drag = { id:t.id,kind,row,ghost:g,ox,zone:null,idx:0 }; row.classList.add("lift"); document.body.classList.add("dragging");
}
function watchFolderDrag(e, head, folder) {
  if(e.button!==0||e.target.closest("button,.folder-actions"))return;
  const x0=e.clientX,y0=e.clientY,ox=e.clientX-head.getBoundingClientRect().left; let on=false;
  const mv=(ev)=>{
    if(!on){if(Math.hypot(ev.clientX-x0,ev.clientY-y0)<5)return;on=true;
      const ghost=el("div","dghost folder-ghost");ghost.style.width=head.offsetWidth+"px";
      ghost.append(ic("folder",16),el("span","ttl",folder.name));document.body.append(ghost);
      drag={id:folder.id,kind:"folder",row:head,ghost,ox,zone:null,idx:0};head.classList.add("lift");document.body.classList.add("dragging");
    } updateDrag(ev);
  };
  const up=(ev)=>{removeEventListener("pointermove",mv);removeEventListener("pointerup",up);removeEventListener("pointercancel",up);if(on){noClick=true;setTimeout(()=>noClick=false,0);endDrag(ev.type==="pointerup");}};
  addEventListener("pointermove",mv);addEventListener("pointerup",up);addEventListener("pointercancel",up);
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
  else if (overRect($("pinned").getBoundingClientRect(), x, y, 8) || y < lr.top + lr.height / 2) zone = "pinned";
  else zone = "tab";
  let idx = 0, box = null;
  if (zone === "fav") {
    const tiles=[...$("favs").children].filter((c)=>c!==d.row);
    tiles.forEach((c)=>{const r=c.getBoundingClientRect();if(r.bottom<y||(y<=r.bottom&&y>=r.top&&r.left+r.width/2<x))idx++;});
    const ref=tiles[idx]||tiles[tiles.length-1],after=!tiles[idx];
    if(ref){const r=ref.getBoundingClientRect();box={left:after?r.right+2:r.left-4,top:r.top+4,width:2,height:r.height-8};}
  } else if(zone==="pinned") {
    const list=$("pinned");
    if(d.kind==="folder") {
      const groups=[...list.children].filter(c=>c.classList.contains("tab-folder")&&c!==d.row.parentElement);
      groups.forEach(c=>{const r=c.getBoundingClientRect();if(r.top+r.height/2<y)idx++;});
    } else {
      const rows=[...list.querySelectorAll(":scope > .row")].filter(c=>c!==d.row);
      rows.forEach(c=>{const r=c.getBoundingClientRect();if(r.top+r.height/2<y)idx++;});
    }
    const lb=list.getBoundingClientRect();box={left:lb.left+6,top:y,width:lb.width-12,height:2};
  } else if(zone==="tab") {
    if (d.kind === "folder") {
      const groups=[...$("tabs").children].filter(c=>c.classList.contains("tab-folder")&&c!==d.row.parentElement);
      groups.forEach(c=>{const r=c.getBoundingClientRect();if(r.top+r.height/2<y)idx++;});
    } else {
      const rows=[...$("tabs").children].filter(c=>c.classList.contains("row")&&c!==d.row);
      rows.forEach(c=>{const r=c.getBoundingClientRect();if(r.top+r.height/2<y)idx++;});
    }
    const lb=$("tabs").getBoundingClientRect();box={left:lb.left+6,top:y,width:lb.width-12,height:2};
  }
  const folderGroup = d.kind === "folder" ? null : document.elementFromPoint(x, y)?.closest(".tab-folder");
  const folderTarget = folderGroup?.querySelector(".folder-head");
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
    zone && zone.startsWith("folder:") ? "Add to folder" :
    d.kind === "folder" ? "Move folder" :
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
    if (d.kind === "folder") {
      if (d.zone === "pinned" || d.zone === "tab")
        send({ cmd: "move_folder", id: d.id, to: d.zone, index: d.idx });
    } else if (d.zone.startsWith("folder:")) send({cmd:"move_tab_to_folder",id:d.id,folder_id:d.zone.slice(7)});
    else { const to=d.zone==="pinned"?"pinned":d.zone==="fav"?"fav":"tab";send({cmd:"move_tab",id:d.id,to,index:d.idx}); }
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
