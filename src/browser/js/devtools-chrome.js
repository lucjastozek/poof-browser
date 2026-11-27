(cfg) => {
  let ui = window.__poofDevtoolsUi;
  if (!cfg) {
    if (ui) ui.handle.style.display = ui.header.style.display = "none";
    return;
  }
  if (!ui) {
    const post = (msg) => window.ipc.postMessage(JSON.stringify(msg));
    ui = window.__poofDevtoolsUi = { cfg };

    // Rasterize the browser's own icon set (ic) into mask-ready data URIs, once, and hand
    // them to the native side, which passes them on to the DevTools frontend.
    const sendIcons = () => {
      if (typeof ic !== "function") return;
      // DevTools icon name -> MingCute icon (embedded by tools/icons.rs).
      const MAP = {
        cross: "close",
        "cross-circle": "close",
        "cross-circle-filled": "close",
        close: "close",
        plus: "add",
        bin: "delete-2",
        clear: "delete-2",
        gear: "settings-3",
        "gear-filled": "settings-3",
        search: "search",
        refresh: "refresh-1",
        copy: "copy-2",
        download: "download-2",
        check: "check",
        checkmark: "check",
        "arrow-left": "arrow-left",
        "arrow-right": "arrow-right",
        "chevron-down": "down",
        "chevron-right": "right",
        "triangle-right": "right",
        "triangle-down": "down",
        star: "star",
        code: "code",
        folder: "folder-2",
        "folder-open": "folder-open",
        link: "link",
        edit: "pencil",
        pen: "pencil",
        lock: "lock",
        "left-panel-open": "sidebar-left-open",
        "left-panel-close": "sidebar-left-close",
        puzzle: "plugin-2",
        terminal: "code",
        elements: "code",
        sources: "code",
        policy: "lock",
        security: "lock",
        "more-tabs": "add",
        "plus-small": "add",
        layers: "layers",
        "bracket-curly": "code",
        braces: "code",
        tune: "settings-3",
      };
      const out = {};
      for (const [name, icon] of Object.entries(MAP)) {
        for (const c of [icon]) {
          let node = null;
          try {
            node = ic(c, 24);
          } catch (_) {}
          const svg =
            node &&
            (node.localName === "svg"
              ? node
              : node.querySelector && node.querySelector("svg"));
          if (!svg || !svg.childElementCount) continue;
          const clone = svg.cloneNode(true);
          clone.setAttribute("xmlns", "http://www.w3.org/2000/svg");
          clone.setAttribute("width", "24");
          clone.setAttribute("height", "24");
          if (!clone.getAttribute("viewBox"))
            clone.setAttribute("viewBox", "0 0 24 24");
          const markup = new XMLSerializer()
            .serializeToString(clone)
            .replace(/currentColor/g, "#000");
          const uri = "data:image/svg+xml," + encodeURIComponent(markup);
          if (uri.length <= 12000) {
            out[name] = uri;
            break;
          }
        }
      }
      if (Object.keys(out).length) post({ cmd: "dev_tools_icons", icons: out });
    };
    try {
      sendIcons();
    } catch (_) {}

    // Styled with the sidebar's own design tokens so it always matches the rest of the browser.
    const style = document.createElement("style");
    style.textContent = `
    .poof-dt-header{position:fixed;z-index:2147483647;display:none;align-items:center;gap:8px;
        box-sizing:border-box;padding:0 4px 0 10px;
        background:var(--chrome);color:var(--ink-2);
        border-bottom:1px solid var(--line);
        font:600 12px var(--f-display);letter-spacing:-.005em;user-select:none;border:none;}
    .poof-dt-header svg{width:14px;height:14px;flex:none}
    .poof-dt-title{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;background:var(--chrome);}
    .poof-dt-close{width:24px;height:24px;padding:0;border:0;border-radius:var(--r-sm);
        display:inline-flex;align-items:center;justify-content:center;background:transparent;
        color:var(--ink-2);cursor:pointer;transition:background .12s,color .12s}
    .poof-dt-close:hover{background:var(--hover);color:var(--ink)}
    .poof-dt-close:active{background:var(--pressed)}
    .poof-dt-handle{position:fixed;z-index:2147483647;display:none;touch-action:none;user-select:none;background:var(--chrome);}
    .poof-dt-handle i{position:absolute;border-radius:2px;background:transparent;transition:background .12s}
    .poof-dt-handle:hover i,.poof-dt-handle.on i{background:var(--space)}`;
    document.head.appendChild(style);

    const icon = (name, size) => {
      try {
        return typeof ic === "function" ? ic(name, size) : null;
      } catch (_) {
        return null;
      }
    };

    const handle = document.createElement("div");
    handle.className = "poof-dt-handle";
    const line = document.createElement("i");
    handle.appendChild(line);

    let drag = null,
      raf = 0,
      last = 0;
    const send = (size, persist) =>
      post({ cmd: "dev_tools_size", size, persist });
    const sizeAt = (e) => {
      const [cx, cy, cw, ch] = ui.cfg.box,
        gap = ui.cfg.gap;
      switch (ui.cfg.position) {
        case "left":
          return e.clientX - cx - gap / 2;
        case "top":
          return e.clientY - cy - gap / 2;
        case "bottom":
          return cy + ch - e.clientY - gap / 2;
        default:
          return cx + cw - e.clientX - gap / 2;
      }
    };
    handle.addEventListener("pointerdown", (e) => {
      if (e.button !== 0) return;
      drag = e.pointerId;
      handle.setPointerCapture(e.pointerId);
      handle.classList.add("on");
      e.preventDefault();
    });
    handle.addEventListener("pointermove", (e) => {
      if (drag !== e.pointerId) return;
      last = sizeAt(e);
      if (!raf)
        raf = requestAnimationFrame(() => {
          raf = 0;
          send(last, false);
        });
    });
    const end = (e) => {
      if (drag !== e.pointerId) return;
      drag = null;
      if (raf) {
        cancelAnimationFrame(raf);
        raf = 0;
      }
      send(sizeAt(e), true);
      handle.classList.remove("on");
    };
    handle.addEventListener("pointerup", end);
    handle.addEventListener("pointercancel", end);

    const header = document.createElement("div");
    header.className = "poof-dt-header";
    const title = document.createElement("span");
    title.className = "poof-dt-title";
    title.textContent = "Developer Tools";
    const close = document.createElement("button");
    close.type = "button";
    close.className = "poof-dt-close";
    close.title = "Close Developer Tools (F12)";
    close.setAttribute("aria-label", "Close Developer Tools");
    const x = icon("close", 14);
    if (x) close.appendChild(x);
    else close.textContent = "×";
    close.addEventListener("click", () => post({ cmd: "dev_tools_toggle" }));
    const lead = icon("code", 14);
    if (lead) header.append(lead);
    header.append(title, close);

    document.body.append(handle, header);
    Object.assign(ui, { handle, line, header, title });
  }
  ui.cfg = cfg;

  const place = (el, b) =>
    Object.assign(el.style, {
      display: "block",
      left: b[0] + "px",
      top: b[1] + "px",
      width: b[2] + "px",
      height: b[3] + "px",
    });
  place(ui.header, cfg.header);
  ui.header.style.display = "flex";

  if (!cfg.handle) {
    ui.handle.style.display = "none";
    return;
  }
  const vertical = cfg.position === "left" || cfg.position === "right";
  place(ui.handle, cfg.handle);
  ui.handle.style.cursor = vertical ? "col-resize" : "row-resize";
  Object.assign(
    ui.line.style,
    vertical
      ? {
          left: "50%",
          top: "0",
          width: "2px",
          height: "100%",
          transform: "translateX(-50%)",
        }
      : {
          top: "50%",
          left: "0",
          height: "2px",
          width: "100%",
          transform: "translateY(-50%)",
        },
  );
};
