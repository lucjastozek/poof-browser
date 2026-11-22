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
