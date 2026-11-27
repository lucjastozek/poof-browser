(() => {
  if (window.__poofSb) return;
  let color = "#6dc5e2";
  let on = __VISIBLE__;
  let host = null,
    thumb = null,
    hideT = 0,
    drag = null,
    hover = false,
    ready = false,
    raf = 0;

  const root = () => document.documentElement;
  const scroller = () => document.scrollingElement || document.documentElement;
  const fill = () => "color-mix(in srgb, " + color + " 65%, transparent)";

  // The scrollbar that applies to the viewport comes from <html>, or from <body> when
  // <html> leaves overflow at its default.
  const scrollable = () => {
    const r = root(),
      b = document.body;
    let ov = getComputedStyle(r).overflowY;
    if (ov === "visible" && b) ov = getComputedStyle(b).overflowY;
    return ov !== "hidden" && ov !== "clip";
  };

  const layout = () => {
    if (!thumb) return false;
    const s = scroller();
    const ch = s.clientHeight,
      sh = s.scrollHeight;
    if (!on || document.fullscreenElement || sh - ch <= 1 || !scrollable()) {
      thumb.style.display = "none";
      return false;
    }
    thumb.style.display = "block";
    const th = Math.max(40, Math.round((ch * ch) / sh));
    const y = (s.scrollTop / (sh - ch)) * (ch - th);
    thumb.style.height = th + "px";
    thumb.style.transform = "translateY(" + Math.max(0, y) + "px)";
    return true;
  };

  const show = () => {
    if (!layout()) return;
    if (host && !host.isConnected && root()) root().appendChild(host);
    thumb.classList.add("on");
    clearTimeout(hideT);
    hideT = setTimeout(() => {
      if (!drag && !hover) thumb.classList.remove("on");
    }, 900);
  };

  const build = () => {
    const r = root();
    if (!r || ready) return;
    ready = true;

    const hide = document.createElement("style");
    hide.textContent =
      "html{scrollbar-width:none !important}" +
      "html::-webkit-scrollbar{display:none !important;width:0 !important;height:0 !important}";
    r.appendChild(hide);

    host = document.createElement("div");
    host.style.cssText =
      "position:fixed!important;top:0!important;right:0!important;bottom:0!important;" +
      "width:0!important;height:100%!important;z-index:2147483647!important;" +
      "pointer-events:none!important;display:block!important;margin:0!important;" +
      "padding:0!important;border:0!important;background:none!important;";
    const sh = host.attachShadow({ mode: "closed" });
    const st = document.createElement("style");
    st.textContent =
      ".t{position:absolute;right:2px;top:0;width:6px;border-radius:999px;display:none;" +
      "opacity:0;transition:opacity .25s,width .12s;pointer-events:none;will-change:transform}" +
      ".t.on{opacity:1;pointer-events:auto}" +
      ".t:hover,.t.drag{width:10px}";
    thumb = document.createElement("div");
    thumb.className = "t";
    thumb.style.background = fill();
    sh.append(st, thumb);
    r.appendChild(host);

    thumb.addEventListener("pointerenter", () => {
      hover = true;
      clearTimeout(hideT);
    });
    thumb.addEventListener("pointerleave", () => {
      hover = false;
      show();
    });
    thumb.addEventListener("pointerdown", (e) => {
      if (e.button !== 0) return;
      e.preventDefault();
      thumb.setPointerCapture(e.pointerId);
      drag = { y: e.clientY, top: scroller().scrollTop };
      thumb.classList.add("drag");
    });
    thumb.addEventListener("pointermove", (e) => {
      if (!drag) return;
      const s = scroller();
      const range = s.scrollHeight - s.clientHeight;
      const track = s.clientHeight - thumb.offsetHeight;
      if (track > 0)
        s.scrollTop = drag.top + ((e.clientY - drag.y) * range) / track;
    });
    const end = () => {
      if (!drag) return;
      drag = null;
      thumb.classList.remove("drag");
      show();
    };
    thumb.addEventListener("pointerup", end);
    thumb.addEventListener("pointercancel", end);

    window.addEventListener(
      "scroll",
      () => {
        if (!drag) show();
      },
      { passive: true },
    );
    window.addEventListener(
      "mousemove",
      (e) => {
        if (e.clientX >= window.innerWidth - 14) show();
      },
      { passive: true },
    );
    window.addEventListener("resize", layout, { passive: true });
    window.addEventListener("load", layout);
    document.addEventListener("fullscreenchange", layout);

    if (window.ResizeObserver) {
      const ro = new ResizeObserver(() => {
        if (raf) return;
        raf = requestAnimationFrame(() => {
          raf = 0;
          layout();
        });
      });
      ro.observe(r);
      if (document.body) ro.observe(document.body);
      else
        document.addEventListener(
          "DOMContentLoaded",
          () => document.body && ro.observe(document.body),
        );
    }
    layout();
  };

  window.__poofSbVisible = (v) => {
    on = !!v;
    if (!thumb) return;
    if (on) show();
    else {
      thumb.classList.remove("on");
      layout();
    }
  };

  window.__poofSb = (c) => {
    if (!/^#[0-9a-fA-F]{6}$/.test(c)) return;
    color = c;
    if (thumb) {
      thumb.style.background = fill();
      return;
    }
    if (root()) {
      build();
      return;
    }
    // Document start: <html> does not exist yet; wait for it, never touch the document itself.
    const mo = new MutationObserver(() => {
      if (root()) {
        mo.disconnect();
        build();
      }
    });
    mo.observe(document, { childList: true });
  };
  window.__poofSb("__ACCENT__");
})();
