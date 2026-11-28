function unloadBtn(fn) {
  const b = ib("close", "Put tab to sleep", fn);
  b.replaceChildren(ic("sleep", 16));
  return b;
}

function row(t, kind) {
  const r = el(
    "div",
    "row" +
      (S.active === t.id ? " on" : "") +
      (t.loaded ? "" : " idle") +
      (S.split && (S.split.a === t.id || S.split.b === t.id) ? " split" : ""),
  );
  r.dataset.id = t.id;
  r.append(icon(t), el("span", "ttl", t.title || pretty(t.url) || "New tab"));
  if (t.errors) {
    const e = el("span", "err-dot");
    e.title =
      t.errors === 1
        ? "1 problem on this page (console error or failed request)"
        : `${t.errors} problems on this page (console errors or failed requests)`;
    e.setAttribute("role", "img");
    e.setAttribute("aria-label", e.title);
    r.append(e);
  }
  if (t.dev) {
    const d = el("span", "dev-badge", "dev");
    d.title = "Local dev server: never auto-slept";
    r.append(d);
  }
  const a = el("span", "acts");
  a.append(
    ib("star", "Add to favorites", () => send({ cmd: "fav_tab", id: t.id })),
  );
  // A tab that is already asleep has nothing to put to sleep.
  if (t.loaded)
    a.append(unloadBtn(() => send({ cmd: "unload_tab", id: t.id })));
  a.append(
    ib("close", kind === "tab" ? "Close" : "Reset to pinned page", () =>
      window.__closeTab(t.id),
    ),
  );
  r.append(a);
  r.onclick = () => {
    if (noClick) return;
    go({ cmd: "select_tab", id: t.id });
  };
  r.onauxclick = (e) => {
    if (e.button === 1) window.__closeTab(t.id);
  };
  r.oncontextmenu = (e) => showRowMenu(e, t);
  r.onpointerdown = (e) => watchDrag(e, r, t, kind);
  return r;
}
