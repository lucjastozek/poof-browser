/* ---------- mini player ----------
   Media still playing (or paused) in a tab you left gets a small strip above the footer:
   play/pause, and a click on the title returns to that tab. Fed by S.media. */
(() => {
  const box = el("div");
  box.id = "mini";
  box.hidden = true;
  $("foot").before(box);
  let painted = "";

  function row(m) {
    const r = el("div", "mini-row");
    const text = el("button", "mini-text");
    text.type = "button";
    text.title = "Go to this tab";
    text.append(
      el("span", "mini-title", m.title || pretty(m.url)),
      el("small", null, m.artist || pretty(m.url)),
    );
    text.onclick = () => go({ cmd: "select_tab", id: m.id });
    const lead = icon(m, true);
    const toggle = ib(
      m.playing ? "pause" : "play",
      m.playing ? "Pause" : "Play",
      () => send({ cmd: "media_toggle", id: m.id }),
    );
    r.append(lead, text, toggle);
    return r;
  }

  function paint(list) {
    const sig = JSON.stringify(list);
    if (sig === painted) return;
    painted = sig;
    box.hidden = !list.length;
    box.replaceChildren(...list.map(row));
  }

  const base = window.__setState;
  window.__setState = (s) => {
    base(s);
    paint(Array.isArray(s.media) ? s.media : []);
  };
})();
