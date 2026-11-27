/* The viewer: continuous scrolling pages, lazy rendering, zoom, rotation, links. */
"use strict";

pdfjsLib.GlobalWorkerOptions.workerSrc = "/vendor/pdf.worker.min.js";

const PAGE_GAP = 12;
const PAD = 16;
const ZOOM_MIN = 0.1;
const ZOOM_MAX = 8;
/* A canvas above this many pixels is rendered at lower resolution. */
const CANVAS_MAX_PIXELS = 24e6;
const RENDER_CONCURRENCY = 2;

const V = {
  doc: null, // {name, bytes, pdf, n, password}
  pages: [], // {num, el, w, h, top, rendered, want, busy, canvas, text, textDivs, textStr}
  tops: [],
  zoomMode: "width", // "width" | "page" | number
  scale: 1,
  rotation: 0,
  current: 1,
  token: 0,
  queue: [],
  active: 0,
  handlers: {},
  textCache: new Map(),
  on(evt, fn) {
    (this.handlers[evt] ||= []).push(fn);
  },
  emit(evt, ...a) {
    (this.handlers[evt] || []).forEach((fn) => fn(...a));
  },
};
const scrollBox = () => $("scroll");
const natural = (p) =>
  V.rotation % 180 ? { w: p.h, h: p.w } : { w: p.w, h: p.h };

/* ---------- open / close ---------- */
async function openDocument(bytes, name, source) {
  const task = pdfjsLib.getDocument({
    data: bytes.slice(),
    isEvalSupported: false,
    password: undefined,
  });
  let password = "";
  let cancelled = false;
  task.onPassword = async (update, reason) => {
    const wrong = reason === pdfjsLib.PasswordResponses.INCORRECT_PASSWORD;
    const pw = await askPassword(name, wrong);
    if (pw == null) {
      cancelled = true;
      return task.destroy();
    }
    password = pw;
    update(pw);
  };
  let pdf;
  try {
    pdf = await task.promise;
  } catch (e) {
    throw cancelled ? new Error("cancelled") : e;
  }
  closeDocument();
  V.doc = { name, bytes, pdf, n: pdf.numPages, password, source };
  V.rotation = 0;
  V.current = 1;
  document.title = name;
  buildPages();
  V.emit("doc", V.doc);
  loadSizes();
}
function closeDocument() {
  if (!V.doc) return;
  V.token++;
  V.queue = [];
  io.disconnect();
  far.disconnect();
  V.doc.pdf.destroy().catch(() => {});
  V.doc = null;
  V.pages = [];
  V.textCache.clear();
  $("pages").replaceChildren();
}

/* ---------- layout ---------- */
const io = new IntersectionObserver(
  (entries) => {
    for (const e of entries) {
      const p = e.target.__page;
      p.want = e.isIntersecting;
      if (p.want) enqueue(p);
    }
  },
  { root: document.getElementById("scroll"), rootMargin: "100% 0px" },
);
const far = new IntersectionObserver(
  (entries) => {
    for (const e of entries) if (!e.isIntersecting) release(e.target.__page);
  },
  { root: document.getElementById("scroll"), rootMargin: "400% 0px" },
);

function buildPages() {
  const holder = $("pages");
  io.disconnect();
  far.disconnect();
  holder.replaceChildren();
  V.pages = [];
  const first = { w: 612, h: 792 };
  for (let i = 1; i <= V.doc.n; i++) {
    const el = document.createElement("div");
    el.className = "pg";
    el.dataset.page = i;
    const p = {
      num: i,
      el,
      w: first.w,
      h: first.h,
      rendered: false,
      want: false,
      busy: false,
    };
    el.__page = p;
    V.pages.push(p);
    holder.append(el);
    if (typeof Ann !== "undefined") Ann.mount(p);
  }
  V.doc.pdf.getPage(1).then((page) => {
    const v = page.getViewport({ scale: 1 });
    V.pages.forEach((p) => ((p.w = v.width), (p.h = v.height)));
    relayout();
    V.emit("sizes");
    V.pages.forEach((p) => (io.observe(p.el), far.observe(p.el)));
  });
}

/* Real page sizes arrive in the background; pages are laid out again when any differ. */
async function loadSizes() {
  const doc = V.doc;
  for (let start = 1; start <= doc.n; start += 24) {
    const batch = [];
    for (let i = start; i < start + 24 && i <= doc.n; i++)
      batch.push(doc.pdf.getPage(i));
    const pages = await Promise.all(batch).catch(() => null);
    if (V.doc !== doc || !pages) return;
    let changed = false;
    pages.forEach((page, k) => {
      const v = page.getViewport({ scale: 1 });
      const p = V.pages[start - 1 + k];
      if (
        p &&
        (Math.abs(p.w - v.width) > 0.5 || Math.abs(p.h - v.height) > 0.5)
      ) {
        p.w = v.width;
        p.h = v.height;
        changed = true;
      }
    });
    if (changed) {
      relayout(true);
      V.emit("sizes");
    }
  }
}

