// Page capture overlay: area, element or full page. Injected on demand by native code (capture.rs);
// the placeholders are filled in there. Everything lives in one shadow root so the page cannot restyle it.
(() => {
  if (window.__poofCapture) return window.__poofCapture.close();
  const NONCE = "__NONCE__";
  const ACCENT = "__ACCENT__";
  const host = document.createElement("div");
  host.style.cssText =
    "all:initial;position:fixed;inset:0;z-index:2147483647;pointer-events:none;";
  const root = host.attachShadow({ mode: "closed" });
  root.innerHTML = `<style>
    :host{all:initial}
    .shade{position:fixed;inset:0;cursor:crosshair;font:13px/1.4 system-ui,"Segoe UI",sans-serif}
    .box{position:fixed;border:2px solid ${ACCENT};border-radius:3px;box-shadow:0 0 0 100vmax rgba(0,0,0,.45);pointer-events:none;display:none}
    .shade.idle{background:rgba(0,0,0,.25)}
    .bar{position:fixed;top:14px;left:50%;transform:translateX(-50%);display:flex;align-items:center;gap:4px;padding:5px;background:#1d1b1a;color:#fffbe6;border-radius:12px;box-shadow:0 8px 28px rgba(0,0,0,.4);cursor:default;user-select:none}
    .bar button{display:flex;align-items:center;gap:6px;border:0;border-radius:8px;padding:7px 11px;background:none;color:inherit;font:inherit;cursor:pointer}
    .bar button:hover{background:rgba(255,251,230,.12)}
    .bar button.on{background:${ACCENT};color:#050505;font-weight:600}
    .bar i{width:1px;height:18px;background:rgba(255,251,230,.2);margin:0 3px}
    .bar .x{padding:7px 9px;opacity:.75}
    .hint{opacity:.7;padding:0 8px 0 4px;font-size:12px}
    .dim{position:fixed;display:none;padding:2px 6px;border-radius:5px;background:#1d1b1a;color:#fffbe6;font:11px system-ui;pointer-events:none}
  </style>
  <div class="shade idle"><div class="box"></div><div class="dim"></div>
    <div class="bar">
      <button data-m="area">Area</button><button data-m="element">Element</button><button data-m="full">Full page</button>
      <i></i><span class="hint"></span><button class="x" title="Cancel (Esc)">✕</button>
    </div></div>`;
  const $ = (s) => root.querySelector(s);
  const shade = $(".shade"),
    box = $(".box"),
    dim = $(".dim"),
    bar = $(".bar"),
    hint = $(".hint");
  const HINTS = {
    area: "Drag to select an area",
    element: "Click an element",
    full: "",
  };
  let mode = "area",
    start = null,
    target = null;

  const post = (o) => {
    try {
      window.ipc.postMessage(JSON.stringify({ t: "capture", n: NONCE, ...o }));
    } catch (e) {}
  };
  const frames = () =>
    new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
  const close = () => {
    removeEventListener("keydown", onKey, true);
    removeEventListener("scroll", onScrollResize, true);
    host.remove();
    delete window.__poofCapture;
  };
  const cancel = () => {
    post({ w: 0, h: 0 });
    close();
  };
  // Page coordinates (document origin) of a viewport rectangle.
  const send = async (r) => {
    const x = r.left + scrollX,
      y = r.top + scrollY;
    close();
    await frames();
    const p = (v) => String(v).padStart(2, "0"),
      n = new Date();
    const s = `${n.getFullYear()}-${p(n.getMonth() + 1)}-${p(n.getDate())} at ${p(n.getHours())}.${p(n.getMinutes())}.${p(n.getSeconds())}`;
    post({ x, y, w: r.width, h: r.height, d: devicePixelRatio || 1, s });
  };
  const place = (r) => {
    Object.assign(box.style, {
      display: "block",
      left: r.left + "px",
      top: r.top + "px",
      width: r.width + "px",
      height: r.height + "px",
    });
    dim.style.display = "block";
    dim.textContent = Math.round(r.width) + " × " + Math.round(r.height);
    dim.style.left = r.left + "px";
    dim.style.top = Math.max(0, r.top - 24) + "px";
  };
  const setMode = (m) => {
    mode = m;
    start = target = null;
    box.style.display = dim.style.display = "none";
    shade.classList.toggle("idle", m !== "area");
    shade.style.pointerEvents = m === "element" ? "none" : "auto";
    bar.style.pointerEvents = "auto";
    root
      .querySelectorAll("[data-m]")
      .forEach((b) => b.classList.toggle("on", b.dataset.m === m));
    hint.textContent = HINTS[m];
    if (m === "full") {
      const d = document.documentElement,
        b = document.body;
      const w = Math.max(d.scrollWidth, b ? b.scrollWidth : 0, innerWidth);
      const h = Math.max(d.scrollHeight, b ? b.scrollHeight : 0, innerHeight);
      send({ left: -scrollX, top: -scrollY, width: w, height: h });
    }
  };
  root
    .querySelectorAll("[data-m]")
    .forEach((b) => (b.onclick = () => setMode(b.dataset.m)));
  $(".x").onclick = cancel;

  shade.addEventListener("pointerdown", (e) => {
    if (mode !== "area" || (e.target !== shade && e.target !== box)) return;
    start = { x: e.clientX, y: e.clientY };
    shade.setPointerCapture(e.pointerId);
    e.preventDefault();
  });
  shade.addEventListener("pointermove", (e) => {
    if (!start) return;
    const x = Math.min(start.x, e.clientX),
      y = Math.min(start.y, e.clientY);
    place({
      left: x,
      top: y,
      width: Math.abs(e.clientX - start.x),
      height: Math.abs(e.clientY - start.y),
    });
  });
  shade.addEventListener("pointerup", (e) => {
    if (!start) return;
    const r = {
      left: Math.min(start.x, e.clientX),
      top: Math.min(start.y, e.clientY),
      width: Math.abs(e.clientX - start.x),
      height: Math.abs(e.clientY - start.y),
    };
    start = null;
    if (r.width < 4 || r.height < 4)
      return (
        place({ left: 0, top: 0, width: 0, height: 0 }),
        (box.style.display = dim.style.display = "none")
      );
    send(r);
  });
  // Element mode: the shade lets the pointer through, so the page under it is hit-tested directly.
  const pick = (e) => {
    if (mode !== "element") return;
    const el = document.elementFromPoint(e.clientX, e.clientY);
    target =
      el && el !== document.documentElement && el !== document.body ? el : null;
    if (target) place(target.getBoundingClientRect());
    else box.style.display = dim.style.display = "none";
  };
  const swallow = (e) => {
    if (mode === "element" && !e.composedPath().includes(host))
      e.preventDefault(), e.stopPropagation();
  };
  addEventListener("mousemove", pick, true);
  addEventListener(
    "click",
    (e) => {
      if (mode !== "element" || e.composedPath().includes(host)) return;
      swallow(e);
      if (target) send(target.getBoundingClientRect());
    },
    true,
  );
  addEventListener("mousedown", swallow, true);
  addEventListener("mouseup", swallow, true);
  const onScrollResize = () => {
    if (mode === "element" && target) place(target.getBoundingClientRect());
  };
  addEventListener("scroll", onScrollResize, true);
  const onKey = (e) => {
    if (e.key === "Escape") e.preventDefault(), e.stopPropagation(), cancel();
  };
  addEventListener("keydown", onKey, true);

  window.__poofCapture = { close };
  document.documentElement.append(host);
  setMode("__MODE__");
})();
