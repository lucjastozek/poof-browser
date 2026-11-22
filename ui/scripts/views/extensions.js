/* ---------- extensions ---------- */
window.__setExts = (l) => {
  EXTS = l;
  if (ov === "ext") paintExts();
};
function paintExts() {
  const box = $("extlist");
  if (!box) return;
  if (!EXTS.length) {
    const e = el("div", "empty-st");
    e.append(ic("puzzle", 26), el("span", null, "No extensions yet"));
    box.replaceChildren(e);
    return;
  }
  box.replaceChildren(
    ...EXTS.map((x) => {
      const r = el("div", "ext");
      r.append(icon({ favicon: x.icon, title: x.name }));
      const m = el("div", "meta");
      m.append(
        el("b", null, x.name),
        el("small", null, "Version " + x.version),
      );
      r.append(m);
      r.append(
        btn("sm ghost danger", "Remove", () =>
          send({ cmd: "remove_ext", dir: x.dir }),
        ),
      );
      return r;
    }),
  );
}
function buildExt(v) {
  const id = el("input", "inp");
  id.placeholder = "Chrome Web Store link or extension ID";
  id.setAttribute("aria-label", "Chrome Web Store link or extension ID");
  const add = btn("primary", "Install", () => {
    if (id.value.trim()) {
      send({ cmd: "install_ext", input: id.value });
      id.value = "";
    }
  });
  id.addEventListener("keydown", (e) => {
    if (e.key === "Enter") add.click();
  });
  const inst = el("div", "row2");
  inst.append(id, add);
  const browse = btn("sm ghost", "Browse the Chrome Web Store", () =>
    go({ cmd: "new_tab", input: "https://chromewebstore.google.com/" }),
  );
  browse.prepend(ic("external", 14));
  const bw = el("div", "row2");
  bw.append(browse);
  const list = el("div");
  list.id = "extlist";
  dialog(v, "Extensions", [
    field(null, inst, bw),
    list,
    el(
      "div",
      "note",
      "On any Web Store page an “Add to poof” button appears in the address bar. Extensions run in every profile. Toolbar icons and popups are not available in this engine (WebView2), but background logic and content scripts, like ad blocking, work.",
    ),
  ]);
  paintExts();
  send({ cmd: "list_ext" });
  setTimeout(() => id.focus(), 30);
}
