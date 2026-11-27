/* ---------- static sidebar chrome ---------- */
const collapseBtn = ib("panelLeftClose", "Collapse sidebar", () => {
  const collapsed = !document.body.classList.contains("collapsed");
  document.body.classList.toggle("collapsed", collapsed);
  collapseBtn.title = collapsed ? "Expand sidebar" : "Collapse sidebar";
  collapseBtn.setAttribute("aria-label", collapseBtn.title);
  collapseBtn.replaceChildren(
    ic(collapsed ? "panelLeftOpen" : "panelLeftClose"),
  );
  send({ cmd: "sidebar_collapsed", collapsed });
});
$("topNav").append(collapseBtn);

// Drag the sidebar's right edge to resize it. The native content view is
// relaid out continuously (at most once per frame); only the final width is
// written to disk.
const resizeHandle = $("side-resize");
let sidebarWidth = 280;
let resizingSidebar = false;
let resizePointerId = null;
let resizeFrame = 0;
function applySidebarWidth(width, persist = false) {
  sidebarWidth = Math.max(180, Math.min(480, width));
  document.documentElement.style.setProperty(
    "--sidebar-width",
    `${sidebarWidth}px`,
  );
  if (persist) {
    if (resizeFrame) {
      cancelAnimationFrame(resizeFrame);
      resizeFrame = 0;
    }
    send({ cmd: "sidebar_width", width: sidebarWidth, persist: true });
    return;
  }
  if (!resizeFrame) {
    resizeFrame = requestAnimationFrame(() => {
      resizeFrame = 0;
      send({ cmd: "sidebar_width", width: sidebarWidth, persist: false });
    });
  }
}
resizeHandle.addEventListener("pointerdown", (e) => {
  if (
    e.button !== 0 ||
    document.body.classList.contains("compact") ||
    document.body.classList.contains("collapsed")
  )
    return;
  e.preventDefault();
  resizingSidebar = true;
  resizePointerId = e.pointerId;
  resizeHandle.setPointerCapture(e.pointerId);
  document.body.classList.add("resizing-sidebar");
});
resizeHandle.addEventListener("pointermove", (e) => {
  if (!resizingSidebar || e.pointerId !== resizePointerId) return;
  applySidebarWidth(e.clientX);
});
function finishSidebarResize(e) {
  if (!resizingSidebar || (e && e.pointerId !== resizePointerId)) return;
  resizingSidebar = false;
  document.body.classList.remove("resizing-sidebar");
  resizePointerId = null;
  applySidebarWidth(sidebarWidth, true);
}
resizeHandle.addEventListener("pointerup", finishSidebarResize);
resizeHandle.addEventListener("pointercancel", finishSidebarResize);
resizeHandle.addEventListener("keydown", (e) => {
  if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
  e.preventDefault();
  applySidebarWidth(sidebarWidth + (e.key === "ArrowRight" ? 12 : -12), true);
});

$("pageNav").append(
  ib("back", "Back", () => send({ cmd: "back" })),
  ib("fwd", "Forward", () => send({ cmd: "forward" })),
  ib("reload", "Reload", () => send({ cmd: "reload" })),
);
$("copyLink").append(ic("link", 15));
// Copies text and says so. Shortcut copies come from native code (a page had focus), which
// passes the tab to give focus back to once the clipboard is written.
async function copyLink(value, message, returnTo) {
  try {
    await navigator.clipboard.writeText(value);
  } catch (_) {
    const tmp = el("textarea");
    tmp.value = value;
    tmp.style.position = "fixed";
    tmp.style.opacity = "0";
    document.body.append(tmp);
    tmp.select();
    try {
      document.execCommand("copy");
    } finally {
      tmp.remove();
    }
  }
  window.__toast(message);
  if (returnTo) send({ cmd: "focus_tab", id: returnTo });
}
window.__copyLink = copyLink;
$("copyLink").onclick = () => {
  if (S && S.url) copyLink(S.url, "Link copied");
};
$("pageToolbar").append(ib("puzzle", "Extensions", () => openView("ext")));
ICONS.camera = SPACE_ICONS.camera; // the toolbar set has no camera; reuse the one of the space icons
$("pageToolbar").append(
  ib("camera", "Capture (Ctrl+Shift+S)", () => send({ cmd: "capture" })),
);
$("pageToolbar").append(
  ib("code", "Developer tools (F12)", () => send({ cmd: "devtools_toggle" })),
);
$("winMin").onclick = () => send({ cmd: "window_minimize" });
$("winMax").onclick = () => send({ cmd: "window_toggle_maximize" });
$("winClose").onclick = () => send({ cmd: "window_close" });
$("titlebar").addEventListener("dblclick", (e) => {
  if (e.target.closest(".window-controls")) return;
  send({ cmd: "window_toggle_maximize" });
});
$("window-drag").addEventListener("pointerdown", (e) => {
  if (e.button === 0) send({ cmd: "window_drag" });
});
// Backup: drag from any empty part of the topbar, in case #window-drag is covered or collapsed.
$("titlebar").addEventListener("pointerdown", (e) => {
  if (e.button !== 0 || e.target.closest("#window-drag")) return;
  if (
    e.target.closest(
      "button,input,textarea,select,a,[role=button],.window-controls",
    )
  )
    return;
  send({ cmd: "window_drag" });
});
{
  const w = el("span", "ico");
  w.append(ic("plus", 15));
  $("newtab").append(w, el("span", "ttl", "New tab"));
}
$("newtab").onclick = () => openView("newtab");
/* Ctrl+T: the same start page with the search bar as the New tab button. */
window.__openNewTab = () => {
  if (ov !== "newtab") openView("newtab");
  else if (window.__focusNewTab) window.__focusNewTab();
};
