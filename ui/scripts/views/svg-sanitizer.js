/* Reduces an uploaded SVG to simple shapes and returns the cleaned markup. Throws when the
   file is not an SVG, or is still unsafe or too large after cleaning. */
function sanitizeSvg(source) {
  const doc = new DOMParser().parseFromString(source, "image/svg+xml");
  if (
    doc.querySelector("parsererror") ||
    doc.documentElement.localName !== "svg"
  )
    throw Error("Invalid SVG");
  const allowed = new Set([
    "svg",
    "g",
    "path",
    "circle",
    "rect",
    "line",
    "polyline",
    "polygon",
    "ellipse",
  ]);
  const attrs = new Set([
    "viewBox",
    "xmlns",
    "d",
    "cx",
    "cy",
    "r",
    "x",
    "y",
    "width",
    "height",
    "rx",
    "ry",
    "x1",
    "y1",
    "x2",
    "y2",
    "points",
    "fill",
    "stroke",
    "stroke-width",
    "stroke-linecap",
    "stroke-linejoin",
    "fill-rule",
    "clip-rule",
  ]);
  const root = doc.documentElement;
  for (const node of [...root.querySelectorAll("*")].reverse()) {
    if (!allowed.has(node.localName)) node.remove();
    else
      for (const a of [...node.attributes])
        if (!attrs.has(a.name)) node.removeAttribute(a.name);
  }
  for (const a of [...root.attributes])
    if (!attrs.has(a.name)) root.removeAttribute(a.name);
  root.setAttribute("viewBox", root.getAttribute("viewBox") || "0 0 24 24");
  root.setAttribute("xmlns", "http://www.w3.org/2000/svg");
  const clean = new XMLSerializer().serializeToString(root);
  if (
    clean.length > 6144 ||
    /<script|javascript:|url\(|on[a-z]+=/i.test(clean)
  )
    throw Error("Unsafe or oversized SVG");
  return clean;
}
