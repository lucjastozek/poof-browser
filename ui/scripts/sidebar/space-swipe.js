// Two-finger horizontal swipe over the sidebar, like Arc. While swiping, every space is drawn as
// a slide (its favorites, pinned tabs and tabs) in a carousel that follows the fingers one to
// one, so you see what you swipe through. When the fingers lift, the carousel settles on the
// nearest space at once and tells native code to switch; it hands over to the real sidebar
// once that has caught up, so the carousel never waits for the (slow) page switch.
//
// A touchpad swipe is a stream of wheel events, inertia included; there is no "fingers up"
// event, so silence for SWIPE_IDLE_MS means the swipe is over. The frame loop and its timers run
// only while a swipe is in progress.
const SWIPE_STEP = 0.9; // fraction of the sidebar width that one space takes
const SWIPE_IDLE_MS = 70; // silence that ends a swipe (touchpad events come every ~8-16ms)
const SWIPE_FLICK_MS = 60; // how far ahead the release velocity is projected
const SWIPE_FLICK_MAX = 0.35; // largest projected distance, in spaces
const SWIPE_TAU_MS = 45; // time constant of the settle (the smaller, the snappier)
const SWIPE_RUBBER = 0.35; // travel kept when pulling past the first / last space
const SWIPE_GIVE_UP_MS = 3000; // drop the carousel if native code never catches up
const SWIPE_SNAPS_MS = 1500; // how long a fetched set of space lists counts as fresh

let snaps = {}, // space id -> { favorites, pinned, tabs, folders }, from native code
  snapsAt = -SWIPE_SNAPS_MS,
  sw = null; // the swipe in progress (or settled and waiting for native code); else null

window.__setSnaps = (all) => {
  snaps = all || {};
  if (sw) {
    swipeBuild();
    swipeRender();
  }
};

// Asked ahead of time (pointer enters the sidebar) so the lists are there when a swipe starts.
function swipeAsk() {
  const now = performance.now();
  if (now - snapsAt < SWIPE_SNAPS_MS) return;
  snapsAt = now;
  send({ cmd: "space_snapshots" });
}
$("side").addEventListener("pointerenter", swipeAsk);

const swipeClamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v));
const swipeIndex = () => S.spaces.findIndex((x) => x.id === S.current);

// Linear mix of two #rrggbb colours; t is 0..1.
function mixHex(a, b, t) {
  const n = (h, i) => parseInt(h.slice(1 + i * 2, 3 + i * 2), 16);
  const c = (i) => Math.round(n(a, i) + (n(b, i) - n(a, i)) * t);
  return "#" + [0, 1, 2].map((i) => c(i).toString(16).padStart(2, "0")).join("");
}

// The lists of a space: live state for the current one, the fetched copy for the others.
function swipeView(sp) {
  if (sp.id === S.current) return S;
  const d = snaps[sp.id] || {};
  return {
    ...S,
    favorites: d.favorites || [],
    pinned: d.pinned || [],
    tabs: d.tabs || [],
    folders: d.folders || [],
    active: null,
    split: null,
  };
}

// A non-interactive copy of the sidebar's favorites + pane for one space.
function swipeSlide(sp) {
  const v = swipeView(sp);
  const favs = $("favs").cloneNode(false);
  favs.className = "";
  favs.replaceChildren(...v.favorites.map((t) => favTile(v, t)));
  favs.classList.toggle("empty", !v.favorites.length);

  const chipi = el("span", "chipi");
  chipi.append(sic(sp.icon, 15));
  const pen = el("span", "pen");
  pen.append(ic("pencil", 14));
  const hdr = $("hdr").cloneNode(false);
  hdr.replaceChildren(chipi, el("b", null, sp.name), pen);
  const pinned = $("pinned").cloneNode(false);
  pinned.replaceChildren(...folderArea(v, true));
  const tabs = $("tabs").cloneNode(false);
  tabs.replaceChildren(...folderArea(v, false));
  const pane = $("pane").cloneNode(false);
  pane.className = ""; // not the real pane's slide-in animation class
  pane.append(hdr, pinned, $("line").cloneNode(true), $("newtab").cloneNode(true), tabs);

  const slide = el("div", "gs");
  slide.append(favs, pane);
  return slide;
}

// Slides are drawn the first time they come into view, so a swipe starts with one slide of
// work, not one per space. Calling this again (fresh lists arrived) redraws them on demand.
function swipeBuild() {
  sw.box.replaceChildren();
  sw.slides = S.spaces.map(() => null);
}

function swipeSlideAt(k) {
  const slide = swipeSlide(S.spaces[k]);
  sw.box.append(slide);
  if (S.spaces[k].id === sw.startId) slide.lastChild.scrollTop = sw.scroll;
  return (sw.slides[k] = slide);
}

function swipeBegin() {
  const pane = $("pane"),
    favs = $("favs"),
    top = favs.offsetHeight ? favs.offsetTop : pane.offsetTop;
  const box = el("div");
  box.id = "swipe";
  Object.assign(box.style, {
    left: pane.offsetLeft + "px",
    top: top + "px",
    width: pane.offsetWidth + "px",
    height: pane.offsetTop + pane.offsetHeight - top + "px",
  });
  $("side").append(box);
  $("side").classList.add("swiping");
  const idx = swipeIndex();
  sw = {
    box,
    W: pane.offsetWidth,
    n: S.spaces.length,
    pos: idx, // position in spaces: 2.5 is halfway between the third and fourth space
    vel: 0, // spaces per millisecond
    last: 0, // time of the last wheel event
    t: 0, // time of the last frame
    raf: 0,
    settling: false,
    target: idx,
    sent: S.current, // the space native code was last asked to show
    startId: S.current, // the space the swipe began on
    scroll: pane.scrollTop, // its tab list's scroll position, kept by its slide
    dot: -1,
    giveUp: 0,
    slides: [],
  };
  swipeBuild();
  swipeRender();
}

