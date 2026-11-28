/* ---------- a dragged tab has weight ----------
   The ghost follows the pointer on springs, so it trails a little and catches up. It pops up
   slightly when picked up, leans into the direction of travel, and its shadow grows with speed.
   Let go on a drop zone and it shrinks into the list; let go anywhere else (or Escape) and it
   springs back to the row it came from. */
function dragLift(d) {
  const g = d.ghost,
    ph = (d.ph = { x: 0, y: 0, vx: 0, vy: 0, sc: 1, init: false });
  g.style.left = "0";
  g.style.top = "0";
  g.style.transformOrigin = d.ox + "px 50%";
  const paint = () => {
    const speed = Math.hypot(ph.vx, ph.vy);
    const rot = Math.max(-7, Math.min(7, ph.vx / 260));
    g.style.transform = `translate3d(${ph.x}px,${ph.y}px,0) rotate(${rot}deg) scale(${ph.sc})`;
    g.style.setProperty("--lift", Math.min(1, speed / 1500).toFixed(3));
  };
  const follow = { k: 900, c: 46, eps: 0.05 };
  ph.sx = spring({
    ...follow,
    from: 0,
    to: 0,
    onUpdate: (x, v) => ((ph.x = x), (ph.vx = v), paint()),
  });
  ph.sy = spring({
    ...follow,
    from: 0,
    to: 0,
    onUpdate: (y, v) => ((ph.y = y), (ph.vy = v), paint()),
  });
  ph.ss = spring({
    ...BOUNCE,
    from: 1,
    to: 1.045,
    eps: 0.002,
    onUpdate: (s) => ((ph.sc = s), paint()),
  });
}
function dragFollow(d, x, y) {
  const ph = d.ph;
  if (!ph) return;
  if (!ph.init) {
    ph.init = true;
    ph.sx.stop();
    ph.sy.stop();
    ph.x = ph.sx.x = x;
    ph.y = ph.sy.x = y;
  }
  ph.sx.go(x);
  ph.sy.go(y);
}
function dragSettle(d, landed) {
  const g = d.ghost,
    ph = d.ph;
  const done = () => {
    g.remove();
    d.row.classList.remove("lift");
  };
  if (!ph) return done();
  ph.sx.stop();
  ph.sy.stop();
  ph.ss.stop();
  const row = d.row.isConnected ? d.row : null;
  if (landed || !row || reduceMotion.matches) {
    d.row.classList.remove("lift");
    const at = `translate3d(${ph.x}px,${ph.y}px,0)`;
    g.animate(
      [
        { opacity: 0.97, transform: `${at} scale(${ph.sc})` },
        { opacity: 0, transform: `${at} scale(0.9)` },
      ],
      { duration: 130, easing: "cubic-bezier(0.3, 0, 0.8, 0.15)" },
    ).finished.then(done, done);
    return;
  }
  // Back to where it came from; the row stays dimmed until the ghost arrives.
  const r = row.getBoundingClientRect();
  ph.sx.target = r.left;
  ph.sy.target = r.top + r.height / 2 - 17;
  ph.ss.target = 1;
  for (const s of [ph.sx, ph.sy]) Object.assign(s, BOUNCE, { eps: 0.1 });
  ph.sx.go();
  ph.sy.go();
  ph.ss.go();
  ph.sy.then(() => ph.sx.then(() => ph.ss.then(done)));
}
