// Page capture overlay: area, element or full page. Injected on demand by native code (capture.rs);
// the placeholders are filled in there. Everything lives in one shadow root so the page cannot restyle it.
(() => {
  if (window.__poofCapture) return window.__poofCapture.close();
  const NONCE = "__NONCE__";
  const ACCENT = "__ACCENT__";
  // Frame, destination and element padding; the last choice is remembered by native code.
  const opt = Object.assign({ dest: "save", pad: 0 }, __OPTS__);
  const PRESET = { phone: [390, 844], browser: [1280, 800] };
  const svg = (d) => `<svg viewBox="0 0 24 24" aria-hidden="true">${d}</svg>`;
  const ICON = {
    "d:rect": svg(
      '<rect x="4" y="4" width="16" height="16" rx="2" stroke-dasharray="3 3"/>',
    ),
    "d:ptr": svg('<path d="M5 4l5 15 2.5-6.5L19 10z"/>'),
    "d:page": svg(
      '<rect x="6" y="3" width="12" height="18" rx="2"/><path d="M9.5 8h5M9.5 12h5M9.5 16h3"/>',
    ),
    "d:phone": svg(
      '<rect x="7" y="2.5" width="10" height="19" rx="2.5"/><path d="M11 18.5h2"/>',
    ),
    "d:browser": svg(
      '<rect x="3" y="4.5" width="18" height="15" rx="2.5"/><path d="M3 9h18"/>',
    ),
    "d:save": svg('<path d="M12 4v11m0 0l-4-4m4 4l4-4M5 19h14"/>'),
    "d:copy": svg(
      '<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2"/>',
    ),
    "d:both": svg('<path d="M12 3l9 5-9 5-9-5z"/><path d="M3 13l9 5 9-5"/>'),
    "d:x": svg('<path d="M6 6l12 12M18 6L6 18"/>'),
  };
  const host = document.createElement("div");
  host.style.cssText =
    "all:initial;position:fixed;inset:0;z-index:2147483647;pointer-events:none;";
  const root = host.attachShadow({ mode: "closed" });
  root.innerHTML = `<style>
    :host{all:initial}
    .shade{position:fixed;inset:0;cursor:crosshair;font:13px/1.4 system-ui,"Segoe UI",sans-serif}
    .box{position:fixed;border:2px solid ${ACCENT};border-radius:3px;box-shadow:0 0 0 100vmax rgba(0,0,0,.45);pointer-events:none;display:none}
    .shade.idle{background:rgba(0,0,0,.25)}
    .bar{position:fixed;top:14px;left:0;right:0;margin:0 auto;width:max-content;max-width:calc(100vw - 24px);box-sizing:border-box;display:flex;flex-wrap:nowrap;align-items:center;gap:6px;padding:5px;background:#1d1b1a;color:#fffbe6;border-radius:12px;box-shadow:0 8px 28px rgba(0,0,0,.4);cursor:default;user-select:none;white-space:nowrap}
    .grp{display:flex;align-items:center;gap:2px}
    .bar button{display:flex;align-items:center;gap:6px;height:30px;border:0;border-radius:8px;padding:0 10px;background:none;color:inherit;font:inherit;line-height:1;cursor:pointer}
    .bar button:hover{background:rgba(255,251,230,.12)}
    .bar button.on{background:${ACCENT};color:#050505;font-weight:600}
    .bar svg{width:16px;height:16px;flex:none;fill:none;stroke:currentColor;stroke-width:1.9;stroke-linecap:round;stroke-linejoin:round}
    .bar i{width:1px;height:18px;background:rgba(255,251,230,.2);flex:none}
    .bar .x{width:30px;padding:0;justify-content:center;opacity:.75}
    .lab{opacity:.6;padding:0 4px 0 2px;font-size:12px}
    .bar.noel .pad{display:none}
    .hint{position:fixed;top:64px;left:50%;transform:translateX(-50%);padding:5px 10px;border-radius:8px;background:#1d1b1a;color:#fffbe6;font-size:12px;white-space:nowrap;pointer-events:none;box-shadow:0 4px 14px rgba(0,0,0,.3)}
    .hint:empty{display:none}
    @media (max-width:860px){.bar .t{display:none}.bar button{padding:0 8px}}
    .dim{position:fixed;display:none;padding:2px 6px;border-radius:5px;background:#1d1b1a;color:#fffbe6;font:11px system-ui;pointer-events:none}
  </style>
  <div class="shade idle"><div class="box"></div><div class="dim"></div>
    <div class="bar">
      <div class="grp">${[
        ["area", "Area", "d:rect"],
        ["element", "Element", "d:ptr"],
        ["full", "Full page", "d:page"],
      ]
        .map(
          ([m, t, k]) =>
            `<button data-m="${m}" title="${t}">${ICON[k]}<span class="t">${t}</span></button>`,
        )
        .join("")}</div>
      <i></i>
      <div class="grp">${[
        ["phone", "Phone", "d:phone"],
        ["browser", "Browser", "d:browser"],
      ]
        .map(
          ([m, t, k]) =>
            `<button data-m="${m}" title="${t} frame, ${PRESET[m][0]} × ${PRESET[m][1]}">${ICON[k]}<span class="t">${t}</span></button>`,
        )
        .join("")}</div>
      <i></i>
      <div class="grp" data-g="dest">${[
        ["save", "Save", "d:save"],
        ["copy", "Copy", "d:copy"],
        ["both", "Both", "d:both"],
      ]
        .map(
          ([v, t, k]) =>
            `<button data-v="${v}" title="${t === "Both" ? "Save and copy" : t}">${ICON[k]}<span class="t">${t}</span></button>`,
        )
        .join("")}</div>
      <i class="pad"></i>
      <div class="grp pad" data-g="pad"><span class="lab">Padding</span><button data-v="0">0</button><button data-v="16">16</button><button data-v="32">32</button></div>
      <i></i>
      <button class="x" title="Cancel (Esc)">${ICON["d:x"]}</button>
    </div><div class="hint"></div></div></div>`;
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
    phone: "Captures the page as a phone-sized screen",
    browser: "Captures the page as a desktop window",
  };
  const FRAMED = { phone: 1, browser: 1 };
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
  // An element with `opt.pad` pixels around it, kept inside the document.
  const grow = (r) => {
    const p = opt.pad,
      d = document.documentElement,
      b = document.body;
    const dw = Math.max(d.scrollWidth, b ? b.scrollWidth : 0, innerWidth);
    const dh = Math.max(d.scrollHeight, b ? b.scrollHeight : 0, innerHeight);
    const l = Math.max(-scrollX, r.left - p),
      t = Math.max(-scrollY, r.top - p);
    const right = Math.min(dw - scrollX, r.left + r.width + p),
      bottom = Math.min(dh - scrollY, r.top + r.height + p);
    return { left: l, top: t, width: right - l, height: bottom - t };
  };
  const send = async (r) => {
    if (mode === "element" && opt.pad) r = grow(r);
    const x = r.left + scrollX,
      y = r.top + scrollY;
    close();
    await frames();
    const p = (v) => String(v).padStart(2, "0"),
      n = new Date();
    const s = `${n.getFullYear()}-${p(n.getMonth() + 1)}-${p(n.getDate())} at ${p(n.getHours())}.${p(n.getMinutes())}.${p(n.getSeconds())}`;
    post({
      x,
      y,
      w: r.width,
      h: r.height,
      d: devicePixelRatio || 1,
      s,
      f: FRAMED[mode] ? mode : "none",
      dst: opt.dest,
      pad: opt.pad,
    });
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
    bar.classList.toggle("noel", m !== "element");
    root
      .querySelectorAll("[data-m]")
      .forEach((b) => b.classList.toggle("on", b.dataset.m === m));
    hint.textContent = HINTS[m];
    if (FRAMED[m]) {
      // A device frame uses a fixed screen size; native code resizes the page for the shot.
      send({ left: 0, top: 0, width: PRESET[m][0], height: PRESET[m][1] });
    }
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
  const markOpts = () =>
    root
      .querySelectorAll("[data-g] button")
      .forEach((b) =>
        b.classList.toggle(
          "on",
          String(opt[b.parentNode.dataset.g]) === b.dataset.v,
        ),
      );
  root.querySelectorAll("[data-g] button").forEach(
    (b) =>
      (b.onclick = () => {
        const g = b.parentNode.dataset.g;
        opt[g] = g === "pad" ? Number(b.dataset.v) : b.dataset.v;
        markOpts();
      }),
  );
  markOpts();

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
