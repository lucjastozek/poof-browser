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
    w.replaceChildren(ic("earth", 16));
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
