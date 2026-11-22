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
