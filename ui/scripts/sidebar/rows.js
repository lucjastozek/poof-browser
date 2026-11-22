function minusIcon() {
  try {
    const n = ic("minus");
    if (
      n &&
      (n.childElementCount || (n.querySelector && n.querySelector("svg")))
    )
      return n;
  } catch (_) {}
  const NS = "http://www.w3.org/2000/svg";
  const s = document.createElementNS(NS, "svg");
  s.setAttribute("viewBox", "0 0 24 24");
  s.setAttribute("width", "15");
  s.setAttribute("height", "15");
  const p = document.createElementNS(NS, "path");
  p.setAttribute("d", "M5 12h14");
  p.setAttribute("stroke", "currentColor");
  p.setAttribute("stroke-width", "2");
  p.setAttribute("stroke-linecap", "round");
  p.setAttribute("fill", "none");
  s.append(p);
  return s;
}
function unloadBtn(fn) {
  const b = ib("x", "Unload tab", fn);
  b.replaceChildren(minusIcon());
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
    unloadBtn(() => send({ cmd: "unload_tab", id: t.id })),
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
