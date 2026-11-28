/* ---------- static sidebar chrome ---------- */
/* Collapsing slides the sidebar out on a spring and only then tells the native side, which
   snaps the page view into its new place (the page is a native window; CSS cannot animate it).
   Expanding does the reverse: the page view snaps, the sidebar springs in with a little bounce. */
function setSidebarCollapsed(collapsed, animateIn) {
  document.body.classList.toggle("collapsed", collapsed);
  collapseBtn.title = collapsed ? "Expand sidebar" : "Collapse sidebar";
  collapseBtn.setAttribute("aria-label", collapseBtn.title);
  collapseBtn.replaceChildren(
    ic(collapsed ? "sidebar-left-open" : "sidebar-left-close"),
  );
  send({ cmd: "sidebar_collapsed", collapsed });
  if (!collapsed && animateIn && !reduceMotion.matches) {
    const side = $("side");
    spring({
      ...BOUNCE,
      from: -56,
      to: 0,
      eps: 0.2,
      onUpdate: (x) => (side.style.transform = x ? `translateX(${x}px)` : ""),
    });
  }
}
function slideOutSidebar(velocity) {
  const side = $("side");
  spring({
    ...CRITICAL,
    from: 0,
    to: -(side.offsetWidth + 14),
    vel: -Math.abs(velocity || 0) * 1000,
    eps: 0.5,
    onUpdate: (x) => (side.style.transform = `translateX(${x}px)`),
    onDone: () => {
      side.style.transform = "";
      setSidebarCollapsed(true);
    },
  });
}
const collapseBtn = ib("sidebar-left-close", "Collapse sidebar", () => {
  if (document.body.classList.contains("collapsed"))
    setSidebarCollapsed(false, true);
  else slideOutSidebar(0);
});
$("topNav").append(collapseBtn);

/* Drag the sidebar's right edge to resize it. The width follows the pointer on a stiff spring
   (so it glides rather than jitters), pulling past the limits is rubber-banded, and a flick
   to the left collapses the sidebar. In the peek overlay of a collapsed sidebar the same
   handle expands it: flick or pull to the right. The native content view is relaid out at most
   once per frame; only the final width is written to disk. */
const RESIZE_MIN = 180,
  RESIZE_MAX = 480,
  FLICK_COLLAPSE = 0.9, // px/ms to the left
  FLICK_EXPAND = 0.6; // px/ms to the right
const resizeHandle = $("side-resize");
let sidebarWidth = 280;
let resizingSidebar = false;
let resizePointerId = null;
let resizeFrame = 0;
let resizeSpring = null;
let resizeRaw = 280;
let resizeVel = 0; // px per ms, smoothed
let resizeLast = { x: 0, t: 0 };
let resizePeek = null; // { x0 } while pulling a collapsed sidebar open
const rubber = (over) => 56 * (1 - 1 / ((over * 0.5) / 56 + 1));
const rubberBand = (w) =>
  w < RESIZE_MIN
    ? RESIZE_MIN - rubber(RESIZE_MIN - w)
    : w > RESIZE_MAX
      ? RESIZE_MAX + rubber(w - RESIZE_MAX)
      : w;
