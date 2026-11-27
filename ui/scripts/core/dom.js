const $ = (id) => document.getElementById(id);
const send = (o) => window.ipc.postMessage(JSON.stringify(o));
const el = (t, c, x) => {
  const e = document.createElement(t);
  if (c) e.className = c;
  if (x != null) e.textContent = x;
  return e;
};
const NS = "http://www.w3.org/2000/svg";
const ic = (n, s = 16) => {
  const e = document.createElementNS(NS, "svg");
  [
    ["viewBox", "0 0 24 24"],
    ["width", s],
    ["height", s],
    // MingCute icons are filled shapes (each path carries fill="currentColor"), not strokes.
    ["fill", "currentColor"],
    ["aria-hidden", "true"],
  ].forEach(([k, v]) => e.setAttribute(k, v));
  if (!ICONS[n]) console.warn("icon not embedded:", n);
  e.innerHTML = ICONS[n] || "";
  return e;
};
const sic = (n, s) => {
  if (typeof n === "string" && n.startsWith("svg:")) {
    const i = el("img", "custom-space-icon");
    i.src = "data:image/svg+xml;base64," + n.slice(4);
    i.width = s;
    i.height = s;
    i.alt = "";
    return i;
  }
  return ic(spaceIconName(n), s);
};
