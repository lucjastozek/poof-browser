/* A command that changes what is shown: close any full-area view first. */
function go(cmd) {
  if (ov) {
    ov = null;
    window.__trimSpaceIcons();
    $("stage").hidden = true;
    send({ cmd: "overlay", open: false });
  }
  send(cmd);
}
function openView(name) {
  ov = name;
  window.__trimSpaceIcons();
  $("stage").hidden = false;
  send({ cmd: "overlay", open: true });
  build(name);
}
function closeView() {
  applyColor(curSpace().color);
  if (!S || !S.active) {
    openView("newtab");
    return;
  }
  ov = null;
  $("stage").hidden = true;
  applyColor(curSpace().color);
  send({ cmd: "overlay", open: false });
  window.__trimSpaceIcons();
}
