/* ---------- motion: poof on close, and rows that glide instead of jump ----------
   The native side removes a closed tab and the sidebar redraws the list at once. Two things hide
   that: a short "poof" of a ghost where the row was, and a FLIP pass that slides the rows below it
   up (the same pass animates reordering, pinning and opening). Both are skipped when the person
   asks for reduced motion. */
(() => {
  const reduce = matchMedia("(prefers-reduced-motion: reduce)");
  const side = $("side");
  const ROWS =
    "#pinned .row, #pinned .folder-head, #tabs .row, #tabs .folder-head";
  const rowsNow = () =>
    [...document.querySelectorAll(ROWS)].filter((n) => !n.closest("#swipe"));
  const keyOf = (n) => (window.__navKeyOf ? window.__navKeyOf(n) : null);
  const EASE_OUT = "cubic-bezier(0.16, 1, 0.3, 1)";

  function poof(id) {
    if (reduce.matches) return;
    const row = rowsNow().find((r) => r.dataset.id === id);
    if (!row) return;
    const sr = side.getBoundingClientRect(),
      r = row.getBoundingClientRect();
    const ghost = row.cloneNode(true);
    ghost.removeAttribute("id");
    ghost.querySelectorAll("[id]").forEach((n) => n.removeAttribute("id"));
    ghost.setAttribute("aria-hidden", "true");
    ghost.removeAttribute("role");
    ghost.tabIndex = -1;
    ghost.classList.add("poof-ghost");
    Object.assign(ghost.style, {
      left: r.left - sr.left + "px",
      top: r.top - sr.top + "px",
      width: r.width + "px",
      height: r.height + "px",
    });
    side.append(ghost);
    ghost
      .animate(
        [
          { opacity: 1, transform: "scale(1)", filter: "blur(0px)" },
          {
            opacity: 0,
            transform: "scale(0.9) translateX(-6px)",
            filter: "blur(2px)",
          },
        ],
        {
          duration: 150,
          easing: "cubic-bezier(0.4, 0, 1, 1)",
          fill: "forwards",
        },
      )
      .finished.then(
        () => ghost.remove(),
        () => ghost.remove(),
      );
    // A few specks fly out from the favicon.
    const cx = r.left - sr.left + 22,
      cy = r.top - sr.top + r.height / 2;
    for (let i = 0; i < 6; i++) {
      const dot = el("span", "poof-dot");
      const a = (Math.PI * 2 * i) / 6 + Math.random() * 0.8;
      const d = 14 + Math.random() * 14;
      dot.style.left = cx - 2 + "px";
      dot.style.top = cy - 2 + "px";
      side.append(dot);
      dot
        .animate(
          [
            { opacity: 0.9, transform: "translate(0,0) scale(1)" },
            {
              opacity: 0,
              transform: `translate(${Math.cos(a) * d}px, ${Math.sin(a) * d}px) scale(0.3)`,
            },
          ],
          {
            duration: 150,
            easing: "cubic-bezier(0.2, 0.7, 0.2, 1)",
            fill: "forwards",
          },
        )
        .finished.then(
          () => dot.remove(),
          () => dot.remove(),
        );
    }
  }
  window.__poof = poof;
  window.__closeTab = (id) => {
    poof(id);
    send({ cmd: "close_tab", id });
  };
  window.__clearTabs = () => {
    document
      .querySelectorAll("#tabs > .row")
      .forEach((r, i) => i < 8 && r.dataset.id && poof(r.dataset.id));
    send({ cmd: "clear_tabs" });
  };

  /* FLIP: measure, let the redraw happen, then slide each row from where it was. */
  const base = window.__setState;
  window.__setState = (s) => {
    const prev = S;
    const animate =
      prev &&
      s.current === prev.current &&
      !reduce.matches &&
      !side.classList.contains("swiping");
    let before = null;
    if (animate) {
      before = new Map();
      for (const n of rowsNow())
        before.set(keyOf(n), n.getBoundingClientRect().top);
    }
    base(s);
    if (!before || before.size === 0) return;
    let fresh = 0;
    const rows = rowsNow();
    for (const n of rows) {
      const old = before.get(keyOf(n));
      if (old === undefined) {
        fresh++;
        continue;
      }
      const dy = old - n.getBoundingClientRect().top;
      if (Math.abs(dy) > 1 && Math.abs(dy) < 480)
        n.animate(
          [{ transform: `translateY(${dy}px)` }, { transform: "none" }],
          {
            duration: 200,
            easing: EASE_OUT,
          },
        );
    }
    // One or two new rows fade in; a bulk change (restore, space load) just appears.
    if (fresh > 0 && fresh <= 2)
      for (const n of rows)
        if (!before.has(keyOf(n)))
          n.animate(
            [
              { opacity: 0, transform: "translateY(-4px)" },
              { opacity: 1, transform: "none" },
            ],
            { duration: 180, easing: EASE_OUT },
          );
  };
})();
