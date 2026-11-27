(cfg) => {
  const st =
    window.__poof ||
    (window.__poof = {
      dock: null,
      loading: false,
      failed: false,
      pos: null,
      resp: null,
      settle: 0,
      emu: null,
      emuTried: false,
      want: 0,
      from: false,
      lastOn: false,
      hooked: false,
      hideAt: 0,
      iconsKey: 0,
    });
  const wait = () => ({ ready: false });
  try {
    if (!document.querySelector(".root-view")) return wait();

    // Closing is handled by the native header; the frontend's own button does nothing here.
    if (!document.getElementById("__poof_style")) {
      const style = document.createElement("style");
      style.id = "__poof_style";
      style.textContent = ".close-devtools{display:none!important}";
      document.head.appendChild(style);
    }

    // The browser's own icons. The frontend draws its icons through CSS masks that read
    // --image-file-<name> variables, so overriding those swaps the whole icon set. Sent only
    // until the native side sees `icons_key` echoed back in the report.
    if (cfg.icons && st.iconsKey !== cfg.icons_key) {
      st.iconsKey = cfg.icons_key;
      let is = document.getElementById("__poof_icons");
      if (!is) {
        is = document.createElement("style");
        is.id = "__poof_icons";
        document.head.appendChild(is);
      }
      is.textContent =
        "html:root{" +
        Object.entries(cfg.icons)
          .map(([n, u]) => `--image-file-${n}:url("${u}")!important;`)
          .join("") +
        "}";
    }

    // Per-element override: the frontend's icon elements read their image from variables whose
    // names differ between versions, so each <devtools-icon> is also masked directly with the
    // browser's icon. Names with no equivalent are reported back (and logged natively).
    if (cfg.icons) st.icons = cfg.icons;
    const px = cfg.icon_px || 14;
    if (st.icons && Date.now() - (st.iconAt || 0) > 250) {
      st.iconAt = Date.now();
      const misses = st.misses || (st.misses = new Set());
      const swap = (root) => {
        for (const el of root.querySelectorAll("*")) {
          if (el.shadowRoot) swap(el.shadowRoot);
          if (el.localName !== "devtools-icon") continue;
          const name = el.name || el.getAttribute("name") || "";
          const uri = st.icons[name];
          if (!uri) {
            if (name && misses.size < 60) misses.add(name);
            continue;
          }
          const tag = name + "|" + px;
          if (el.dataset.poofIcon === tag) continue;
          el.dataset.poofIcon = tag;
          const v = `url("${uri}")`;
          for (const [k, val] of [
            ["-webkit-mask-image", v],
            ["mask-image", v],
            ["-webkit-mask-size", "contain"],
            ["mask-size", "contain"],
            ["-webkit-mask-repeat", "no-repeat"],
            ["mask-repeat", "no-repeat"],
            ["-webkit-mask-position", "center"],
            ["mask-position", "center"],
          ])
            el.style.setProperty(k, val, "important");
          // One small, uniform size for every icon (the frontend's own sizes are ~20px).
          for (const k of [
            "width",
            "height",
            "min-width",
            "min-height",
            "max-width",
            "max-height",
          ])
            el.style.setProperty(k, px + "px", "important");
        }
      };
      swap(document);
    }

    // Match the browser's design: the same token formulas as ui/styles/foundation/theme.css (with this space's color),
    // mapped onto the variables the DevTools frontend reads. Neutral surfaces are built from the
    // sidebar's chrome color; the space color only tints small non-text indicators.
    const themeKey = (cfg.dark ? "d" : "l") + cfg.accent;
    if (st.themeKey !== themeKey) {
      st.themeKey = themeKey;
      const sp = cfg.accent;
      const sbCss = (c) => `
    *::-webkit-scrollbar{width:10px!important;height:10px!important;background:transparent!important}
    *::-webkit-scrollbar-track,*::-webkit-scrollbar-track-piece,*::-webkit-scrollbar-corner{
        background:transparent!important;border:0!important}
    *::-webkit-scrollbar-button{display:none!important;width:0!important;height:0!important}
    *::-webkit-scrollbar-thumb{min-height:32px;min-width:32px;border-radius:999px!important;
        border:3px solid transparent!important;background-clip:padding-box!important;
        background-color:color-mix(in srgb,${c} 45%,transparent)!important}
    *::-webkit-scrollbar-thumb:hover{border-width:2px!important;
        background-color:color-mix(in srgb,${c} 80%,transparent)!important}`;
      if (!st.sheet) st.sheet = new CSSStyleSheet();
      st.sheet.replaceSync(sbCss(sp));
      const tokens = cfg.dark
        ? `
        color-scheme:dark;
        --p-space:${sp};
        --p-chrome:color-mix(in srgb,${sp} 17%,color-mix(in srgb,#fffbe6 2.5%,#050505));
        --p-ink:color-mix(in srgb,#fffbe6 92%,var(--p-chrome));
        --p-ink2:color-mix(in srgb,#fffbe6 68%,var(--p-chrome));
        --p-ink3:color-mix(in srgb,#fffbe6 50%,var(--p-chrome));
        --p-line:color-mix(in srgb,#fffbe6 10%,transparent);
        --p-line2:color-mix(in srgb,#fffbe6 18%,transparent);
        --p-surface:color-mix(in srgb,#fffbe6 4%,var(--p-chrome));
        --p-raised:color-mix(in srgb,#fffbe6 8%,var(--p-chrome));
        --p-select:color-mix(in srgb,#fffbe6 14%,var(--p-chrome));
        --p-hover:color-mix(in srgb,#fffbe6 7%,transparent);
        --p-pressed:color-mix(in srgb,#fffbe6 12%,transparent);
        --p-on-accent:#050505;
        --p-red:#ee7563;--p-orange:#ea8644;--p-yellow:#ebcd47;--p-green:#56c876;
        --p-blue:#6dc5e2;--p-purple:#b186d5;--p-pink:#de7c8e;`
        : `
        color-scheme:light;
        --p-space:${sp};
        --p-chrome:color-mix(in srgb,${sp} 15%,color-mix(in srgb,#050505 3%,#fffbe6));
        --p-ink:#050505;
        --p-ink2:color-mix(in srgb,#050505 66%,var(--p-chrome));
        --p-ink3:color-mix(in srgb,#050505 52%,var(--p-chrome));
        --p-line:color-mix(in srgb,#050505 11%,transparent);
        --p-line2:color-mix(in srgb,#050505 18%,transparent);
        --p-surface:color-mix(in srgb,#fff 35%,var(--p-chrome));
        --p-raised:color-mix(in srgb,#fff 65%,var(--p-chrome));
        --p-select:color-mix(in srgb,#050505 10%,var(--p-chrome));
        --p-hover:color-mix(in srgb,#050505 6%,transparent);
        --p-pressed:color-mix(in srgb,#050505 11%,transparent);
        --p-on-accent:#fffbe6;
        --p-red:#992f1e;--p-orange:#913c03;--p-yellow:#6f5006;--p-green:#205b33;
        --p-blue:#0f5a76;--p-purple:#573280;--p-pink:#832232;`;
      let ts = document.getElementById("__poof_theme");
      if (!ts) {
        ts = document.createElement("style");
        ts.id = "__poof_theme";
        document.head.appendChild(ts);
      }
      ts.textContent = `html:root{
        ${tokens}
        /* neutral accent: ink with a hint of the space color, so text/links stay readable */
        --p-accent:color-mix(in srgb,var(--p-space) 22%,var(--p-ink));
        --p-mono:"Cascadia Code","Cascadia Mono",Consolas,"SF Mono",Menlo,monospace;

        /* typography */
        --default-font-family:Inter,"Segoe UI Variable Text","Segoe UI",system-ui,sans-serif!important;
        --default-font-size:12px!important;
        --monospace-font-family:var(--p-mono)!important;
        --source-code-font-family:var(--p-mono)!important;
        --monospace-font-size:12px!important;
        --source-code-font-size:12px!important;

        /* surfaces: one card color, flat toolbars, a step up only for popovers */
        --sys-color-cdt-base-container:var(--p-surface)!important;
        --sys-color-base:var(--p-surface)!important;
        --sys-color-base-container:var(--p-surface)!important;
        --sys-color-surface:var(--p-surface)!important;
        --sys-color-surface-bright:var(--p-surface)!important;
        --sys-color-surface1:var(--p-surface)!important;
        --sys-color-surface2:var(--p-surface)!important;
        --sys-color-surface3:var(--p-surface)!important;
        --sys-color-surface4:var(--p-surface)!important;
        --sys-color-surface5:var(--p-surface)!important;
        --sys-color-surface-container:var(--p-surface)!important;
        --sys-color-surface-container-high:var(--p-raised)!important;
        --sys-color-surface-variant:var(--p-surface)!important;
        --sys-color-neutral-container:var(--p-raised)!important;

        /* selection: neutral, not space */
        --sys-color-tonal-container:var(--p-select)!important;
        --sys-color-secondary-container:var(--p-select)!important;
        --sys-color-state-focus-highlight:var(--p-select)!important;
        --sys-color-state-focus-select:var(--p-select)!important;
        --sys-color-on-tonal-container:var(--p-ink)!important;
        --sys-color-on-secondary-container:var(--p-ink)!important;

        /* text */
        --sys-color-on-surface:var(--p-ink)!important;
        --sys-color-on-base:var(--p-ink)!important;
        --sys-color-on-surface-subtle:var(--p-ink2)!important;
        --sys-color-on-surface-variant:var(--p-ink2)!important;
        --sys-color-token-subtle:var(--p-ink2)!important;
        --sys-color-state-disabled:var(--p-ink3)!important;
        --sys-color-on-surface-inverse:var(--p-chrome)!important;
        --sys-color-inverse-surface:var(--p-ink)!important;

        /* dividers and hover/press states */
        --sys-color-divider:var(--p-line)!important;
        --sys-color-outline:var(--p-line)!important;
        --sys-color-outline-variant:var(--p-line)!important;
        --sys-color-neutral-outline:var(--p-line2)!important;
        --sys-color-state-hover-on-subtle:var(--p-hover)!important;
        --sys-color-state-hover-on-prominent:var(--p-hover)!important;
        --sys-color-state-ripple-neutral-on-subtle:var(--p-pressed)!important;
        --sys-color-state-ripple-primary:var(--p-pressed)!important;

        /* primary: readable ink-based accent; the space color only appears as a thin outline */
        --sys-color-primary:var(--p-accent)!important;
        --sys-color-primary-bright:var(--p-accent)!important;
        --sys-color-tonal-outline:var(--p-space)!important;
        --sys-color-on-primary:var(--p-on-accent)!important;

        /* status */
        --sys-color-error:var(--p-red)!important;
        --sys-color-on-error:var(--p-on-accent)!important;
        --sys-color-error-container:color-mix(in srgb,var(--p-red) 16%,var(--p-chrome))!important;
        --sys-color-on-error-container:var(--p-ink)!important;
        --sys-color-orange-container:color-mix(in srgb,var(--p-orange) 16%,var(--p-chrome))!important;
        --sys-color-yellow-container:color-mix(in srgb,var(--p-yellow) 16%,var(--p-chrome))!important;

        /* syntax: the browser's own palette, tuned per mode for contrast */
        --sys-color-token-meta:var(--p-ink3)!important;
        --sys-color-token-comment:var(--p-ink3)!important;
        --sys-color-token-keyword:var(--p-purple)!important;
        --sys-color-token-tag:var(--p-red)!important;
        --sys-color-token-attribute:var(--p-orange)!important;
        --sys-color-token-attribute-value:var(--p-blue)!important;
        --sys-color-token-string:var(--p-green)!important;
        --sys-color-token-string-special:var(--p-green)!important;
        --sys-color-token-number:var(--p-blue)!important;
        --sys-color-token-property:var(--p-blue)!important;
        --sys-color-token-variable:var(--p-ink)!important;
        --sys-color-token-variable-special:var(--p-pink)!important;
        --sys-color-token-definition:var(--p-ink)!important;
        --sys-color-token-type:var(--p-yellow)!important;

        /* icons */
        --icon-default:var(--p-ink2)!important;
        --icon-default-hover:var(--p-ink)!important;
        --icon-active:var(--p-ink)!important;
        --icon-toggled:var(--p-ink)!important;
        --icon-disabled:var(--p-ink3)!important;
        --icon-primary:var(--p-accent)!important;

        /* legacy variables some panels still read */
        --color-background:var(--p-surface)!important;
        --color-background-elevation-1:var(--p-surface)!important;
        --color-background-elevation-2:var(--p-raised)!important;
        --color-text-primary:var(--p-ink)!important;
        --color-text-secondary:var(--p-ink2)!important;
        --color-text-disabled:var(--p-ink3)!important;
        --color-details-hairline:var(--p-line)!important;
        --color-primary:var(--p-accent)!important;
    }
    html,body,.root-view{background:var(--p-surface)!important;color:var(--p-ink)!important;
        font-family:var(--default-font-family)!important;font-size:12px}
    ::selection{background:color-mix(in srgb,var(--p-space) 35%,transparent)}`;
      // Also make the frontend's own light/dark mode match (best effort).
      import("./core/common/common.js")
        .then((m) => {
          m.Settings.Settings.instance()
            .moduleSetting("uiTheme")
            .set(cfg.dark ? "dark" : "default");
        })
        .catch(() => {});
    }

    if (st.sheet && Date.now() - (st.sheetAt || 0) > 300) {
      st.sheetAt = Date.now();
      const adopt = (root) => {
        if (!root.adoptedStyleSheets.includes(st.sheet))
          root.adoptedStyleSheets = [...root.adoptedStyleSheets, st.sheet];
        for (const el of root.querySelectorAll("*"))
          if (el.shadowRoot) adopt(el.shadowRoot);
      };
      adopt(document);
    }

    // A click on the device emulation button (or Ctrl+Shift+M) expands the frontend immediately.
    // composedPath() sees through shadow roots.
    if (!st.hooked) {
      st.hooked = true;
      const mark = () => {
        st.want = Date.now();
        st.from = st.lastOn;
      };
      const isToggle = (n) =>
        n &&
        n.getAttribute &&
        /device (emulation|toolbar)/i.test(
          (n.getAttribute("aria-label") || "") +
            " " +
            (n.getAttribute("title") || ""),
        );
      document.addEventListener(
        "click",
        (e) => {
          if (e.composedPath().some(isToggle)) mark();
        },
        true,
      );
      document.addEventListener(
        "keydown",
        (e) => {
          if (
            (e.ctrlKey || e.metaKey) &&
            e.shiftKey &&
            (e.key === "M" || e.key === "m")
          )
            mark();
        },
        true,
      );
    }

    // Toolbar buttons live in shadow roots, so a document-level stylesheet can't reach them.
    // Hide the screencast toggle and the built-in close button (the native header has its own)
    // by walking the shadow roots (throttled; the toolbar can re-render).
    if (Date.now() - st.hideAt > 300) {
      st.hideAt = Date.now();
      const attrs = ["title", "aria-label", "jslog"];
      const hide = (root) => {
        for (const el of root.querySelectorAll("*")) {
          if (el.shadowRoot) hide(el.shadowRoot);
          const cls = typeof el.className === "string" ? el.className : "";
          if (
            !/close-devtools/.test(cls) &&
            !attrs.some((a) => /screencast/i.test(el.getAttribute(a) || ""))
          )
            continue;
          let target = el;
          for (;;) {
            const r = target.getRootNode();
            if (r instanceof ShadowRoot && /button/i.test(r.host.localName))
              target = r.host;
            else break;
          }
          target.style.setProperty("display", "none", "important");
        }
      };
      hide(document);
    }

    if (!st.emu && !st.emuTried) {
      st.emuTried = true;
      import("./models/emulation/emulation.js")
        .then((m) => {
          st.emu = m;
        })
        .catch(() => {});
    }

    const advanced = window.Emulation && window.Emulation.AdvancedApp;
    const app = advanced && advanced.instance();
    const split = app && app.rootSplitWidget;
    if (!split) return wait();

    const sides = ["right", "bottom", "left", "undocked"];
    const dockSide = () =>
      sides.find((s) => document.body.classList.contains(s)) || "none";
    if (dockSide() === "undocked" || dockSide() === "none") {
      if (!st.dock) {
        if (st.failed) return wait();
        if (!st.loading) {
          st.loading = true;
          import("./ui/legacy/legacy.js")
            .then((m) => {
              st.dock = m.DockController.DockController.instance();
            })
            .catch(() => {
              st.failed = true;
            });
        }
        return wait();
      }
      // The frontend only knows right, bottom and left; top is bottom with the sides swapped below.
      st.dock.setDockSide(cfg.position === "top" ? "bottom" : cfg.position);
      st.pos = null;
    }

    const wrapper = app.deviceModeView;
    let on = false;
    try {
      on = Boolean(
        wrapper && wrapper.isDeviceModeOn && wrapper.isDeviceModeOn(),
      );
    } catch (e) {}
    if (!on) {
      try {
        const model =
          st.emu && st.emu.DeviceModeModel.DeviceModeModel.instance();
        on = Boolean(model && model.enabledSetting().get());
      } catch (e) {}
    }
    if (!on) {
      const bar = document.querySelector(".device-mode-toolbar");
      on = Boolean(bar && bar.offsetParent);
    }

    // Expand right away on a click that turns device mode on; drop it if nothing confirms in 1.5s.
    const wantFull = on || (!st.from && Date.now() - st.want < 1500);
    const full =
      Math.abs(window.innerWidth - cfg.width) <= 4 &&
      Math.abs(window.innerHeight - cfg.height) <= 4;

    const changed = st.pos !== cfg.position || st.resp !== wantFull;
    const mode = split.showMode && split.showMode();
    if (
      changed ||
      (mode !== undefined && mode !== (wantFull ? "Both" : "OnlySidebar"))
    ) {
      split.setVertical(cfg.position === "left" || cfg.position === "right");
      split.setSecondIsSidebar(
        cfg.position === "right" || cfg.position === "bottom",
      );
      if (wantFull) split.showBoth();
      else split.hideMain();
    }
    st.pos = cfg.position;

    // The split clamps the sidebar while the WebView is still panel-sized, so re-apply the
    // panel size for a few ticks once the WebView has really grown. After that the user can
    // drag the frontend's own splitter freely.
    if (changed || !wantFull || !full) st.settle = 0;
    if (wantFull && full && st.settle < 6) {
      st.settle++;
      split.setSidebarSize(cfg.size);
    }
    st.resp = wantFull;
    st.lastOn = on;

    let bounds = null;
    const el =
      app.inspectedPagePlaceholder && app.inspectedPagePlaceholder.element;
    if (on && el && el.isConnected) {
      const r = el.getBoundingClientRect();
      bounds = [
        Math.round(r.left),
        Math.round(r.top),
        Math.round(r.width),
        Math.round(r.height),
      ];
    }
    return {
      ready: true,
      responsive: wantFull,
      bounds,
      iw: window.innerWidth,
      ih: window.innerHeight,
      icons_key: st.iconsKey || 0,
      icon_misses: [...(st.misses || [])],
    };
  } catch (e) {
    return wait();
  }
};
