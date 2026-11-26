// Two-finger horizontal swipe over the page goes back / forward, like Chrome and Arc.
// A touchpad swipe is a stream of wheel events (inertia included). The page always gets them
// first: a site that handles the wheel itself (maps, Figma, carousels: it calls preventDefault)
// or content that can still scroll sideways keeps the gesture. Main frame only; idle pages
// run nothing but the wheel listener.
(function () {
  if (window.top !== window) return;
  var GAP_MS = 220; // silence that ends a gesture
  var DISTANCE = 130; // px of horizontal travel that commits
  var DECIDE = 12; // px of travel before the direction is judged
  var CHIP = 36; // px, indicator diameter
  var g = null; // the gesture in progress
  var host = null,
    chip = null,
    arrow = null,
    hideTimer = 0;

  function canGo(dir) {
    try {
      var nav = window.navigation;
      if (nav && typeof nav.canGoBack === "boolean")
        return dir < 0 ? nav.canGoBack : nav.canGoForward;
    } catch (e) {}
    return true;
  }

  // True when something under the pointer can still scroll sideways in that direction.
  function scrollsSideways(node, dir) {
    for (var n = node; n && n.nodeType === 1; n = n.parentElement) {
      var room = dir < 0 ? n.scrollLeft : n.scrollWidth - n.clientWidth - n.scrollLeft;
      if (room <= 1) continue;
      var overflow = getComputedStyle(n).overflowX;
      if (overflow === "auto" || overflow === "scroll" || n === document.scrollingElement)
        return true;
    }
    return false;
  }

  function build() {
    host = document.createElement("div");
    host.style.cssText =
      "position:fixed!important;top:0!important;left:0!important;width:100%!important;" +
      "height:100%!important;z-index:2147483647!important;pointer-events:none!important;" +
      "display:block!important;margin:0!important;padding:0!important;border:0!important;" +
      "background:none!important;";
    var root = host.attachShadow({ mode: "closed" });
    chip = document.createElement("div");
    chip.style.cssText =
      "position:absolute;top:50%;width:" + CHIP + "px;height:" + CHIP + "px;margin-top:-" +
      CHIP / 2 + "px;border-radius:50%;background:rgba(32,32,32,.88);color:#fff;" +
      "box-shadow:0 2px 10px rgba(0,0,0,.35);display:flex;align-items:center;" +
      "justify-content:center;opacity:0;transition:opacity .12s,transform .1s";
    var ns = "http://www.w3.org/2000/svg";
    arrow = document.createElementNS(ns, "svg");
    arrow.setAttribute("viewBox", "0 0 24 24");
    arrow.setAttribute("width", "18");
    arrow.setAttribute("height", "18");
    var path = document.createElementNS(ns, "path");
    path.setAttribute("d", "M15 5l-7 7 7 7");
    path.setAttribute("fill", "none");
    path.setAttribute("stroke", "currentColor");
    path.setAttribute("stroke-width", "2.4");
    path.setAttribute("stroke-linecap", "round");
    path.setAttribute("stroke-linejoin", "round");
    arrow.appendChild(path);
    chip.appendChild(arrow);
    root.appendChild(chip);
  }

  function show(dir, progress) {
    if (!host) build();
    if (!host.isConnected) document.documentElement.appendChild(host);
    chip.style.left = dir < 0 ? "0" : "auto";
    chip.style.right = dir < 0 ? "auto" : "0";
    var slide = (progress - 1) * (CHIP + 6) + 14; // from off-screen to 14px in
    chip.style.transform =
      "translateX(" + (dir < 0 ? slide : -slide) + "px) scale(" + (0.8 + 0.2 * progress) + ")";
    arrow.style.transform = dir < 0 ? "none" : "scaleX(-1)";
    chip.style.opacity = String(Math.min(1, progress * 1.4));
    chip.style.background = progress >= 1 ? "rgba(32,32,32,.96)" : "rgba(32,32,32,.88)";
  }

  function hide() {
    clearTimeout(hideTimer);
    hideTimer = 0;
    if (chip) chip.style.opacity = "0";
  }

  // Runs after the page's own wheel handlers, so defaultPrevented is final.
  function judge(e) {
    if (!g || e.defaultPrevented) {
      if (g) g.ok = false;
      hide();
      return;
    }
    if (g.ok === false || g.done) return;
    var ax = Math.abs(g.dx),
      ay = Math.abs(g.dy);
    if (g.ok === null) {
      if (ax < DECIDE && ay < DECIDE) return;
      var dir = g.dx < 0 ? -1 : 1;
      g.ok = ax > ay * 1.5 && canGo(dir) && !scrollsSideways(g.target, dir);
      g.dir = dir;
      if (!g.ok) return;
    }
    // The swipe turned vertical or reversed: it was a scroll after all.
    if (ay > ax || (g.dx < 0 ? -1 : 1) !== g.dir) {
      g.ok = false;
      hide();
      return;
    }
    var progress = Math.min(1, ax / DISTANCE);
    show(g.dir, progress);
    clearTimeout(hideTimer);
    hideTimer = setTimeout(hide, GAP_MS);
    if (progress >= 1) {
      g.done = true;
      if (g.dir < 0) history.back();
      else history.forward();
    }
  }

  addEventListener(
    "wheel",
    function (e) {
      try {
        if (e.ctrlKey || e.deltaMode !== 0) return;
        if (!g || e.timeStamp - g.last > GAP_MS) {
          g = { dx: 0, dy: 0, last: 0, ok: null, done: false, dir: 0, target: e.target };
        }
        g.last = e.timeStamp;
        g.dx += e.deltaX;
        g.dy += e.deltaY;
        if (g.done) return;
        setTimeout(function () {
          try {
            judge(e);
          } catch (err) {}
        }, 0);
      } catch (err) {}
    },
    { capture: true, passive: true },
  );
})();
