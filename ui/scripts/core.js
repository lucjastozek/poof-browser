const $ = (id) => document.getElementById(id);
const send = (o) => window.ipc.postMessage(JSON.stringify(o));
const el = (t, c, x) => {
  const e = document.createElement(t);
  if (c) e.className = c;
  if (x != null) e.textContent = x;
  return e;
};
const NS = "http://www.w3.org/2000/svg";
const svg = (set, n, s = 16) => {
  const e = document.createElementNS(NS, "svg");
  [
    ["viewBox", "0 0 24 24"],
    ["width", s],
    ["height", s],
    // MingCute icons are filled shapes (each path carries fill="currentColor"), not strokes.
    ["fill", "currentColor"],
    ["aria-hidden", "true"],
  ].forEach(([k, v]) => e.setAttribute(k, v));
  e.innerHTML = set[n] || "";
  return e;
};
const ic = (n, s) => svg(ICONS, n, s);
const sic = (n, s) => {
  if (typeof n === "string" && n.startsWith("svg:")) {
    const i = el("img", "custom-space-icon");
    i.src = "data:image/svg+xml;base64," + n.slice(4);
    i.width = s;
    i.height = s;
    i.alt = "";
    return i;
  }
  return svg(SPACE_ICONS, spaceIconName(n), s);
};
/* Saved spaces may hold older names (Lucide, or MingCute's "home_1_line" spelling). Resolve
   them to a key that exists in SPACE_ICONS: exact name, then SPACE_ALIASES (Lucide names that
   MingCute spells differently, built by tools/gen-icons.mjs), then a normalised spelling.
   Anything unknown falls back to "house". */
const spaceIconName = (n) => {
  if (typeof n !== "string") return "house";
  const k = n
    .toLowerCase()
    .replace(/_line$/, "")
    .replace(/_/g, "-");
  for (const c of [n, k]) {
    if (SPACE_ICONS[c]) return c;
    const a = SPACE_ALIASES[c];
    if (a && SPACE_ICONS[a]) return a;
  }
  return "house";
};
/* Every MingCute icon is shipped gzipped in SPACE_ICONS_PACKED (the page itself must stay under
   WebView2's 2 MB NavigateToString limit). Unpack it in the background and merge it into
   SPACE_ICONS; the small embedded set works immediately. The space picker should read
   SPACE_ICONS when it opens, or await window.__spaceIconsReady first. */
window.__spaceIconsReady = (async () => {
  try {
    if (typeof SPACE_ICONS_PACKED !== "string" || !window.DecompressionStream)
      return;
    const bin = Uint8Array.from(atob(SPACE_ICONS_PACKED), (ch) =>
      ch.charCodeAt(0),
    );
    const stream = new Blob([bin])
      .stream()
      .pipeThrough(new DecompressionStream("gzip"));
    Object.assign(SPACE_ICONS, JSON.parse(await new Response(stream).text()));
    // Spaces that use an icon that was not embedded showed the "house" fallback until now.
    if (
      typeof S !== "undefined" &&
      S &&
      window.__setState &&
      S.spaces.some(
        (sp) => sp.icon !== "house" && spaceIconName(sp.icon) === "house",
      )
    )
      window.__setState(S);
  } catch (e) {
    console.warn("space icons failed to unpack", e);
  }
})();
const ib = (n, tip, fn) => {
  const b = el("button", "ib");
  b.title = tip;
  b.setAttribute("aria-label", tip);
  b.append(ic(n));
  b.onclick = (e) => {
    e.stopPropagation();
    fn();
  };
  return b;
};
const btn = (cls, txt, fn) => {
  const b = el("button", "btn " + cls, txt);
  if (fn) b.onclick = fn;
  return b;
};

/* ---------- colors ---------- */
/* The twelve logo colors from the palette. They read well on both beige and near-black. */
const PRESETS = [
  "#d64933",
  "#fa8334",
  "#ea9010",
  "#56c876",
  "#4fb561",
  "#6dc5e2",
  "#23b5d3",
  "#a288b1",
  "#8c4dc0",
  "#da4167",
  "#e87461",
  "#831f32",
];
const hex2rgb = (h) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));
const rgb2hex = (c) =>
  "#" + c.map((v) => Math.round(v).toString(16).padStart(2, "0")).join("");
