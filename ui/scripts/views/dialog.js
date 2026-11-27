/* ---------- dialog scaffold ---------- */
function dialog(v, title, body, foot) {
  const wrap = el("div", "dlg-wrap"),
    d = el("div", "dlg");
  d.setAttribute("role", "dialog");
  d.setAttribute("aria-label", title);
  const h = el("header");
  h.append(el("h2", null, title), ib("close", "Close", closeView));
  const b = el("div", "body");
  b.append(...body);
  d.append(h, b);
  if (foot) {
    const f = el("footer");
    f.append(...foot);
    d.append(f);
  }
  wrap.append(d);
  wrap.addEventListener("pointerdown", (e) => {
    if (e.target === wrap) closeView();
  });
  v.replaceChildren(wrap);
}
/* Escape closes any dialog-style view (the start page and shortcut capture handle their own). */
document.addEventListener("keydown", (e) => {
  if (e.key !== "Escape" || e.defaultPrevented || !ov || ov === "newtab")
    return;
  if (e.target.closest && e.target.closest(".folder-rename")) return;
  closeView();
});
const field = (l, ...c) => {
  const d = el("div", "fl");
  if (l) d.append(el("span", "label", l));
  d.append(...c);
  return d;
};