/* `visual` may stray past the limits (the rubber band); the native width never does. */
function applySidebarWidth(width, persist = false, visual = width) {
  sidebarWidth = Math.max(RESIZE_MIN, Math.min(RESIZE_MAX, width));
  document.documentElement.style.setProperty("--sidebar-width", `${visual}px`);
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
function endResizeSpring() {
  resizingSidebar = false;
  resizeSpring = null;
  document.body.classList.remove("resizing-sidebar");
  applySidebarWidth(sidebarWidth, true);
}
resizeHandle.addEventListener("pointerdown", (e) => {
  const peek = document.body.classList.contains("peek");
  if (
    e.button !== 0 ||
    document.body.classList.contains("compact") ||
    (document.body.classList.contains("collapsed") && !peek)
  )
    return;
  e.preventDefault();
  if (resizeSpring) resizeSpring.stop(); // grabbed again while it was still settling
  resizingSidebar = true;
  resizePointerId = e.pointerId;
  resizeVel = 0;
  resizeLast = { x: e.clientX, t: e.timeStamp };
  resizeRaw = e.clientX;
  resizePeek = peek ? { x0: e.clientX } : null;
  resizeHandle.setPointerCapture(e.pointerId);
  if (peek) return;
  document.body.classList.add("resizing-sidebar");
  resizeSpring = spring({
    k: 1100,
    c: 52,
    eps: 0.2,
    from: sidebarWidth,
    to: rubberBand(e.clientX),
    onUpdate: (x) => applySidebarWidth(x, false, x),
  });
});
resizeHandle.addEventListener("pointermove", (e) => {
  if (!resizingSidebar || e.pointerId !== resizePointerId) return;
  const dt = Math.max(1, e.timeStamp - resizeLast.t);
  resizeVel = resizeVel * 0.6 + ((e.clientX - resizeLast.x) / dt) * 0.4;
  resizeLast = { x: e.clientX, t: e.timeStamp };
  resizeRaw = e.clientX;
  if (resizeSpring) resizeSpring.go(rubberBand(resizeRaw));
});
function finishSidebarResize(e) {
  if (!resizingSidebar || (e && e.pointerId !== resizePointerId)) return;
  resizePointerId = null;
  // A pointer that stopped before lifting has no flick left in it.
  const v = e && e.timeStamp - resizeLast.t > 90 ? 0 : resizeVel;
  const cancelled = e && e.type === "pointercancel";
  if (resizePeek) {
    const pulled = resizeRaw - resizePeek.x0;
    resizePeek = null;
    resizingSidebar = false;
    if (!cancelled && (v > FLICK_EXPAND || pulled > 40))
      setSidebarCollapsed(false, true);
    return;
  }
  if (!cancelled && (v < -FLICK_COLLAPSE || resizeRaw < RESIZE_MIN * 0.65)) {
    // Flicked (or dragged) shut: snap back to the old width underneath while sliding out.
    resizeSpring.stop();
    const from = sidebarWidth;
    document.body.classList.remove("resizing-sidebar");
    resizingSidebar = false;
    resizeSpring = null;
    applySidebarWidth(from, true);
    slideOutSidebar(Math.abs(v));
    return;
  }
  // Settle on the nearest allowed width, bouncing if it was pulled past a limit.
  Object.assign(resizeSpring, BOUNCE, { eps: 0.2 });
  resizeSpring
    .go(Math.max(RESIZE_MIN, Math.min(RESIZE_MAX, resizeRaw)))
    .then(endResizeSpring);
}
resizeHandle.addEventListener("pointerup", finishSidebarResize);
resizeHandle.addEventListener("pointercancel", finishSidebarResize);
resizeHandle.addEventListener("keydown", (e) => {
  if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
  e.preventDefault();
  applySidebarWidth(sidebarWidth + (e.key === "ArrowRight" ? 12 : -12), true);
});

$("pageNav").append(
  ib("arrow-left", "Back", () => send({ cmd: "back" })),
  ib("arrow-right", "Forward", () => send({ cmd: "forward" })),
  ib("refresh-1", "Reload", () => send({ cmd: "reload" })),
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
$("pageToolbar").append(ib("plugin-2", "Extensions", () => openView("ext")));
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
  w.append(ic("add", 15));
  $("newtab").append(w, el("span", "ttl", "New tab"));
}
$("newtab").onclick = () => openView("newtab");
/* Ctrl+T: the same start page with the search bar as the New tab button. */
window.__openNewTab = () => {
  if (ov !== "newtab") openView("newtab");
  else if (window.__focusNewTab) window.__focusNewTab();
};
