/* ---------- history view (Ctrl+H) ----------
   Uses the helpers from scripts/core (el, ib, ic, $, send, hostOf, dayLabel), the global
   `ov` (the currently open full-area view) and `S` (last pushed state). Styled with the tokens
   from styles/foundation/theme.css and the shared .ib/.btn/.inp/.field/.ico classes, and sits exactly where #stage does. */
(() => {
  const RETENTION = [
    [0, "Keep forever"],
    [24, "Clear after 1 day"],
    [48, "Clear after 2 days"],
    [72, "Clear after 3 days"],
    [168, "Clear after 1 week"],
    [336, "Clear after 2 weeks"],
  ];

  const css = document.createElement("style");
  css.textContent = `
    #history-panel{position:fixed;z-index:50;left:calc(var(--sidebar-width,280px) + 8px);top:40px;right:8px;bottom:8px;
      display:flex;flex-direction:column;overflow:hidden;border-radius:8px;background:var(--stage);
      border:1px solid var(--line);color:var(--ink);font-family:var(--f-text);animation:fade .18s ease}
    body.compact #history-panel{left:64px}
    body.collapsed #history-panel{left:8px}
    #history-panel[hidden]{display:none}
    #history-panel .h-top{flex:none;padding:28px 32px 14px}
    #history-panel .h-wrap{max-width:860px;margin:0 auto;width:100%}
    #history-panel .h-head{display:flex;align-items:center;gap:12px;margin-bottom:16px}
    #history-panel h1{flex:1;margin:0;font:600 24px/1.2 var(--f-display);letter-spacing:-.01em}
    #history-panel .h-tools{display:flex;gap:8px;align-items:center}
    #history-panel .h-keep{height:34px;padding:0 10px;border-radius:var(--r-md);background:var(--surface);
      border:1px solid var(--line);color:var(--ink-2);font:inherit;font-size:12px;font-weight:500;outline:0;cursor:pointer}
    #history-panel .h-keep:hover{border-color:var(--line-2);color:var(--ink)}
    #history-panel .h-keep:focus-visible{border-color:color-mix(in srgb,var(--space) 65%,var(--ink));
      box-shadow:0 0 0 3px color-mix(in srgb,var(--space) 30%,transparent)}
    #history-panel .h-keep option{background:var(--raised);color:var(--ink)}
    #history-panel .field{height:40px}
    #history-panel .h-scroll{flex:1;overflow:auto;padding:0 32px 28px}
    #history-panel .h-day{padding:20px 12px 8px;font-size:12px;font-weight:600;color:var(--ink-2);
      text-transform:none;letter-spacing:.01em}
    #history-panel .h-card{background:var(--surface);border:1px solid var(--line);border-radius:var(--r-lg);padding:4px}
    #history-panel .h-row{display:flex;align-items:center;gap:12px;height:40px;padding:0 8px 0 10px;
      border-radius:var(--r-md);cursor:pointer;transition:background .12s}
    #history-panel .h-row:hover,#history-panel .h-row:focus-visible{background:var(--hover);outline:0}
    #history-panel .h-row:active{background:var(--pressed)}
    #history-panel .h-time{width:58px;flex:none;font-size:12px;color:var(--ink-3);font-variant-numeric:tabular-nums}
    #history-panel .h-title{flex:0 1 auto;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-weight:500}
    #history-panel .h-host{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;
      font-size:12px;color:var(--ink-3)}
    #history-panel .h-row .ib{opacity:0;flex:none}
    #history-panel .h-row:hover .ib,#history-panel .h-row:focus-within .ib{opacity:1}
    #history-panel .h-empty{padding:72px 0;text-align:center;color:var(--ink-3)}
  `;
  document.head.append(css);

  let panel = null;
  let list = null;
  let search = null;
  let select = null;
  let clearBtn = null;
  let entries = [];
  let isOpen = false;
  let openedActive = null;
  let openedUrl = null;
  let searchTimer = 0;
  let clearTimer = 0;

  // Favicons the browser already knows about, keyed by host (taken from the new-tab suggestions).
  function faviconFor(host) {
    for (const s of (S && S.suggestions) || []) {
      if (s.favicon && hostOf(s.url) === host) return s.favicon;
    }
    return null;
  }

  function siteIcon(host) {
    const box = el("span", "ico");
    const src = faviconFor(host);
    if (src) {
      const img = el("img");
      img.src = src;
      img.alt = "";
      img.onerror = () => box.replaceChildren(host.charAt(0).toUpperCase());
      box.append(img);
    } else {
      box.textContent = host.charAt(0).toUpperCase();
    }
    return box;
  }

  function visit(h) {
    closeHistory(true);
    send({ cmd: "navigate", input: h.url });
  }

  function render() {
    if (!list) return;
    list.replaceChildren();
    if (!entries.length) {
      list.append(
        el(
          "div",
          "h-empty",
          search.value ? "No matching history." : "No history yet.",
        ),
      );
      return;
    }
    let card = null;
    let last = "";
    for (const h of entries) {
      const label = dayLabel(h.ts);
      if (label !== last) {
        last = label;
        list.append(el("div", "h-day", label));
        card = el("div", "h-card");
        list.append(card);
      }
      const host = hostOf(h.url);
      const r = el("div", "h-row");
      r.tabIndex = 0;
      r.title = h.url;
      const del = ib("x", "Remove from history", () =>
        send({ cmd: "delete_history", id: h.id }),
      );
      del.addEventListener("click", (e) => e.stopPropagation());
      r.append(
        el(
          "span",
          "h-time",
          new Date(h.ts * 1000).toLocaleTimeString(undefined, {
            hour: "numeric",
            minute: "2-digit",
          }),
        ),
        siteIcon(host),
        el("span", "h-title", h.title || h.url),
        el("span", "h-host", host),
        del,
      );
      r.onclick = () => visit(h);
      r.onkeydown = (e) => {
        if (e.key === "Enter" && e.target === r) {
          e.preventDefault();
          visit(h);
        }
      };
      card.append(r);
    }
  }

  function syncRetention() {
    const hours =
      (S && S.profile_cleanup && S.profile_cleanup.history_retention_hours) ||
      0;
    select.value = String(RETENTION.some(([h]) => h === hours) ? hours : 0);
  }

  function resetClear() {
    clearTimeout(clearTimer);
    clearBtn.classList.remove("sure");
    clearBtn.textContent = "Clear history";
  }

  function build() {
    if (panel) return;
    panel = el("div");
    panel.id = "history-panel";
    panel.hidden = true;
    panel.setAttribute("role", "dialog");
    panel.setAttribute("aria-label", "History");

    search = el("input");
    search.type = "text";
    search.placeholder = "Search history";
    search.spellcheck = false;
    search.autocomplete = "off";
    search.setAttribute("aria-label", "Search history");
    search.addEventListener("input", () => {
      clearTimeout(searchTimer);
      searchTimer = setTimeout(
        () => send({ cmd: "list_history", query: search.value }),
        150,
      );
    });
    const field = el("label", "field");
    field.append(ic("search", 16), search);

    select = el("select", "h-keep");
    select.title = "How long history is kept";
    for (const [hours, text] of RETENTION) {
      const o = el("option", null, text);
      o.value = String(hours);
      select.append(o);
    }
    select.addEventListener("change", () => {
      const c = (S && S.profile_cleanup) || {};
      send({
        cmd: "update_profile_cleanup",
        tab_retention_hours: c.tab_retention_hours || 48,
        cookie_retention_hours: c.cookie_retention_hours || 0,
        cache_retention_hours: c.cache_retention_hours || 0,
        history_retention_hours: Number(select.value),
      });
    });

    // Two clicks to clear, because confirm() dialogs are not available in this WebView.
    clearBtn = el("button", "btn danger sm", "Clear history");
    clearBtn.type = "button";
    clearBtn.onclick = () => {
      if (clearBtn.classList.contains("sure")) {
        resetClear();
        send({ cmd: "clear_history" });
        return;
      }
      clearBtn.classList.add("sure");
      clearBtn.textContent = "Click again to confirm";
      clearTimer = setTimeout(resetClear, 3000);
    };

    const head = el("div", "h-head");
    const tools = el("div", "h-tools");
    tools.append(
      select,
      clearBtn,
      ib("x", "Close history (Esc)", () => closeHistory(true)),
    );
    head.append(el("h1", null, "History"), tools);

    const top = el("div", "h-top");
    const topWrap = el("div", "h-wrap");
    topWrap.append(head, field);
    top.append(topWrap);

    const scroll = el("div", "h-scroll");
    list = el("div", "h-wrap");
    scroll.append(list);
    panel.append(top, scroll);
    document.body.append(panel);
  }

  function openHistory() {
    build();
    openedActive = S ? S.active : null;
    openedUrl = S ? S.url : null;
    search.value = "";
    resetClear();
    syncRetention();
    panel.hidden = false;
    isOpen = true;
    // Another full-area view already hides the page; otherwise hide it ourselves.
    if (!ov) send({ cmd: "overlay", open: true });
    send({ cmd: "list_history", query: "" });
    search.focus();
  }

  function closeHistory(notify) {
    if (!isOpen) return;
    isOpen = false;
    panel.hidden = true;
    if (notify && !ov) send({ cmd: "overlay", open: false });
  }

  window.__toggleHistory = () => (isOpen ? closeHistory(true) : openHistory());
  window.__setHistory = (rows) => {
    entries = Array.isArray(rows) ? rows : [];
    if (isOpen) render();
  };

  // If the tab or address changes underneath (tab clicked, address typed), the page is shown
  // again by the native side, so just drop the panel without touching the overlay.
  const baseSetState = window.__setState;
  window.__setState = (s) => {
    baseSetState(s);
    if (isOpen) {
      if (s.active !== openedActive || s.url !== openedUrl) closeHistory(false);
      else syncRetention();
    }
  };

  // Clicking anything in the sidebar leaves the history view.
  document.addEventListener(
    "pointerdown",
    (e) => {
      if (
        isOpen &&
        e.target.closest &&
        e.target.closest("#side") &&
        !e.target.closest("#side-resize")
      )
        closeHistory(true);
    },
    true,
  );
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && isOpen) {
      e.preventDefault();
      closeHistory(true);
    }
  });
})();
