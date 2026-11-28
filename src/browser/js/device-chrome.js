(cfg) => {
  let ui = window.__poofDeviceUi;
  if (!cfg) {
    if (ui) ui.root.style.display = "none";
    return;
  }
  const PRESETS = [
    ["Small", 320, 568],
    ["Phone", 390, 844],
    ["Tablet", 820, 1180],
    ["Laptop", 1280, 800],
    ["Desktop", 1920, 1080],
  ];
  const post = (m) => window.ipc.postMessage(JSON.stringify(m));
  const size = (w, h, persist) => post({ cmd: "device_size", w, h, persist });
  const mk = (tag, cls, text) => {
    const n = document.createElement(tag);
    if (cls) n.className = cls;
    if (text) n.textContent = text;
    return n;
  };
  const chip = (label, tip, fn) => {
    const b = mk("button", "dev-chip", label);
    b.type = "button";
    b.title = tip;
    b.setAttribute("aria-label", tip);
    b.onclick = fn;
    return b;
  };

  if (!ui) {
    const root = mk("div", "dev-root");
    const bar = mk("div", "dev-bar");
    const presets = mk("div", "dev-presets");
    const chips = PRESETS.map(([name, w, h]) => {
      const c = chip(name, `${name} ${w} × ${h}`, () => size(w, h, true));
      c.dataset.w = w;
      c.dataset.h = h;
      presets.append(c);
      return c;
    });
    const readout = mk("span", "dev-size");
    readout.setAttribute("aria-live", "polite");
    const rotate = chip("Rotate", "Rotate (swap width and height)", () => {
      const [w, h] = ui.cfg.size;
      size(h, w, true);
    });
    const tools = chip(
      "DevTools",
      "Page + DevTools docked right (Ctrl+Shift+D)",
      () => post({ cmd: "device_layout", kind: "tools" }),
    );
    const widths = chip("2 widths", "Page at two widths (Ctrl+Shift+U)", () =>
      post({ cmd: "device_layout", kind: "widths" }),
    );
    const fit = chip("Fit", "Fill the available space", () =>
      size(1e5, 1e5, true),
    );
    const close = chip("✕", "Exit device mode (Ctrl+Shift+M)", () =>
      post({ cmd: "device_toggle" }),
    );
    close.classList.add("dev-close");
    const sep = () => mk("i", "dev-sep");
    bar.append(
      presets,
      sep(),
      readout,
      rotate,
      fit,
      sep(),
      tools,
      widths,
      close,
    );

    let drag = null,
      raf = 0,
      last = null;
    // Each handle changes one or both sides. The page is centred, so the width grows twice as
    // fast as the pointer moves.
    const grip = (kind, label, cursor, keys) => {
      const h = mk("div", `dev-handle dev-${kind}`);
      h.tabIndex = 0;
      h.setAttribute("role", "separator");
      h.setAttribute("aria-label", label);
      h.style.cursor = cursor;
      h.append(mk("i"));
      h.addEventListener("pointerdown", (e) => {
        if (e.button !== 0) return;
        const [, , fw, fh] = ui.cfg.frame;
        drag = {
          id: e.pointerId,
          x: e.clientX,
          y: e.clientY,
          w: fw,
          h: fh,
          kind,
        };
        h.setPointerCapture(e.pointerId);
        h.classList.add("on");
        e.preventDefault();
      });
      h.addEventListener("pointermove", (e) => {
        if (!drag || drag.id !== e.pointerId) return;
        last = [
          kind === "bottom" ? drag.w : drag.w + 2 * (e.clientX - drag.x),
          kind === "right" ? drag.h : drag.h + (e.clientY - drag.y),
        ];
        if (!raf)
          raf = requestAnimationFrame(() => {
            raf = 0;
            size(last[0], last[1], false);
          });
      });
      const end = (e) => {
        if (!drag || drag.id !== e.pointerId) return;
        if (raf) cancelAnimationFrame(raf), (raf = 0);
        if (last) size(last[0], last[1], true);
        drag = last = null;
        h.classList.remove("on");
      };
      h.addEventListener("pointerup", end);
      h.addEventListener("pointercancel", end);
      h.addEventListener("keydown", (e) => {
        const step = (e.shiftKey ? 50 : 10) * (keys[e.key] || 0);
        if (!step) return;
        const [w, h2] = ui.cfg.size;
        size(
          kind === "bottom" ? w : w + step,
          kind === "right" ? h2 : h2 + step,
          true,
        );
        e.preventDefault();
      });
      return h;
    };
    const right = grip("right", "Page width", "ew-resize", {
      ArrowRight: 1,
      ArrowLeft: -1,
    });
    const bottom = grip("bottom", "Page height", "ns-resize", {
      ArrowDown: 1,
      ArrowUp: -1,
    });
    const corner = grip("corner", "Page size", "nwse-resize", {
      ArrowRight: 1,
      ArrowLeft: -1,
    });
    root.append(bar, right, bottom, corner);
    document.body.append(root);
    ui = window.__poofDeviceUi = {
      cfg,
      root,
      bar,
      right,
      bottom,
      corner,
      chips,
      readout,
      tools,
    };
  }

  ui.cfg = cfg;
  const [ax, ay, aw] = cfg.area;
  const [fx, fy, fw, fh] = cfg.frame;
  const g = cfg.gutter;
  const place = (n, x, y, w, h) =>
    Object.assign(n.style, {
      left: x + "px",
      top: y + "px",
      width: w + "px",
      height: h + "px",
    });
  ui.root.style.display = "block";
  place(ui.bar, ax, ay, aw, cfg.bar - 6);
  place(ui.right, fx + fw, fy, g, fh);
  place(ui.bottom, fx, fy + fh, fw, g);
  place(ui.corner, fx + fw, fy + fh, g, g);
  const [w, h] = cfg.size;
  const shown = [Math.round(fw), Math.round(fh)];
  ui.readout.textContent = `${shown[0]} × ${shown[1]}`;
  ui.readout.title =
    shown[0] === w && shown[1] === h
      ? ""
      : `Saved size ${w} × ${h} (window too small)`;
  ui.chips.forEach((c) =>
    c.classList.toggle("on", +c.dataset.w === w && +c.dataset.h === h),
  );
  ui.tools.classList.toggle("on", !!cfg.tools);
  ui.tools.setAttribute("aria-pressed", String(!!cfg.tools));
};
