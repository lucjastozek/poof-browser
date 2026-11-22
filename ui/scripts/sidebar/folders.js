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
