const SLEEP_ICON = {
  sleep:
    '<path fill="currentColor" d="M13.586 17H12c-.198-.013-.325-.033-.5-.134-.755-.436-.601-1.606.241-1.832L12 15h4c.271.018.511.096.707.293.404.404.359 1.005 0 1.414L14.414 19H16c.198.013.325.033.5.134.755.436.601 1.606-.241 1.832L16 21h-4c-.271-.018-.511-.096-.707-.293-.404-.404-.359-1.005 0-1.414Zm-7-7H4c-1.204-.079-1.357-1.672-.259-1.966L4 8h5c.271.018.511.096.707.293.404.404.359 1.005 0 1.414L6.414 13H9c1.204.079 1.357 1.672.259 1.966L9 15H4c-.271-.018-.511-.096-.707-.293-.404-.404-.359-1.005 0-1.414Zm11-5H13c-.639-.042-1.142-.603-.966-1.259.122-.453.515-.711.966-.741h7c.36.024.68.178.866.5.235.407.139.868-.159 1.207L15.414 10H20c.639.042 1.142.603.966 1.259-.122.453-.515.711-.966.741h-7c-.36-.024-.68-.178-.866-.5-.235-.407-.139-.868.159-1.207Z"/>',
};

function unloadBtn(fn) {
  const b = ib("x", "Put tab to sleep", fn);
  b.replaceChildren(svg(SLEEP_ICON, "sleep", 16));
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
  const a = el("span", "acts");
  a.append(
    ib("star", "Add to favorites", () => send({ cmd: "fav_tab", id: t.id })),
  );
  // A tab that is already asleep has nothing to put to sleep.
  if (t.loaded)
    a.append(unloadBtn(() => send({ cmd: "unload_tab", id: t.id })));
  a.append(
    ib("x", kind === "tab" ? "Close" : "Reset to pinned page", () =>
      send({ cmd: "close_tab", id: t.id }),
    ),
  );
  r.append(a);
  r.onclick = () => {
    if (noClick) return;
    go({ cmd: "select_tab", id: t.id });
  };
  r.onauxclick = (e) => {
    if (e.button === 1) send({ cmd: "close_tab", id: t.id });
  };
  r.oncontextmenu = (e) => showRowMenu(e, t);
  r.onpointerdown = (e) => watchDrag(e, r, t, kind);
  return r;
}
