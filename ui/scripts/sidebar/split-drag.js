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