function swipeRender() {
  const { pos, W, n } = sw;
  sw.slides.forEach((slide, k) => {
    const near = Math.abs(k - pos) < 1;
    if (!slide && near) slide = swipeSlideAt(k);
    if (!slide) return;
    slide.style.visibility = near ? "visible" : "hidden";
    slide.style.transform = `translate3d(${(k - pos) * W}px,0,0)`;
  });
  const a = swipeClamp(Math.floor(pos), 0, n - 1),
    b = Math.min(n - 1, a + 1);
  applyColor(mixHex(S.spaces[a].color, S.spaces[b].color, swipeClamp(pos - a, 0, 1)));
  const dot = swipeClamp(Math.round(pos), 0, n - 1);
  if (dot !== sw.dot) {
    sw.dot = dot;
    [...$("dots").children].forEach((d, i) => d.classList.toggle("on", i === dot));
  }
}

// Tells native code to show the target space, once per distinct target.
function swipeCommit() {
  const id = S.spaces[sw.target].id;
  if (id === sw.sent) return;
  sw.sent = id;
  go({ cmd: "switch_space", id, keep_focus: true });
}

function swipeTick(t) {
  if (!sw) return;
  sw.raf = 0;
  if (t - sw.last > SWIPE_IDLE_MS) {
    if (!sw.settling) {
      sw.settling = true;
      const ahead = swipeClamp(sw.vel * SWIPE_FLICK_MS, -SWIPE_FLICK_MAX, SWIPE_FLICK_MAX);
      sw.target = swipeClamp(Math.round(sw.pos + ahead), 0, sw.n - 1);
      swipeCommit(); // runs alongside the settle animation
    }
    const dt = Math.min(48, t - (sw.t || t));
    sw.pos += (sw.target - sw.pos) * (1 - Math.exp(-dt / SWIPE_TAU_MS));
    if (Math.abs(sw.target - sw.pos) < 0.003) {
      sw.pos = sw.target;
      swipeRender();
      swipeSettled();
      return;
    }
  }
  sw.t = t;
  swipeRender();
  sw.raf = requestAnimationFrame(swipeTick);
}

// The carousel rests on its target. If native code already shows that space, hand over now;
// otherwise stay (the carousel looks identical) until swipeOnState sees it arrive.
function swipeSettled() {
  if (S.spaces[sw.target].id === S.current) swipeEnd();
  else sw.giveUp = setTimeout(swipeEnd, SWIPE_GIVE_UP_MS);
}

// Called with every new state while a swipe is on screen.
function swipeOnState() {
  sw.dot = -1;
  // Parked on the target and native code now shows it: hand over. Otherwise the real sidebar
  // just re-applied its own colour and dots, so draw them again.
  if (sw.settling && !sw.raf && S.spaces[sw.target].id === S.current) swipeEnd();
  else swipeRender();
}

function swipeEnd() {
  cancelAnimationFrame(sw.raf);
  clearTimeout(sw.giveUp);
  sw.box.remove();
  $("side").classList.remove("swiping");
  if (S.current !== sw.startId) $("pane").scrollTop = 0; // a new space starts at its top
  const idx = swipeIndex();
  applyColor(S.spaces[idx].color);
  [...$("dots").children].forEach((d, i) => d.classList.toggle("on", i === idx));
  sw = null;
}

$("side").addEventListener(
  "wheel",
  (e) => {
    // Pinch-zoom arrives as ctrl+wheel; mouse wheels report lines; vertical belongs to the lists.
    // The start page is what an empty space shows, so it must not block swiping; other full-area
    // views (settings, space editor, ...) do. `go` closes the start page when the switch is sent.
    if (e.ctrlKey || e.deltaMode !== 0 || !S || drag || S.spaces.length < 2) return;
    if (ov && ov !== "newtab") return;
    if (document.body.classList.contains("compact") || document.body.classList.contains("collapsed")) return;
    const horizontal = Math.abs(e.deltaX) > Math.abs(e.deltaY);
    if (!sw && !horizontal) return;
    e.preventDefault(); // no horizontal overscroll in the sidebar
    if (!horizontal) return;
    swipeAsk();
    if (!sw) swipeBegin();
    clearTimeout(sw.giveUp);
    let d = e.deltaX / (sw.W * SWIPE_STEP);
    if ((sw.pos <= 0 && d < 0) || (sw.pos >= sw.n - 1 && d > 0)) d *= SWIPE_RUBBER;
    const dt = Math.max(1, e.timeStamp - sw.last);
    sw.vel = sw.vel * 0.6 + (d / dt) * 0.4;
    sw.pos = swipeClamp(sw.pos + d, -SWIPE_FLICK_MAX, sw.n - 1 + SWIPE_FLICK_MAX);
    sw.last = e.timeStamp;
    sw.settling = false;
    if (!sw.raf) sw.raf = requestAnimationFrame(swipeTick);
  },
  { passive: false },
);
