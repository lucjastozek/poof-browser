// Gives the built-in PDF viewer the colour of the current space (the viewer's own CSS derives the rest).
(() => {
  if (
    location.protocol !== "poofpdf:" &&
    location.hostname !== "poofpdf.localhost"
  )
    return;
  const c = "__ACCENT__";
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(c.slice(i, i + 2), 16) / 255);
  const f = (v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
  const light = 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b) > 0.4;
  const set = () => {
    const s = document.documentElement.style;
    s.setProperty("--space", c);
    s.setProperty("--on-space", light ? "#050505" : "#fffbe6");
  };
  document.documentElement ? set() : addEventListener("DOMContentLoaded", set);
})();
