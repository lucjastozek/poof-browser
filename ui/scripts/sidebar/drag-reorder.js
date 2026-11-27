/* ---------- drag to pin / unpin / favorite / reorder ---------- */
/* A touch must be held this long before moving drags a row; moving sooner scrolls the list. */
const TOUCH_HOLD_MS = 250;
/* Shared pointer tracking for rows and folders: `begin(offsetX)` creates the drag. */
function armDrag(e, node, begin) {
  const x0 = e.clientX,
    y0 = e.clientY,
    ox = e.clientX - node.getBoundingClientRect().left,
    t0 = Date.now(),
    touch = e.pointerType === "touch";
  let on = false;
  const block = (ev) => {
    if (on && ev.cancelable) ev.preventDefault();
  };
  const stop = () => {
    removeEventListener("pointermove", mv);
    removeEventListener("pointerup", up);
    removeEventListener("pointercancel", up);
    removeEventListener("touchmove", block);
  };
  const mv = (ev) => {
    if (!on) {
      const far = Math.hypot(ev.clientX - x0, ev.clientY - y0);
      if (touch && Date.now() - t0 < TOUCH_HOLD_MS) {
        if (far > 8) stop();
        return;
      }
      if (far < 5) return;
      on = true;
      begin(ox);
      // Rows are rebuilt while dragging; keep receiving this touch after that.
      try {
        document.body.setPointerCapture(ev.pointerId);
      } catch (_) {}
    }
    updateDrag(ev);
  };
  const up = (ev) => {
    stop();
    if (on) {
      noClick = true;
      setTimeout(() => (noClick = false), 0);
      endDrag(ev.type === "pointerup");
    }
  };
  addEventListener("pointermove", mv);
  addEventListener("pointerup", up);
  addEventListener("pointercancel", up);
  addEventListener("touchmove", block, { passive: false });
}
function watchDrag(e, row, t, kind) {
  if (e.button !== 0 || e.target.closest(".acts")) return;
  armDrag(e, row, (ox) => beginDrag(row, t, kind, ox));
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
  armDrag(e, head, (ox) => {
    const ghost = el("div", "dghost folder-ghost");
    ghost.style.width = head.offsetWidth + "px";
    ghost.append(ic("folder-2", 16), el("span", "ttl", folder.name));
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
  });
}
function overRect(r, x, y, pad = 0) {
  return (
    x >= r.left - pad &&
    x <= r.right + pad &&
    y >= r.top - pad &&
    y <= r.bottom + pad
  );
}
