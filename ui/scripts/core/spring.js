/* ---------- springs ----------
   One small integrator behind every bit of motion that should feel physical: it carries the
   velocity of a gesture into the settle, so a flick keeps going and a slow drag just lands.
   Presets: SNAP (quick, a hair of overshoot), BOUNCE (playful), CRITICAL (no overshoot).
   With "reduce motion" a spring jumps straight to its target. */
const reduceMotion = matchMedia("(prefers-reduced-motion: reduce)");
const SNAP = { k: 520, c: 34 };
const BOUNCE = { k: 380, c: 20 };
const CRITICAL = { k: 320, c: 36 };

/* spring({ from, to, vel (units/s), k, c, eps, onUpdate(x, v), onDone }) -> handle with
   .go(to) retarget / restart, .then(fn) run fn when at rest, .stop(), and mutable k, c. */
function spring(o) {
  const s = {
    x: o.from,
    v: o.vel || 0,
    target: o.to,
    k: o.k ?? SNAP.k,
    c: o.c ?? SNAP.c,
    eps: o.eps ?? 0.01,
    raf: 0,
    last: 0,
    onUpdate: o.onUpdate,
    onDone: o.onDone || null,
  };
  const rest = () => {
    s.x = s.target;
    s.v = 0;
    s.last = 0;
    s.onUpdate(s.x, 0);
    const d = s.onDone;
    s.onDone = null;
    if (d) d();
  };
  const step = (t) => {
    s.raf = 0;
    const dt = Math.min(0.034, (t - (s.last || t)) / 1000);
    s.last = t;
    const n = Math.max(1, Math.ceil(dt / 0.004)),
      h = dt / n;
    for (let i = 0; i < n; i++) {
      s.v += (-s.k * (s.x - s.target) - s.c * s.v) * h;
      s.x += s.v * h;
    }
    if (Math.abs(s.x - s.target) < s.eps && Math.abs(s.v) < s.eps * 10)
      return rest();
    s.onUpdate(s.x, s.v);
    s.raf = requestAnimationFrame(step);
  };
  s.go = (to) => {
    if (to !== undefined) s.target = to;
    if (reduceMotion.matches) {
      cancelAnimationFrame(s.raf);
      s.raf = 0;
      rest();
    } else if (!s.raf) {
      s.last = 0;
      s.raf = requestAnimationFrame(step);
    }
    return s;
  };
  s.then = (fn) => {
    if (!s.raf) fn();
    else s.onDone = fn;
    return s;
  };
  s.stop = () => {
    cancelAnimationFrame(s.raf);
    s.raf = 0;
    s.last = 0;
  };
  return s.go();
}

/* A very quiet tick (a 40 ms sine blip), off unless the person turned it on. */
let feedbackCtx = null;
window.__feedback = () => {
  try {
    if (!S) return;
    if (S.feedback_sound) {
      feedbackCtx = feedbackCtx || new AudioContext();
      if (feedbackCtx.state === "suspended") feedbackCtx.resume();
      const t = feedbackCtx.currentTime;
      const osc = feedbackCtx.createOscillator(),
        gain = feedbackCtx.createGain();
      osc.frequency.setValueAtTime(1500, t);
      osc.frequency.exponentialRampToValueAtTime(650, t + 0.03);
      gain.gain.setValueAtTime(0.0001, t);
      gain.gain.exponentialRampToValueAtTime(0.03, t + 0.003);
      gain.gain.exponentialRampToValueAtTime(0.0001, t + 0.04);
      osc.connect(gain).connect(feedbackCtx.destination);
      osc.start(t);
      osc.stop(t + 0.05);
    }
  } catch (_) {}
};
