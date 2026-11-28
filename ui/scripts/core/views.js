/* Where focus was before a full-area view opened, so closing it puts focus back. */
let viewReturn = null;
function saveViewFocus() {
  const a = document.activeElement;
  viewReturn =
    a && a !== document.body && !$("stage").contains(a)
      ? { el: a, key: window.__navKeyOf ? window.__navKeyOf(a) : null }
      : null;
}
function restoreViewFocus() {
  const r = viewReturn;
  viewReturn = null;
  if (r && r.key && window.__focusNavKey && window.__focusNavKey(r.key)) return;
  if (r && r.el && r.el.isConnected) r.el.focus({ preventScroll: true });
  else if (S && S.active) send({ cmd: "focus_tab", id: S.active });
}
/* A command that changes what is shown: close any full-area view first. */
function go(cmd) {
  if (ov) {
    viewReturn = null; // the page takes focus now
    ov = null;
    leaveView();
    $("stage").hidden = true;
    send({ cmd: "overlay", open: false });
  }
  send(cmd);
}
/* Shared cleanup when a full-area view closes or another opens. */
function leaveView() {
  window.__trimSpaceIcons();
  if (window.__stopShortcutCapture) window.__stopShortcutCapture();
  const stage = $("stage");
  stage.classList.remove("has-backdrop");
  stage.style.removeProperty("--backdrop");
}
function openView(name) {
  if (window.__closeDownloads) window.__closeDownloads();
  // Settings float over a picture of the page that was showing; other views replace it.
  const snapshot = name === "settings" && !ov;
  if (!ov) saveViewFocus();
  leaveView();
  ov = name;
  $("stage").hidden = false;
  send({ cmd: "overlay", open: true, snapshot });
  build(name);
}
/* The browser captured the page: show it, then let the browser hide the real page. */
window.__pageBackdrop = (url) => {
  const done = () =>
    requestAnimationFrame(() => send({ cmd: "backdrop_ready" }));
  const img = new Image();
  img.src = url;
  img
    .decode()
    .then(() => {
      if (ov === "settings") {
        const stage = $("stage");
        stage.style.setProperty("--backdrop", "url(" + url + ")");
        stage.classList.add("has-backdrop");
      }
    })
    .catch(() => {})
    .then(() => requestAnimationFrame(done));
};
function closeView() {
  applyColor(curSpace().color);
  if (!S || !S.active) {
    openView("newtab");
    return;
  }
  ov = null;
  leaveView();
  $("stage").hidden = true;
  applyColor(curSpace().color);
  send({ cmd: "overlay", open: false });
  restoreViewFocus();
}
function build(name) {
  const v = $("stage");
  if (name === "newtab") buildNewTab(v);
  else if (name === "ext") buildExt(v);
  else if (name === "settings") buildSettings(v);
  else if (name.startsWith("space")) buildSpace(v, name.split(":")[1]);
}