function hsv2rgb(h, s, v) {
  const f = (n) => {
    const k = (n + h / 60) % 6;
    return (v - v * s * Math.max(0, Math.min(k, 4 - k, 1))) * 255;
  };
  return [f(5), f(3), f(1)];
}
function rgb2hsv([r, g, b]) {
  r /= 255;
  g /= 255;
  b /= 255;
  const mx = Math.max(r, g, b),
    mn = Math.min(r, g, b),
    d = mx - mn;
  let h = 0;
  if (d) {
    h =
      mx === r
        ? ((g - b) / d) % 6
        : mx === g
          ? (b - r) / d + 2
          : (r - g) / d + 4;
    h *= 60;
    if (h < 0) h += 360;
  }
  return [h, mx ? d / mx : 0, mx];
}
const lum = ([r, g, b]) => {
  const f = (c) => {
    c /= 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
};
const onColor = (h) => (lum(hex2rgb(h)) > 0.4 ? "#050505" : "#fffbe6");
function applyColor(c) {
  const r = document.documentElement.style;
  r.setProperty("--space", c);
  r.setProperty("--on-space", onColor(c));
}

/* ---------- light / dark ---------- */
const mq = matchMedia("(prefers-color-scheme: dark)");
let pref = "system";
function applyMode(p) {
  pref = p;
  document.documentElement.dataset.theme =
    p === "system" ? (mq.matches ? "dark" : "light") : p;
}
mq.addEventListener("change", () => applyMode(pref));
applyMode("system");

/* ---------- state ---------- */
let S = null,
  prevIdx = -1,
  ov = null,
  EXTS = [],
  lastKey = "",
  pending = null,
  drag = null,
  noClick = false;
const curSpace = () => S.spaces.find((x) => x.id === S.current);

/* Resolved favicon sources, keyed by their candidate list. The sidebar rebuilds every row on
   each state push; without this each rebuild re-ran the whole candidate chain
   (/favicon.ico, apple-touch-icon, Google's favicon service) and re-created Image objects.
   Failures are remembered for a few minutes so a dead icon is not retried on every render. */
const ICON_CACHE_MAX = 400;
const ICON_FAIL_TTL = 5 * 60 * 1000;
const iconCache = new Map();
function rememberIcon(key, src) {
  iconCache.delete(key);
  if (iconCache.size >= ICON_CACHE_MAX)
    iconCache.delete(iconCache.keys().next().value);
  iconCache.set(key, { src, t: Date.now() });
}
function icon(t, guess = true) {
  const w = el("span", "ico");
  const candidates = [];
  const add = (src) => {
    if (src && !candidates.includes(src)) candidates.push(src);
  };
  // Older versions persisted WebView2's tiny PNG snapshot as a data URI.
  // Prefer the source URL so lazy-loaded favourites do not keep showing it.
  if (t.favicon && !t.favicon.startsWith("data:image/")) add(t.favicon);
  try {
    const site = new URL(t.url);
    if (guess) {
      add(new URL("/favicon.ico", site).href);
      add(new URL("/apple-touch-icon.png", site).href);
      add(
        "https://www.google.com/s2/favicons?domain=" +
          encodeURIComponent(site.hostname) +
          "&sz=64",
      );
    }
  } catch (e) {}
  const showFallback = () => {
    w.replaceChildren(ic("globe", 16));
    w.setAttribute("aria-label", "Website");
  };
  if (!candidates.length) {
    showFallback();
    return w;
  }
  const key = candidates.join("|");
  const cached = iconCache.get(key);
  if (cached && cached.src === null && Date.now() - cached.t < ICON_FAIL_TTL) {
    showFallback();
    return w;
  }
  const i = new Image();
  i.referrerPolicy = "no-referrer";
  i.draggable = false;
  i.decoding = "async";
  if (cached && cached.src) {
    // Known-good source: one request (served from the HTTP cache), no chain.
    i.onerror = () => {
      iconCache.delete(key);
      showFallback();
    };
    w.appendChild(i);
    i.src = cached.src;
    return w;
  }
  let index = 0;
  i.onerror = () => {
    index += 1;
    if (index < candidates.length) i.src = candidates[index];
    else {
      rememberIcon(key, null);
      showFallback();
    }
  };
  i.onload = () => {
    if (!i.naturalWidth || !i.naturalHeight) i.onerror();
    else rememberIcon(key, candidates[index]);
  };
  w.appendChild(i);
  i.src = candidates[index];
  return w;
}
const pretty = (u) => {
  try {
    const x = new URL(u);
    return x.host.replace(/^www\./, "");
  } catch (e) {
    return u || "";
  }
};

/* A command that changes what is shown: close any full-area view first. */
function go(cmd) {
  if (ov) {
    ov = null;
    $("stage").hidden = true;
    send({ cmd: "overlay", open: false });
  }
  send(cmd);
}
function openView(name) {
  ov = name;
  $("stage").hidden = false;
  send({ cmd: "overlay", open: true });
  build(name);
}
function closeView() {
  applyColor(curSpace().color);
  if (!S || !S.active) {
    openView("newtab");
    return;
  }
  ov = null;
  $("stage").hidden = true;
  applyColor(curSpace().color);
  send({ cmd: "overlay", open: false });
}
