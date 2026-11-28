// Page right-click menu, drawn in the page's top document inside a closed shadow root so the page
// cannot restyle or read it. Native code (context_menu.rs) lists the entries and keeps WebView2's
// own menu request waiting; this only shows them and reports one choice with a one-time token.
// Placeholders are filled in there. The look mirrors the sidebar's own menus (ui/styles/layout).
(() => {
  if (window.__poofCtx) window.__poofCtx.close(true);
  const NONCE = "__NONCE__";
  const ACCENT = "__ACCENT__";
  const THEME = "__THEME__";
  const ITEMS = __ITEMS__;
  const TARGET = __TARGET__; // link or image address under the pointer, or ""
  const LOCATORS = __LOCATORS__; // (element, url) => "Copy as" entries (js/locators.js)
  const PX = __X__ / (devicePixelRatio || 1);
  const PY = __Y__ / (devicePixelRatio || 1);
  const dark =
    THEME === "dark" ||
    (THEME !== "light" && matchMedia("(prefers-color-scheme: dark)").matches);

  const host = document.createElement("div");
  host.style.cssText =
    "all:initial;position:fixed;inset:0;z-index:2147483647;pointer-events:none;";
  const root = host.attachShadow({ mode: "closed" });
  root.innerHTML = `<style>
    :host{all:initial}
    .m{
      --space:${ACCENT};
      --ink:${dark ? "#fffbe6" : "#050505"};
      --ink-3:color-mix(in srgb,var(--ink) 52%,transparent);
      --line:color-mix(in srgb,var(--ink) ${dark ? 18 : 11}%,transparent);
      --bg:${
        dark
          ? "color-mix(in srgb,#fffbe6 11%,color-mix(in srgb,var(--space) 17%,color-mix(in srgb,#fffbe6 2.5%,#050505)))"
          : "color-mix(in srgb,#fff 78%,#fffbe6)"
      };
      --hover:color-mix(in srgb,var(--ink) ${dark ? 12 : 8}%,transparent);
      position:fixed;pointer-events:auto;box-sizing:border-box;min-width:176px;max-width:320px;padding:5px;
      border-radius:14px;color:var(--ink);font:12px/1.4 "Inter","Segoe UI Variable Text","Segoe UI",system-ui,sans-serif;
      background:color-mix(in srgb,var(--bg) 94%,transparent);
      -webkit-backdrop-filter:blur(16px) saturate(1.4);backdrop-filter:blur(16px) saturate(1.4);
      box-shadow:${
        dark
          ? "0 0 0 1px var(--line),0 2px 4px rgba(0,0,0,.3),0 14px 36px -8px rgba(0,0,0,.7)"
          : "0 0 0 1px var(--line),0 2px 4px rgba(5,5,5,.05),0 12px 32px -8px rgba(5,5,5,.22)"
      };
      user-select:none;cursor:default;transform-origin:top left;animation:pop .14s cubic-bezier(.16,1,.3,1);
      max-height:calc(100vh - 12px);overflow:auto;scrollbar-width:none}
    .m.sub{animation-duration:.1s}
    .i{display:flex;align-items:center;gap:16px;width:100%;min-height:30px;padding:0 10px;border:0;border-radius:8px;
      background:none;color:inherit;font:inherit;text-align:left;cursor:pointer;white-space:nowrap}
    .i .l{flex:1;overflow:hidden;text-overflow:ellipsis}
    .i .h{color:var(--ink-3);font-size:11px}
    .i:hover,.i:focus-visible,.i.open{background:var(--hover);outline:0}
    .i:focus-visible{box-shadow:inset 0 0 0 1.5px var(--space)}
    .i:active{transform:scale(.985)}
    .i[aria-disabled=true]{opacity:.4;pointer-events:none}
    .i .c{width:12px;margin-left:-4px;margin-right:-10px;color:var(--space)}
    .i .a{color:var(--ink-3)}
    .s{height:1px;margin:4px 6px;background:var(--line)}
    @keyframes pop{from{opacity:0;transform:translateY(6px) scale(.985)}to{opacity:1;transform:none}}
    @media (prefers-reduced-motion:reduce){.m{animation:none}}
  </style>`;

  // Tidy the list: no leading, trailing or doubled separators, no empty submenus.
  const tidy = (list) => {
    const out = [];
    for (const it of list) {
      if (it.k === "x" && !(it.c || []).length) continue;
      if (it.k === "s" && (!out.length || out[out.length - 1].k === "s"))
        continue;
      out.push(it);
    }
    while (out.length && out[out.length - 1].k === "s") out.pop();
    return out;
  };

  let done = false;
  const menus = [];
  const post = (id, extra) => {
    try {
      window.ipc.postMessage(
        JSON.stringify({ t: "ctx_pick", n: NONCE, id, ...extra }),
      );
    } catch (e) {}
  };
  const close = (silent) => {
    if (done) return;
    done = true;
    removeEventListener("pointerdown", onOutside, true);
    removeEventListener("keydown", onKey, true);
    removeEventListener("blur", onBlur);
    removeEventListener("scroll", onBlur, true);
    removeEventListener("resize", onBlur);
    removeEventListener("wheel", onBlur, { capture: true });
    host.remove();
    delete window.__poofCtx;
    if (!silent) post(0);
  };
  const pick = (id) => {
    if (done) return;
    close(true);
    post(id);
  };
  // "Copy as" entry: native code puts the text on the clipboard and shows the toast.
  const copyAs = (it) => {
    if (done) return;
    close(true);
    post(-5, { c: it.t, l: it.l });
  };
  const onOutside = (e) => {
    if (!e.composedPath().includes(host)) close();
  };
  const onBlur = () => close();

  function build(list, x, y, sub, parentBtn) {
    const m = document.createElement("div");
    m.className = "m" + (sub ? " sub" : "");
    m.setAttribute("role", "menu");
    m.style.visibility = "hidden";
    tidy(list).forEach((it) => {
      if (it.k === "s") {
        const s = document.createElement("div");
        s.className = "s";
        s.setAttribute("role", "separator");
        m.append(s);
        return;
      }
      const b = document.createElement("button");
      b.className = "i";
      b.tabIndex = -1;
      b.type = "button";
      const label = (it.l || "").replace(/&(.)/g, "$1");
      b.setAttribute(
        "role",
        it.k === "c"
          ? "menuitemcheckbox"
          : it.k === "r"
            ? "menuitemradio"
            : "menuitem",
      );
      if (it.k === "c" || it.k === "r")
        b.setAttribute("aria-checked", String(!!it.v));
      if (it.e === false) b.setAttribute("aria-disabled", "true");
      if (it.k === "c" || it.k === "r") {
        const c = document.createElement("span");
        c.className = "c";
        c.textContent = it.v ? "✓" : "";
        b.append(c);
      }
      const l = document.createElement("span");
      l.className = "l";
      l.textContent = label;
      b.append(l);
      if (it.h) {
        const h = document.createElement("span");
        h.className = "h";
        h.textContent = it.h;
        b.append(h);
      }
      if (it.k === "x") {
        b.setAttribute("aria-haspopup", "menu");
        const a = document.createElement("span");
        a.className = "a";
        a.textContent = "›";
        b.append(a);
        let t = 0;
        const open = () => {
          clearTimeout(t);
          closeSubs(m);
          b.classList.add("open");
          const r = b.getBoundingClientRect();
          const s = build(it.c, r.right + 2, r.top - 5, true, b);
          s.__parent = m;
        };
        b.onpointerenter = () => (t = setTimeout(open, 120));
        b.onpointerleave = () => clearTimeout(t);
        b.onclick = open;
        b.__open = open;
      } else {
        b.onpointerenter = () => {
          closeSubs(m);
          b.focus({ preventScroll: true });
        };
        b.onclick = () =>
          it.e !== false && (it.t !== undefined ? copyAs(it) : pick(it.i));
      }
      m.append(b);
    });
    root.append(m);
    menus.push(m);
    const w = m.offsetWidth,
      h = m.offsetHeight,
      vw = innerWidth,
      vh = innerHeight;
    let left = x,
      top = y;
    if (left + w > vw - 6)
      left =
        sub && parentBtn
          ? parentBtn.getBoundingClientRect().left - w - 2
          : vw - w - 6;
    if (top + h > vh - 6) top = Math.max(6, vh - h - 6);
    m.style.left = Math.max(6, left) + "px";
    m.style.top = Math.max(6, top) + "px";
    m.style.visibility = "";
    return m;
  }
  function closeSubs(m) {
    for (let i = menus.length - 1; i >= 0; i--) {
      if (menus[i] === m) break;
      if (menus[i].classList.contains("sub")) {
        menus[i].remove();
        menus.pop();
      }
    }
    m.querySelectorAll(".open").forEach((b) => b.classList.remove("open"));
  }
  const active = () => menus[menus.length - 1];
  const itemsOf = (m) => [
    ...m.querySelectorAll(".i:not([aria-disabled=true])"),
  ];
  const onKey = (e) => {
    const m = active();
    const btns = itemsOf(m);
    const cur = btns.findIndex((b) => b === root.activeElement);
    const stop = () => {
      e.preventDefault();
      e.stopPropagation();
    };
    if (e.key === "Escape") {
      stop();
      if (menus.length > 1) {
        const parent = menus[menus.length - 2];
        closeSubs(parent);
        const open =
          parent.querySelector(".open") || parent.querySelector(".i");
        open && open.focus();
      } else close();
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      stop();
      const d = e.key === "ArrowDown" ? 1 : -1;
      btns[(cur + d + btns.length) % btns.length]?.focus();
    } else if (e.key === "Home" || e.key === "End") {
      stop();
      btns[e.key === "Home" ? 0 : btns.length - 1]?.focus();
    } else if (e.key === "ArrowRight") {
      const b = btns[cur];
      if (b && b.__open) {
        stop();
        b.__open();
        const s = active();
        itemsOf(s)[0]?.focus();
      }
    } else if (e.key === "ArrowLeft" && menus.length > 1) {
      stop();
      const parent = menus[menus.length - 2];
      closeSubs(parent);
      (
        parent.querySelector(".i[aria-haspopup]") || parent.querySelector(".i")
      )?.focus();
    } else if (e.key === "Enter" || e.key === " ") {
      stop();
      btns[cur]?.click();
    } else if (e.key === "Tab") stop();
    else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
      stop(); // type-ahead
      const k = e.key.toLowerCase();
      const n = btns.length;
      for (let i = 1; i <= n; i++) {
        const b = btns[(Math.max(cur, -1) + i) % n];
        if (b.textContent.trim().toLowerCase().startsWith(k)) return b.focus();
      }
    }
  };

  try {
    let el = document.elementFromPoint(PX, PY);
    while (el && el.shadowRoot) {
      const inner = el.shadowRoot.elementFromPoint(PX, PY);
      if (!inner || inner === el) break;
      el = inner;
    }
    const extra =
      el && el !== document.documentElement ? LOCATORS(el, TARGET) : [];
    if (extra.length)
      ITEMS.push({ k: "s" }, { k: "x", l: "Copy as", e: true, c: extra });
  } catch (e) {}
  const m = build(ITEMS, PX, PY, false, null);
  document.documentElement.append(host);
  // Place again now that it is attached and measurable.
  const w = m.offsetWidth,
    h = m.offsetHeight;
  m.style.left = Math.max(6, Math.min(PX, innerWidth - w - 6)) + "px";
  m.style.top = Math.max(6, PY + h > innerHeight - 6 ? PY - h : PY) + "px";
  m.style.visibility = "";
  const first = itemsOf(m)[0];
  first && first.focus({ preventScroll: true });
  addEventListener("pointerdown", onOutside, true);
  addEventListener("keydown", onKey, true);
  addEventListener("blur", onBlur);
  addEventListener("scroll", onBlur, true);
  addEventListener("resize", onBlur);
  addEventListener("wheel", onBlur, { capture: true, passive: true });
  Object.defineProperty(window, "__poofCtx", {
    value: { close },
    configurable: true,
  });
})();