function fitScale(mode) {
  const box = scrollBox();
  const widest = Math.max(...V.pages.map((p) => natural(p).w));
  const byWidth = (box.clientWidth - PAD * 2) / widest;
  if (mode === "width") return byWidth;
  const n = natural(V.pages[clamp(V.current - 1, 0, V.pages.length - 1)]);
  return Math.min(byWidth, (box.clientHeight - PAD * 2) / n.h);
}

/* Sizes and positions every page. With `keep`, the page at the top of the view stays there. */
function relayout(keep) {
  if (!V.doc || !V.pages.length) return;
  const box = scrollBox();
  const anchor = keep ? anchorAt(box.scrollTop) : null;
  V.scale = clamp(
    typeof V.zoomMode === "number" ? V.zoomMode : fitScale(V.zoomMode),
    ZOOM_MIN,
    ZOOM_MAX,
  );
  let y = PAD;
  V.tops = V.pages.map((p) => {
    const n = natural(p);
    p.el.style.width = n.w * V.scale + "px";
    p.el.style.height = n.h * V.scale + "px";
    p.el.style.setProperty("--scale-factor", V.scale);
    const top = y;
    y += n.h * V.scale + PAGE_GAP;
    return top;
  });
  $("pages").style.height = y - PAGE_GAP + PAD + "px";
  if (anchor)
    box.scrollTop =
      V.tops[anchor.i] + anchor.f * natural(V.pages[anchor.i]).h * V.scale;
  V.emit("zoom", V.scale);
}
function anchorAt(scrollTop) {
  const i = pageAt(scrollTop);
  const h = natural(V.pages[i]).h * V.scale;
  return { i, f: clamp((scrollTop - V.tops[i]) / h, 0, 1) };
}
/* Index of the page under a scroll position (binary search over cached tops). */
function pageAt(y) {
  let lo = 0;
  let hi = V.tops.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (V.tops[mid] <= y) lo = mid;
    else hi = mid - 1;
  }
  return lo;
}

/* ---------- zoom, rotate, navigate ---------- */
let rerenderTimer = 0;
function setZoom(mode) {
  V.zoomMode =
    typeof mode === "number" ? clamp(mode, ZOOM_MIN, ZOOM_MAX) : mode;
  relayout(true);
  clearTimeout(rerenderTimer);
  rerenderTimer = setTimeout(rerenderAll, 90);
}
const zoomBy = (factor) => setZoom(V.scale * factor);
function rotateView(deg) {
  if (typeof Ann !== "undefined" && Ann.active)
    return toast("Finish editing before rotating the view.");
  V.rotation = (V.rotation + deg + 360) % 360;
  relayout(true);
  rerenderAll();
}
function rerenderAll() {
  V.token++;
  V.queue = [];
  for (const p of V.pages) if (p.rendered || p.want) p.stale = true;
  for (const p of V.pages) if (p.want) enqueue(p);
}
function goToPage(n, frac = 0) {
  if (!V.doc) return;
  const i = clamp(Math.round(n) - 1, 0, V.pages.length - 1);
  scrollBox().scrollTop =
    V.tops[i] - PAD + frac * natural(V.pages[i]).h * V.scale;
}
function updateCurrent() {
  if (!V.doc || !V.tops.length) return;
  const box = scrollBox();
  const i = pageAt(box.scrollTop + box.clientHeight * 0.35) + 1;
  if (i !== V.current) {
    V.current = i;
    V.emit("page", i);
  }
}
let scrollQueued = false;
function onScroll() {
  if (scrollQueued) return;
  scrollQueued = true;
  requestAnimationFrame(() => {
    scrollQueued = false;
    updateCurrent();
  });
}

/* ---------- rendering ---------- */
function enqueue(p) {
  if (p.busy || (p.rendered && !p.stale) || V.queue.includes(p)) return;
  V.queue.push(p);
  pump();
}
function pump() {
  while (V.active < RENDER_CONCURRENCY && V.queue.length) {
    // The page nearest the viewport goes first.
    const mid = scrollBox().scrollTop + scrollBox().clientHeight / 2;
    V.queue.sort(
      (a, b) =>
        Math.abs(V.tops[a.num - 1] - mid) - Math.abs(V.tops[b.num - 1] - mid),
    );
    const p = V.queue.shift();
    if (!p.want) continue;
    V.active++;
    renderPage(p)
      .catch((e) => console.warn("render failed", e))
      .finally(() => {
        V.active--;
        pump();
      });
  }
}

