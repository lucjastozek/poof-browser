/* Saved spaces may hold older names (Lucide, or MingCute's "home_1_line" spelling). Resolve
   them to a key that exists in ICONS: exact name, then SPACE_ALIASES (Lucide names that
   MingCute spells differently, from icon-aliases.js), then a normalised spelling.
   Anything unknown falls back to "house". */
const spaceIconName = (n) => {
  if (typeof n !== "string") return "house";
  const k = n
    .toLowerCase()
    .replace(/_line$/, "")
    .replace(/_/g, "-");
  for (const c of [n, k]) {
    if (ICONS[c]) return c;
    const a = SPACE_ALIASES[c];
    if (a && ICONS[a]) return a;
  }
  return "house";
};
/* Icons named in the UI sources are embedded; the rest of MingCute (~2 MB unpacked) is not
   unpacked at startup. The gzipped pack in ICONS_PACKED is unpacked only when it is needed: when the space editor opens, or when a saved space uses an icon that is not in
   the small embedded set. Once nothing needs it any more, __trimSpaceIcons drops everything
   except the embedded icons and the ones spaces actually use, so the memory goes back. */
const BASE_ICON_KEYS = new Set(Object.keys(ICONS));
let packLoaded = false,
  packWait = null;
window.__loadSpaceIcons = () => {
  if (packLoaded) return Promise.resolve();
  if (packWait) return packWait;
  packWait = (async () => {
    try {
      if (typeof ICONS_PACKED === "string" && window.DecompressionStream) {
        const bin = Uint8Array.from(atob(ICONS_PACKED), (ch) =>
          ch.charCodeAt(0),
        );
        const stream = new Blob([bin])
          .stream()
          .pipeThrough(new DecompressionStream("gzip"));
        const all = JSON.parse(await new Response(stream).text());
        for (const k in all) if (!(k in ICONS)) ICONS[k] = all[k];
        packLoaded = true;
      }
    } catch (e) {
      console.warn("space icons failed to unpack", e);
    }
    packWait = null;
  })();
  return packWait;
};
window.__trimSpaceIcons = () => {
  if (!packLoaded) return;
  if (typeof ov !== "undefined" && ov && ov.startsWith("space")) return;
  const keep = new Set();
  if (typeof S !== "undefined" && S)
    for (const sp of S.spaces) keep.add(spaceIconName(sp.icon));
  for (const k of Object.keys(ICONS))
    if (!BASE_ICON_KEYS.has(k) && !keep.has(k)) delete ICONS[k];
  packLoaded = false;
};
/* Spaces whose icon is not embedded show the "house" fallback until the pack is loaded. */
const unknownIcons = new Set();
function ensureSpaceIcons(s) {
  if (packLoaded || packWait) return;
  const lacking = (sp) =>
    typeof sp.icon === "string" &&
    !sp.icon.startsWith("svg:") &&
    sp.icon !== "house" &&
    !unknownIcons.has(sp.icon) &&
    spaceIconName(sp.icon) === "house";
  if (!s.spaces.some(lacking)) return;
  window.__loadSpaceIcons().then(() => {
    // Names that stay unresolved even with the full set are not looked up again.
    if (S)
      for (const sp of S.spaces) if (lacking(sp)) unknownIcons.add(sp.icon);
    if (S && window.__setState) window.__setState(S);
    // Keep the icons spaces use; release the rest.
    window.__trimSpaceIcons();
  });
}
