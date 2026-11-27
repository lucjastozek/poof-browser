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
/* Live folders: links that a feed or a GitHub search provides and the browser keeps fresh. */
/* "5 min ago" for a Unix time in seconds. */
function liveAgo(secs) {
  const minutes = Math.floor((Date.now() / 1000 - secs) / 60);
  if (minutes < 1) return "just now";
  if (minutes < 60) return minutes + " min ago";
  const hours = Math.floor(minutes / 60);
  return hours < 24 ? hours + " h ago" : Math.floor(hours / 24) + " d ago";
}
function liveStatus(live) {
  const what = live.source.kind === "github" ? "GitHub search" : "Feed";
  if (live.error) return what + " · " + live.error;
  return (
    what +
    (live.checked_at ? " · updated " + liveAgo(live.checked_at) : " · loading")
  );
}
/* A link of a live folder. Its full address is in the tooltip so the destination is visible
   before it is opened; opening always makes a new tab. */
function liveRow(item) {
  const r = el("div", "live-item");
  r.tabIndex = 0;
  r.setAttribute("role", "link");
  r.title = item.title + "\n" + item.url;
  const glyph = el("span", "live-glyph");
  glyph.append(ic("link", 13));
  r.append(glyph, el("span", "ttl", item.title));
  const open = () => go({ cmd: "new_tab", input: item.url });
  r.onclick = () => {
    if (!noClick) open();
  };
  r.onauxclick = (e) => {
    if (e.button === 1) open();
  };
  r.onkeydown = (e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      open();
    }
  };
  return r;
}
function liveRows(live) {
  const rows = live.items.map(liveRow);
  const note = live.error
    ? live.error
    : live.items.length
      ? ""
      : live.checked_at
        ? "Nothing here right now"
        : "Loading…";
  if (note)
    rows.push(el("div", "live-note" + (live.error ? " error" : ""), note));
  return rows;
}
function liveActions(folder) {
  return [
    ib("refresh-2", "Refresh now", () => {
      window.__toast("Refreshing " + folder.name + "…");
      send({ cmd: "refresh_live_folder", id: folder.id });
    }),
    ib("pencil", "Edit live folder", () => openView("livefolder:" + folder.id)),
  ];
}
// Flip the folder's look immediately; the native state that follows rebuilds it identically.
function toggleFolderNow(group, head, toggle, glyph, folder) {
  const collapsed = !group.classList.contains("collapsed");
  group.classList.toggle("collapsed", collapsed);
  head.setAttribute("aria-expanded", String(!collapsed));
  if (!folder.live)
    glyph.replaceChildren(ic(collapsed ? "folder-2" : "folder-open", 18));
  send({ cmd: "toggle_folder_collapsed", id: folder.id });
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
  toggle.append(ic(folder.collapsed ? "right" : "down", 13));
  const live = folder.live;
  const glyph = el("span", "folder-glyph");
  if (live) {
    glyph.append(
      ic(live.source.kind === "github" ? "git-pull-request" : "rss", 18),
    );
    glyph.title = liveStatus(live);
    head.dataset.live = "1";
  } else glyph.append(ic(folder.collapsed ? "folder-2" : "folder-open", 18));
  const label = el("span", "folder-name", folder.name);
  label.title = "Double-click to rename";
  label.ondblclick = (e) => {
    e.stopPropagation();
    renameFolderInline(label, folder);
  };
  const actions = el("span", "folder-actions");
  actions.append(
    ...(live
      ? liveActions(folder)
      : [
          ib("pencil", "Rename folder", () =>
            renameFolderInline(label, folder),
          ),
        ]),
    ib("delete-2", "Delete folder", () =>
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
    toggleFolderNow(group, head, toggle, glyph, folder);
  };
  head.onkeydown = (e) => {
    if (
      (e.key === "Enter" || e.key === " ") &&
      !e.target.closest(".folder-actions,.folder-rename")
    ) {
      e.preventDefault();
      toggleFolderNow(group, head, toggle, glyph, folder);
    }
  };
  head.onpointerdown = (e) => watchFolderDrag(e, head, folder);
  group.append(head);
  const contents = el("div", "folder-contents");
  // A collapsed folder's rows are never visible, so they are not built at all.
  if (!folder.collapsed)
    contents.append(
      ...(live
        ? liveRows(live)
        : members.map((t) => row(t, pinned ? "pin" : "tab"))),
    );
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