async function renderPage(p) {
  const doc = V.doc;
  const token = V.token;
  p.busy = true;
  try {
    const page = await doc.pdf.getPage(p.num);
    const rotation = (page.rotate + V.rotation) % 360;
    const viewport = page.getViewport({ scale: V.scale, rotation });
    let q = Math.min(window.devicePixelRatio || 1, 3);
    if (viewport.width * viewport.height * q * q > CANVAS_MAX_PIXELS) {
      q = Math.sqrt(CANVAS_MAX_PIXELS / (viewport.width * viewport.height));
    }
    const hi = page.getViewport({ scale: V.scale * q, rotation });
    const canvas = document.createElement("canvas");
    canvas.width = Math.floor(hi.width);
    canvas.height = Math.floor(hi.height);
    await page.render({
      canvasContext: canvas.getContext("2d", { alpha: false }),
      viewport: hi,
    }).promise;
    if (V.doc !== doc || token !== V.token) return;

    const content = await page.getTextContent();
    const layer = document.createElement("div");
    layer.className = "textLayer";
    const textDivs = [];
    const textStr = [];
    await pdfjsLib.renderTextLayer({
      textContentSource: content,
      container: layer,
      viewport,
      textDivs,
      textContentItemsStr: textStr,
    }).promise;
    const links = await buildLinks(page, viewport);
    if (V.doc !== doc || token !== V.token) return;

    p.el
      .querySelectorAll("canvas,.textLayer,.links")
      .forEach((n) => n.remove());
    p.el.prepend(canvas, layer, links);
    Object.assign(p, { textDivs, textStr, rendered: true, stale: false });
    V.emit("rendered", p);
  } finally {
    p.busy = false;
  }
}

function release(p) {
  if (!p || !p.rendered || p.want) return;
  p.el.querySelectorAll("canvas,.textLayer,.links").forEach((n) => n.remove());
  Object.assign(p, { rendered: false, textDivs: null, textStr: null });
}

/* Clickable areas for the page's own link annotations. */
async function buildLinks(page, viewport) {
  const box = document.createElement("div");
  box.className = "links";
  const annots = await page.getAnnotations({ intent: "display" });
  for (const a of annots) {
    if (a.subtype !== "Link") continue;
    const url = a.url || a.unsafeUrl;
    if (!(a.dest || a.action || /^(https?:|mailto:)/i.test(url || "")))
      continue;
    const [x1, y1, x2, y2] = viewport.convertToViewportRectangle(a.rect);
    const link = document.createElement("a");
    link.style.cssText = `left:${Math.min(x1, x2)}px;top:${Math.min(y1, y2)}px;width:${Math.abs(x2 - x1)}px;height:${Math.abs(y2 - y1)}px`;
    if (url) {
      link.href = url;
      link.title = url;
      link.target = "_blank";
      link.rel = "noopener noreferrer";
    } else {
      link.href = "#";
      link.onclick = (e) => {
        e.preventDefault();
        goToDestination(a.dest);
      };
    }
    box.append(link);
  }
  return box;
}

/* A named or explicit destination -> scroll there. */
async function goToDestination(dest) {
  if (!V.doc || !dest) return;
  try {
    const explicit =
      typeof dest === "string" ? await V.doc.pdf.getDestination(dest) : dest;
    if (!explicit) return;
    const ref = explicit[0];
    const index =
      typeof ref === "object" ? await V.doc.pdf.getPageIndex(ref) : ref;
    const p = V.pages[index];
    if (!p) return;
    const page = await V.doc.pdf.getPage(index + 1);
    const view = page.getViewport({ scale: 1 });
    const y = /^(XYZ|FitH|FitBH)$/.test(explicit[1]?.name)
      ? (explicit[3] ?? explicit[2])
      : null;
    const frac =
      y == null
        ? 0
        : clamp((page.view[3] - y) / (page.view[3] - page.view[1]), 0, 1);
    goToPage(index + 1, V.rotation === 0 && view.rotation === 0 ? frac : 0);
  } catch (e) {
    console.warn("bad destination", e);
  }
}

/* ---------- input ---------- */
function initViewerInput() {
  const box = scrollBox();
  box.addEventListener("scroll", onScroll, { passive: true });
  new ResizeObserver(() => {
    if (V.doc && typeof V.zoomMode === "string") {
      relayout(true);
      clearTimeout(rerenderTimer);
      rerenderTimer = setTimeout(rerenderAll, 120);
    }
  }).observe(box);
  // Ctrl + wheel and pinch zoom the document, not the whole window.
  box.addEventListener(
    "wheel",
    (e) => {
      if (!e.ctrlKey) return;
      e.preventDefault();
      zoomBy(Math.exp(-e.deltaY * 0.0025));
    },
    { passive: false },
  );
}
