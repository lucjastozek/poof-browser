(() => {
  if (window.__poofUV) return;
  const uv = (window.__poofUV = new WeakSet());
  const WINDOW_MS = 2500;
  const SCROLL_KEYS = new Set([
    "ArrowUp",
    "ArrowDown",
    "PageUp",
    "PageDown",
    "Home",
    "End",
  ]);
  const post = (o) => {
    try {
      window.ipc.postMessage(JSON.stringify(o));
    } catch (e) {}
  };
  let last = 0,
    told = false;
  const stamp = (e) => {
    if (!e.isTrusted) return;
    if (e.type === "keydown" && SCROLL_KEYS.has(e.key)) return;
    last = performance.now();
  };
  const opts = { capture: true, passive: true };
  for (const t of [
    "pointerdown",
    "mousedown",
    "touchstart",
    "keydown",
    "click",
  ])
    window.addEventListener(t, stamp, opts);
  // Calls (Meet, Teams, Zoom) start their video without a click and play a live MediaStream:
  // the user joined the call, so those videos count as user-started.
  const isLive = (v) =>
    typeof MediaStream !== "undefined" && v.srcObject instanceof MediaStream;
  const mark = (e) => {
    const v = e.target;
    if (!(v instanceof HTMLVideoElement)) return;
    if (!isLive(v) && performance.now() - last >= WINDOW_MS) return;
    uv.add(v);
    // Tell the host once, so it only wakes this page for auto-PIP when a video was started.
    if (!told) {
      told = true;
      post({ t: "vid" });
    }
  };
  for (const t of ["play", "playing", "volumechange"])
    document.addEventListener(t, mark, opts);
  // Lets the host keep a page that is showing a PIP window alive and untrimmed.
  document.addEventListener(
    "enterpictureinpicture",
    () => post({ t: "pip", active: true }),
    opts,
  );
  document.addEventListener(
    "leavepictureinpicture",
    () => post({ t: "pip", active: false }),
    opts,
  );
})();
