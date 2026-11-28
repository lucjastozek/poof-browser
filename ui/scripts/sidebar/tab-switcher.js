/* ---------- Ctrl+Tab tab switcher ---------- */
{
  const st = document.createElement("style");
  st.textContent = `
  .tabsw{position:fixed;inset:0;z-index:2147483000;display:flex;align-items:center;justify-content:center;
    background:rgba(0,0,0,.38);backdrop-filter:blur(10px);-webkit-backdrop-filter:blur(10px);
    animation:tabsw-fade .12s ease-out}
  .tabsw[hidden]{display:none}
  .tabsw-panel{max-width:min(920px,calc(100vw - 48px));padding:18px 18px 14px;border-radius:20px;
    background:color-mix(in srgb,var(--chrome) 90%,transparent);border:1px solid var(--line);
    box-shadow:0 24px 70px rgba(0,0,0,.35);animation:tabsw-pop .14s cubic-bezier(.2,.8,.2,1)}
  .tabsw-strip{display:flex;gap:12px;overflow-x:auto;padding:8px 6px 10px;scrollbar-width:none}
  .tabsw-strip::-webkit-scrollbar{display:none}
  .tabsw-card{flex:none;width:148px;cursor:pointer;border-radius:14px;padding:6px;
    transition:background .1s,transform .12s;outline:2px solid transparent;outline-offset:0}
  .tabsw-card:hover{background:var(--hover)}
  .tabsw-card.on{background:var(--pressed);outline-color:var(--space);transform:translateY(-2px) scale(1.03)}
  .tabsw-art{position:relative;height:92px;border-radius:10px;display:flex;align-items:center;justify-content:center;
    background:linear-gradient(160deg,color-mix(in srgb,var(--space) 22%,var(--chrome)),
      color-mix(in srgb,var(--space) 6%,var(--chrome)));
    border:1px solid var(--line);overflow:hidden}
  .tabsw-card.idle .tabsw-art{opacity:.6}
  .tabsw-art img{width:40px;height:40px;object-fit:contain;border-radius:9px}
  .tabsw-art .ltr{font:700 30px var(--f-display);color:var(--ink-2);text-transform:uppercase}
  .tabsw-pin{position:absolute;top:6px;right:6px;width:7px;height:7px;border-radius:50%;background:var(--space)}
  .tabsw-ttl{margin:8px 2px 0;font:600 12px var(--f-display);color:var(--ink);
    white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
  .tabsw-host{margin:1px 2px 0;font:500 11px var(--f-display);color:var(--ink-3);
    white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
  .tabsw-hint{margin-top:6px;text-align:center;font:500 11px var(--f-display);color:var(--ink-3)}
  @keyframes tabsw-fade{from{opacity:0}to{opacity:1}}
  @keyframes tabsw-pop{from{opacity:0;transform:scale(.97)}to{opacity:1;transform:none}}`;
  document.head.append(st);

  const layer = el("div", "tabsw");
  layer.hidden = true;
  layer.setAttribute("role", "dialog");
  layer.setAttribute("aria-label", "Switch tab");
  const panel = el("div", "tabsw-panel");
  const strip = el("div", "tabsw-strip");
  strip.setAttribute("role", "listbox");
  strip.setAttribute("aria-label", "Open tabs");
  const hint = el(
    "div",
    "tabsw-hint",
    "Keep holding Ctrl and press Tab to cycle · release to switch · Esc to cancel",
  );
  panel.append(strip, hint);
  layer.append(panel);
  document.body.append(layer);

  let timer = 0;
  const hostOf = (u) => {
    try {
      return new URL(u).host.replace(/^www\./, "");
    } catch (_) {
      return u || "";
    }
  };
  const paint = (i) => {
    [...strip.children].forEach((c, n) => {
      c.classList.toggle("on", n === i);
      c.setAttribute("aria-selected", String(n === i));
    });
    const c = strip.children[i];
    if (c) {
      c.scrollIntoView({ inline: "center", block: "nearest" });
      window.__announce &&
        window.__announce(
          c.querySelector(".tabsw-ttl").textContent + ", " + (i + 1) + " of " + strip.children.length,
        );
    }
  };
  window.__switcher = (p) => {
    strip.replaceChildren(
      ...p.cards.map((t) => {
        const c = el("div", "tabsw-card" + (t.loaded ? "" : " idle"));
        c.setAttribute("role", "option");
        const art = el("div", "tabsw-art");
        if (t.favicon) {
          const img = el("img");
          img.src = t.favicon;
          img.draggable = false;
          img.onerror = () => {
            img.replaceWith(el("span", "ltr", hostOf(t.url)[0] || "•"));
          };
          art.append(img);
        } else art.append(el("span", "ltr", hostOf(t.url)[0] || "•"));
        if (t.pinned) art.append(el("span", "tabsw-pin"));
        const title = t.title || pretty(t.url) || "New tab";
        const ttl = el("div", "tabsw-ttl", title);
        ttl.title = title;
        c.append(
          art,
          ttl,
          el(
            "div",
            "tabsw-host",
            hostOf(t.url) + (t.loaded ? "" : " · asleep"),
          ),
        );
        c.onclick = () => send({ cmd: "switcher_pick", id: t.id });
        return c;
      }),
    );
    layer.hidden = false;
    paint(p.index);
    clearInterval(timer);
    timer = setInterval(() => send({ cmd: "switcher_poll" }), 70);
  };
  window.__switcherSelect = paint;
  window.__switcherClose = () => {
    layer.hidden = true;
    clearInterval(timer);
    timer = 0;
    strip.replaceChildren();
  };
  layer.addEventListener("pointerdown", (e) => {
    if (e.target === layer) send({ cmd: "switcher_cancel" });
  });
  addEventListener(
    "keyup",
    (e) => {
      if (e.key === "Control" && !layer.hidden)
        send({ cmd: "switcher_commit" });
    },
    true,
  );
  addEventListener(
    "keydown",
    (e) => {
      if (layer.hidden) return;
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        send({ cmd: "switcher_cancel" });
      } else if (e.key === "Enter") {
        e.preventDefault();
        send({ cmd: "switcher_commit" });
      }
    },
    true,
  );
}
