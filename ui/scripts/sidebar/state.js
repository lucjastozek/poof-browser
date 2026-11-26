// One favorite tile; also used by the swipe carousel to draw other spaces.
function favTile(s, t) {
  const d = el(
    "div",
    "tile" + (s.active === t.id ? " on" : "") + (t.loaded ? "" : " idle"),
  );
  d.dataset.id = t.id;
  d.title = (t.title || t.url) + "\nRight-click to remove";
  d.append(icon(t));
  d.onclick = () => {
    if (noClick) return;
    go({ cmd: "select_tab", id: t.id });
  };
  d.onpointerdown = (e) => watchDrag(e, d, t, "fav");
  d.oncontextmenu = (e) => {
    e.preventDefault();
    send({ cmd: "unfav_tab", id: t.id });
  };
  return d;
}

window.__setState = (s) => {
  if (drag) {
    pending = s;
    return;
  }
  // The native side already drops identical states, so no JSON.stringify of the whole
  // snapshot (favicons included) is needed here. `lastKey` is kept only because the
  // drag code resets it.
  lastKey = "";
  S = s;
  if (Number.isFinite(s.sidebar_width) && !resizingSidebar) {
    sidebarWidth = Math.max(180, Math.min(480, s.sidebar_width));
    document.documentElement.style.setProperty(
      "--sidebar-width",
      `${sidebarWidth}px`,
    );
  }
  document.body.classList.toggle("has-page", !!s.active || !!s.switching);
  document.body.classList.toggle("collapsed", !!s.sidebar_collapsed);
  document.body.classList.toggle(
    "peek",
    !!s.sidebar_peek && !!s.sidebar_collapsed,
  );
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
  $("favs").replaceChildren(...s.favorites.map((t) => favTile(s, t)));
  $("favs").classList.toggle("empty", !s.favorites.length);
  $("pinned").replaceChildren(...folderArea(s, true));
  $("tabs").replaceChildren(...folderArea(s, false));
  clearTabsBtn.hidden = !s.tabs.some((t) => !t.folder_id);
  const dots = [];
  let lastP = null;
  s.spaces.forEach((x) => {
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
  // A swipe already slid the new space in; the carousel hands over to this real content.
  if (prevIdx >= 0 && idx !== prevIdx && !sw) {
    const pane = $("pane");
    pane.classList.remove("l", "r");
    void pane.offsetWidth;
    pane.classList.add(idx > prevIdx ? "l" : "r");
  }
  prevIdx = idx;
  $("addExt").hidden = !s.store_ext;
  syncUrl();
  ensureSpaceIcons(s);
  if (sw) swipeOnState();
  if (!s.active && !ov && !s.switching)
    openView("newtab"); /* no page open: show the start page */
};
$("addExt").onclick = () => send({ cmd: "install_ext", input: S.store_ext });
