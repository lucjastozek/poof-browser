/*
 * Editing layer: text, highlights, drawings, shapes, images and signatures placed on pages.
 * Items live in page units (the page at 100%, top-left origin) so they survive zooming, and are
 * written into the PDF by annotate-export.js.
 */
"use strict";

const SVGNS = "http://www.w3.org/2000/svg";
const TEXT_BASELINE = 0.95; // first baseline, in font sizes below the top of a text box
const LINE_HEIGHT = 1.2;
const HANDLE = 5; // px of the resize handles
const SWATCHES = [
  "#111111",
  "#e0342b",
  "#f08a24",
  "#f5c518",
  "#2f9e44",
  "#1c7ed6",
  "#7048e8",
  "#ffffff",
];
/* Google Fonts offered in the editor (loaded in index.html) and the standard PDF font each is written with. */
const FONTS = [
  ["Arimo", "sans"],
  ["Inter", "sans"],
  ["Roboto", "sans"],
  ["Open Sans", "sans"],
  ["Lato", "sans"],
  ["Montserrat", "sans"],
  ["Source Sans 3", "sans"],
  ["Noto Sans", "sans"],
  ["Fira Sans", "sans"],
  ["Jost", "sans"],
  ["Carlito", "sans"],
  ["Tinos", "serif"],
  ["Source Serif 4", "serif"],
  ["Gelasio", "serif"],
  ["Caladea", "serif"],
  ["EB Garamond", "serif"],
  ["Libre Baskerville", "serif"],
  ["Crimson Pro", "serif"],
  ["Cousine", "mono"],
  ["Courier Prime", "mono"],
  ["Roboto Mono", "mono"],
  ["Inconsolata", "mono"],
];
const GENERIC = Object.fromEntries(FONTS);
const GENERIC_CSS = {
  sans: "Arial, Helvetica, sans-serif",
  serif: '"Times New Roman", Times, serif',
  mono: '"Courier New", Courier, monospace',
};
/* PDF font name -> closest Google font. Arimo, Tinos and Cousine are metric-compatible with Arial, Times and Courier New. */
const FONT_RULES = [
  [/courier ?new|cousine|liberation ?mono/, "Cousine"],
  [/courier|nimbus ?mon/, "Courier Prime"],
  [/consolas|inconsolata/, "Inconsolata"],
  [
    /menlo|monaco|lucida ?console|sf ?mono|roboto ?mono|dejavu ?sans ?mono/,
    "Roboto Mono",
  ],
  [/arial|helvetica|liberation ?sans|arimo|nimbus ?sans/, "Arimo"],
  [/times|liberation ?serif|tinos|nimbus ?rom/, "Tinos"],
  [/calibri|carlito/, "Carlito"],
  [/cambria|caladea/, "Caladea"],
  [/georgia/, "Gelasio"],
  [/garamond/, "EB Garamond"],
  [/baskerville/, "Libre Baskerville"],
  [/palatino|book ?antiqua|minion|crimson/, "Crimson Pro"],
  [/century|charter|utopia|bookman|source ?serif/, "Source Serif 4"],
  [/futura|jost/, "Jost"],
  [/montserrat|gotham|proxima/, "Montserrat"],
  [/roboto/, "Roboto"],
  [/lato/, "Lato"],
  [/source ?sans/, "Source Sans 3"],
  [/fira|trebuchet|gill/, "Fira Sans"],
  [/inter|sf ?pro|san ?francisco/, "Inter"],
  [/segoe|open ?sans|myriad|frutiger|univers|optima/, "Open Sans"],
  [/verdana|tahoma|noto|dejavu/, "Noto Sans"],
];
const cssFont = (i) =>
  `"${i.font || "Arimo"}", ${GENERIC_CSS[GENERIC[i.font] || "sans"]}`;
const fontShorthand = (i, px) =>
  `${i.italic ? "italic " : ""}${i.bold ? 700 : 400} ${px}px ${cssFont(i)}`;
const TEXT_KINDS = ["text", "replace"];
const ADD_TOOLS = [
  ["text", "textbox", "Text", "Add text: click the page and type"],
  ["highlight", "mark-pen", "Highlight", "Highlight an area"],
  ["draw", "pen", "Draw", "Draw freehand"],
];
const SHAPE_TOOLS = [
  ["rect", "square", "Rectangle"],
  ["ellipse", "ellipse", "Ellipse"],
  ["line", "line", "Line"],
  ["arrow", "arrow-right-up", "Arrow"],
];
const HINTS = {
  select: "Click an item you added to move, resize or restyle it.",
  edittext: "Click any text in the page to change its words, font or color.",
  editimage: "Click an image in the page, then replace or remove it.",
  text: "Click the page where the text should go.",
  highlight: "Drag over the area to highlight.",
  draw: "Draw freehand on the page.",
  rect: "Drag to draw a rectangle.",
  ellipse: "Drag to draw an ellipse.",
  line: "Drag to draw a line (Shift snaps the angle).",
  arrow: "Drag to draw an arrow.",
};

const Ann = {
  active: false,
  tool: "select",
  color: SWATCHES[0],
  width: 3,
  size: 16,
  font: "Arimo",
  bold: false,
  italic: false,
  pickImg: null, // an image of the page chosen with “Edit image”: {page, box}
  items: [],
  imgs: new Map(), // key -> dataURL
  sel: null,
  past: [],
  future: [],
  nextId: 1,
};
let drag = null;
const svgEl = (tag, attrs = {}) => {
  const n = document.createElementNS(SVGNS, tag);
  for (const k in attrs) n.setAttribute(k, attrs[k]);
  return n;
};
const measureCtx = document.createElement("canvas").getContext("2d");

/* ---------- model ---------- */
function textLines(item) {
  return String(item.text).split("\n");
}
function textBox(item) {
  measureCtx.font = fontShorthand(item, item.size);
  const w = Math.max(
    ...textLines(item).map((l) => measureCtx.measureText(l || " ").width),
  );
  return {
    x: item.x,
    y: item.y,
    w,
    h: textLines(item).length * item.size * LINE_HEIGHT,
  };
}
function bbox(item) {
  switch (item.type) {
    case "text":
      return textBox(item);
    case "line":
      return {
        x: Math.min(item.x1, item.x2),
        y: Math.min(item.y1, item.y2),
        w: Math.abs(item.x2 - item.x1),
        h: Math.abs(item.y2 - item.y1),
      };
    case "draw": {
      const xs = item.pts.map((p) => p[0]);
      const ys = item.pts.map((p) => p[1]);
      return {
        x: Math.min(...xs),
        y: Math.min(...ys),
        w: Math.max(...xs) - Math.min(...xs),
        h: Math.max(...ys) - Math.min(...ys),
      };
    }
    default:
      return { x: item.x, y: item.y, w: item.w, h: item.h };
  }
}
function snapshot() {
  Ann.past.push(JSON.stringify(Ann.items));
  if (Ann.past.length > 100) Ann.past.shift();
  Ann.future = [];
}
function restore(json) {
  Ann.items = JSON.parse(json);
  if (!Ann.items.some((i) => i.id === Ann.sel)) Ann.sel = null;
  drawAll();
  refreshBar();
}
function undo() {
  if (!Ann.past.length) return;
  Ann.future.push(JSON.stringify(Ann.items));
  restore(Ann.past.pop());
}
function redo() {
  if (!Ann.future.length) return;
  Ann.past.push(JSON.stringify(Ann.items));
  restore(Ann.future.pop());
}
function addItem(item) {
  snapshot();
  item.id = Ann.nextId++;
  Ann.items.push(item);
  return item;
}
function removeSelected() {
  if (Ann.sel == null) return;
  snapshot();
  const item = Ann.items.find((i) => i.id === Ann.sel);
  Ann.items = Ann.items.filter((i) => i.id !== Ann.sel);
  Ann.sel = null;
  if (item) draw(item.page);
  refreshBar();
}
const selected = () => Ann.items.find((i) => i.id === Ann.sel);

/* ---------- drawing ---------- */
Ann.mount = (p) => {
  const svg = svgEl("svg", {
    class: "ann",
    viewBox: `0 0 ${p.w} ${p.h}`,
    preserveAspectRatio: "none",
  });
  svg.dataset.page = p.num;
  svg.addEventListener("pointerdown", onDown);
  svg.addEventListener("pointermove", onMove);
  svg.addEventListener("pointerup", onUp);
  svg.addEventListener("pointercancel", onUp);
  svg.addEventListener("dblclick", onDouble);
  p.svg = svg;
  p.el.append(svg);
};
function drawAll() {
  V.pages.forEach((p) => draw(p.num));
}
function draw(num) {
  const p = V.pages[num - 1];
  if (!p || !p.svg) return;
  p.svg.replaceChildren();
  for (const item of Ann.items)
    if (item.page === num && (!item.editing || item.type === "replace"))
      p.svg.append(shapeFor(item));
  const hover = Ann.hover;
  if (hover && hover.run && hover.page === num && Ann.tool === "edittext") {
    const r = runBox(hover.run);
    p.svg.append(
      svgEl("rect", {
        class: "hover",
        x: r.x - 1,
        y: r.y,
        width: r.w + 2,
        height: r.h,
      }),
    );
  }
  const hi = Ann.hover;
  if (hi && hi.page === num && hi.box && Ann.tool === "editimage")
    p.svg.append(
      svgEl("rect", {
        class: "hover",
        x: hi.box.x,
        y: hi.box.y,
        width: hi.box.w,
        height: hi.box.h,
      }),
    );
  const pk = Ann.pickImg;
  if (pk && pk.page === num && Ann.tool === "editimage")
    p.svg.append(
      svgEl("rect", {
        class: "sel-box",
        x: pk.box.x - 2,
        y: pk.box.y - 2,
        width: pk.box.w + 4,
        height: pk.box.h + 4,
      }),
    );
  const s = selected();
  if (s && s.page === num && Ann.active) p.svg.append(selectionFor(s));
}
function shapeFor(item) {
  const common = {
    "data-id": item.id,
    stroke: item.color,
    "stroke-width": item.width,
    fill: "none",
    "stroke-linecap": "round",
    "stroke-linejoin": "round",
  };
  const g = svgEl("g", { "data-id": item.id });
  switch (item.type) {
    case "text": {
      const t = svgEl("text", {
        x: item.x,
        y: item.y,
        fill: item.color,
        "font-size": item.size,
        "font-family": cssFont(item),
        "font-weight": item.bold ? 700 : 400,
        "font-style": item.italic ? "italic" : "normal",
        "data-id": item.id,
        "xml:space": "preserve",
      });
      textLines(item).forEach((line, i) => {
        const span = svgEl("tspan", {
          x: item.x,
          y: item.y + item.size * (TEXT_BASELINE + i * LINE_HEIGHT),
        });
        span.textContent = line || " ";
        t.append(span);
      });
      g.append(t);
      break;
    }
    case "replace": {
      g.append(
        svgEl("rect", {
          x: item.x - 1,
          y: item.y,
          width: item.w + 2,
          height: item.h,
          fill: item.bg,
          "data-id": item.id,
        }),
      );
      if (item.editing) break;
      const t = svgEl("text", {
        x: item.x,
        y: item.base,
        fill: item.color,
        "font-size": item.size,
        "font-family": cssFont(item),
        "font-weight": item.bold ? 700 : 400,
        "font-style": item.italic ? "italic" : "normal",
        "data-id": item.id,
        "xml:space": "preserve",
      });
      t.textContent = item.text || " ";
      g.append(t);
      break;
    }
    case "cover":
      g.append(
        svgEl("rect", {
          x: item.x,
          y: item.y,
          width: item.w,
          height: item.h,
          fill: item.bg,
          "data-id": item.id,
        }),
      );
      break;
    case "highlight":
      g.append(
        svgEl("rect", {
          x: item.x,
          y: item.y,
          width: item.w,
          height: item.h,
          fill: item.color,
          "fill-opacity": 0.35,
          style: "mix-blend-mode:multiply",
          "data-id": item.id,
        }),
      );
      break;
    case "rect":
      g.append(
        svgEl("rect", {
          ...common,
          x: item.x,
          y: item.y,
          width: item.w,
          height: item.h,
          fill: "transparent",
        }),
      );
      break;
    case "ellipse":
      g.append(
        svgEl("ellipse", {
          ...common,
          cx: item.x + item.w / 2,
          cy: item.y + item.h / 2,
          rx: item.w / 2,
          ry: item.h / 2,
          fill: "transparent",
        }),
      );
      break;
    case "line":
    case "arrow": {
      g.append(
        svgEl("line", {
          ...common,
          "stroke-width": Math.max(item.width, 12),
          stroke: "transparent",
          x1: item.x1,
          y1: item.y1,
          x2: item.x2,
          y2: item.y2,
        }),
      );
      g.append(
        svgEl("line", {
          ...common,
          x1: item.x1,
          y1: item.y1,
          x2: item.x2,
          y2: item.y2,
        }),
      );
      if (item.type === "arrow")
        for (const [x, y] of arrowHead(item))
          g.append(
            svgEl("line", {
              ...common,
              x1: item.x2,
              y1: item.y2,
              x2: x,
              y2: y,
            }),
          );
      break;
    }
    case "draw":
      g.append(
        svgEl("polyline", {
          ...common,
          points: item.pts.map((q) => q.join(",")).join(" "),
        }),
      );
      break;
    case "image":
      g.append(
        svgEl("image", {
          href: Ann.imgs.get(item.img),
          x: item.x,
          y: item.y,
          width: item.w,
          height: item.h,
          preserveAspectRatio: "none",
          "data-id": item.id,
        }),
      );
      break;
  }
  return g;
}
/* The two barbs of an arrow head, as end points. */
function arrowHead(item) {
  const angle = Math.atan2(item.y2 - item.y1, item.x2 - item.x1);
  const len = Math.max(10, item.width * 4);
  return [0.5, -0.5].map((d) => [
    item.x2 - len * Math.cos(angle - d),
    item.y2 - len * Math.sin(angle - d),
  ]);
}
function selectionFor(item) {
  const g = svgEl("g", { class: "sel" });
  const b = bbox(item);
  g.append(
    svgEl("rect", {
      x: b.x - 2,
      y: b.y - 2,
      width: b.w + 4,
      height: b.h + 4,
      class: "sel-box",
    }),
  );
  const k = 1 / (V.scale || 1);
  const handle = (name, x, y) =>
    g.append(
      svgEl("rect", {
        class: "sel-h",
        "data-handle": name,
        x: x - HANDLE * k,
        y: y - HANDLE * k,
        width: HANDLE * 2 * k,
        height: HANDLE * 2 * k,
      }),
    );
  if (item.type === "line" || item.type === "arrow") {
    handle("ep1", item.x1, item.y1);
    handle("ep2", item.x2, item.y2);
  } else if (!["text", "draw", "replace"].includes(item.type))
    handle("br", b.x + b.w, b.y + b.h);
  return g;
}

/* ---------- pointer interaction ---------- */
function pagePoint(e, svg) {
  const r = svg.getBoundingClientRect();
  const p = V.pages[+svg.dataset.page - 1];
  return {
    x: ((e.clientX - r.left) / r.width) * p.w,
    y: ((e.clientY - r.top) / r.height) * p.h,
  };
}
function onDown(e) {
  if (!Ann.active || e.button !== 0) return;
  const svg = e.currentTarget;
  const page = +svg.dataset.page;
  const pt = pagePoint(e, svg);
  const handle = e.target.dataset?.handle;
  const hit = e.target.closest?.("[data-id]");
  const item = hit ? Ann.items.find((i) => i.id === +hit.dataset.id) : null;
  svg.setPointerCapture(e.pointerId);
  e.preventDefault();
  if (handle && selected()) {
    snapshot();
    drag = { kind: handle, page, start: pt, item: selected() };
    return;
  }
  if (Ann.tool === "select") {
    const prev = Ann.sel;
    Ann.sel = item ? item.id : null;
    if (prev !== Ann.sel) {
      const old = prev && Ann.items.find((i) => i.id === prev);
      if (old) draw(old.page);
      refreshBar();
    }
    draw(page);
    if (item) {
      snapshot();
      drag = {
        kind: "move",
        page,
        start: pt,
        item,
        orig: JSON.stringify(item),
        moved: false,
      };
    }
    return;
  }
  if (Ann.tool === "text") return startText(page, pt);
  if (Ann.tool === "edittext") return pickText(page, pt);
  if (Ann.tool === "editimage") return pickImage(page, pt);
  startShape(page, pt);
}
function startShape(page, pt) {
  const base = { page, color: Ann.color, width: Ann.width };
  const t = Ann.tool;
  snapshot();
  const item = { id: Ann.nextId++, type: t, ...base };
  if (t === "draw") item.pts = [[pt.x, pt.y]];
  else if (t === "line" || t === "arrow")
    Object.assign(item, { x1: pt.x, y1: pt.y, x2: pt.x, y2: pt.y });
  else Object.assign(item, { x: pt.x, y: pt.y, w: 0, h: 0 });
  if (t === "highlight")
    item.color =
      Ann.color === "#111111" || Ann.color === "#ffffff"
        ? "#f5c518"
        : Ann.color;
  Ann.items.push(item);
  drag = { kind: "create", page, start: pt, item };
}

function onMove(e) {
  if (!drag)
    return (
      (Ann.tool === "edittext" && hoverText(e)) ||
      (Ann.tool === "editimage" && hoverImage(e))
    );
  const pt = pagePoint(e, e.currentTarget);
  const { item } = drag;
  const dx = pt.x - drag.start.x;
  const dy = pt.y - drag.start.y;
  switch (drag.kind) {
    case "move":
      if (!drag.moved && Math.hypot(dx, dy) < 2 / V.scale) return;
      drag.moved = true;
      Object.assign(item, JSON.parse(drag.orig));
      translate(item, dx, dy);
      break;
    case "create":
      resizeNew(item, pt, e.shiftKey);
      break;
    case "br": {
      const b = bbox(item);
      item.w = Math.max(4, pt.x - b.x);
      item.h =
        item.type === "image" ? item.w * item.ratio : Math.max(4, pt.y - b.y);
      break;
    }
    case "ep1":
      item.x1 = pt.x;
      item.y1 = pt.y;
      break;
    case "ep2":
      item.x2 = pt.x;
      item.y2 = pt.y;
      break;
  }
  draw(drag.page);
}
function translate(item, dx, dy) {
  if (item.type === "line" || item.type === "arrow") {
    item.x1 += dx;
    item.x2 += dx;
    item.y1 += dy;
    item.y2 += dy;
  } else if (item.type === "draw")
    item.pts = item.pts.map(([x, y]) => [x + dx, y + dy]);
  else {
    item.x += dx;
    item.y += dy;
    if (item.type === "replace") item.base += dy;
  }
}
function resizeNew(item, pt, shift) {
  const s = drag.start;
  if (item.type === "draw") {
    const last = item.pts[item.pts.length - 1];
    if (Math.hypot(pt.x - last[0], pt.y - last[1]) > 1.5 / V.scale)
      item.pts.push([pt.x, pt.y]);
  } else if (item.type === "line" || item.type === "arrow") {
    item.x2 = pt.x;
    item.y2 = pt.y;
    if (shift) {
      const a =
        Math.round(Math.atan2(pt.y - s.y, pt.x - s.x) / (Math.PI / 4)) *
        (Math.PI / 4);
      const len = Math.hypot(pt.x - s.x, pt.y - s.y);
      item.x2 = s.x + len * Math.cos(a);
      item.y2 = s.y + len * Math.sin(a);
    }
  } else {
    let w = pt.x - s.x;
    let h = pt.y - s.y;
    if (shift && item.type !== "highlight") {
      const m = Math.max(Math.abs(w), Math.abs(h));
      w = Math.sign(w || 1) * m;
      h = Math.sign(h || 1) * m;
    }
    Object.assign(item, {
      x: Math.min(s.x, s.x + w),
      y: Math.min(s.y, s.y + h),
      w: Math.abs(w),
      h: Math.abs(h),
    });
  }
}
function onUp(e) {
  if (!drag) return;
  const d = drag;
  drag = null;
  const { item } = d;
  if (d.kind === "create") {
    const b = bbox(item);
    const tiny = item.type !== "draw" && b.w < 3 && b.h < 3;
    if (tiny || (item.type === "draw" && item.pts.length < 2)) {
      Ann.items = Ann.items.filter((i) => i !== item);
      Ann.past.pop();
    } else {
      Ann.sel = item.id;
      setTool("select");
    }
  } else if (d.kind === "move" && !d.moved) Ann.past.pop();
  draw(d.page);
  refreshBar();
}

/* ---------- text ---------- */
function startText(page, pt) {
  snapshot();
  const item = {
    id: Ann.nextId++,
    type: "text",
    page,
    x: pt.x,
    y: pt.y - Ann.size * 0.6,
    text: "",
    size: Ann.size,
    color: Ann.color,
    font: Ann.font,
    bold: Ann.bold,
    italic: Ann.italic,
  };
  Ann.items.push(item);
  editText(item);
}
function onDouble(e) {
  const hit = e.target.closest?.("[data-id]");
  const item = hit && Ann.items.find((i) => i.id === +hit.dataset.id);
  if (Ann.active && item && (item.type === "text" || item.type === "replace"))
    editText(item, true);
}
/* `existing`: an item that was already placed, so this edit needs its own undo step. */
function editText(item, existing = false) {
  const p = V.pages[item.page - 1];
  const area = el("textarea", "ann-text");
  const s = V.scale;
  const replacing = item.type === "replace";
  const before = item.text;
  if (existing) snapshot();
  area.value = item.text;
  const top = replacing ? item.base - item.size * TEXT_BASELINE : item.y;
  area.style.cssText = `left:${item.x * s}px;top:${top * s}px;font:${fontShorthand(item, item.size * s)};color:${item.color};line-height:${LINE_HEIGHT}`;
  if (replacing) area.style.background = item.bg;
  area.spellcheck = false;
  item.editing = true;
  draw(item.page);
  const fit = () => {
    area.style.height = "auto";
    area.style.width = "auto";
    area.rows = Math.max(1, area.value.split("\n").length);
    area.cols = Math.max(2, ...area.value.split("\n").map((l) => l.length + 1));
  };
  let finished = false;
  const done = () => {
    if (finished) return;
    finished = true;
    item.editing = false;
    item.text = replacing
      ? area.value.replace(/\n/g, " ")
      : area.value.replace(/\s+$/, "");
    area.remove();
    if (replacing && item.fresh && item.text === item.orig) {
      Ann.items = Ann.items.filter((i) => i !== item); // nothing was changed
      Ann.past.pop();
    } else if (!replacing && !item.text) {
      Ann.items = Ann.items.filter((i) => i !== item);
      Ann.past.pop();
    } else {
      if (existing && item.text === before) Ann.past.pop();
      delete item.fresh;
      Ann.sel = item.id;
    }
    draw(item.page);
    refreshBar();
    setTool("select");
  };
  area.oninput = fit;
  area.onblur = done;
  area.onkeydown = (e) => {
    e.stopPropagation();
    if (
      e.key === "Escape" ||
      (e.key === "Enter" &&
        (e.ctrlKey || e.metaKey || (replacing && !e.shiftKey)))
    ) {
      e.preventDefault();
      done();
    }
  };
  p.el.append(area);
  fit();
  setTimeout(() => (area.focus(), area.select()), 0);
}

/* ---------- editing existing text ---------- */
/* Text runs of a page in page units: [{x, base, w, size, text, font}], upright text only. */
async function ensureRuns(p) {
  if (p.runs || p.runsBusy || !V.doc) return;
  p.runsBusy = true;
  const doc = V.doc;
  try {
    const page = await doc.pdf.getPage(p.num);
    const viewport = page.getViewport({ scale: 1, rotation: page.rotate });
    const content = await page.getTextContent();
    p.runs = buildRuns(content.items, viewport, page);
  } catch (e) {
    console.warn("text runs failed", e);
    p.runs = [];
  }
  p.runsBusy = false;
  if (V.doc !== doc) p.runs = null;
}
function buildRuns(items, viewport, page) {
  const runs = [];
  let cur = null;
  if (viewport.rotation !== 0) return runs;
  for (const it of items) {
    if (typeof it.str !== "string") continue;
    const [a, b, , , e, f] = it.transform;
    const size = Math.hypot(a, b);
    if (!size || Math.abs(b) > 0.01 * size) {
      cur = null;
      continue;
    }
    const [x, base] = viewport.convertToViewportPoint(e, f);
    const gap = cur ? x - (cur.x + cur.w) : 0;
    if (
      cur &&
      cur.fontName === it.fontName &&
      Math.abs(cur.base - base) < 0.6 &&
      Math.abs(cur.size - size) < 0.3 &&
      gap > -size * 0.3 &&
      gap < size * 0.6
    ) {
      cur.text += it.str;
      cur.w = x + it.width - cur.x;
    } else
      runs.push(
        (cur = {
          x,
          base,
          w: it.width,
          size,
          text: it.str,
          fontName: it.fontName,
        }),
      );
  }
  return runs
    .filter((r) => r.text.trim())
    .map((r) => ({ ...r, ...fontTraits(page, r.fontName) }));
}
function fontTraits(page, name) {
  let f = null;
  try {
    if (page.commonObjs.has(name)) f = page.commonObjs.get(name);
  } catch (_) {}
  const n = String((f && f.name) || "")
    .replace(/^[A-Z]{6}\+/, "")
    .replace(/[-_,]/g, " ")
    .toLowerCase();
  const mono = !!(f && f.isMonospace) || /mono|courier|consolas/.test(n);
  const serif =
    !mono &&
    ((f && f.isSerifFont) ||
      /times|serif|georgia|garamond|minion|cambria/.test(n)) &&
    !/sans/.test(n);
  const hit = FONT_RULES.find(([re]) => re.test(n));
  return {
    font: hit ? hit[1] : mono ? "Cousine" : serif ? "Tinos" : "Arimo",
    bold: !!(f && f.bold) || /bold|black|heavy/.test(n),
    italic: !!(f && f.italic) || /italic|oblique/.test(n),
  };
}
const runBox = (r) => ({
  x: r.x,
  y: r.base - r.size * 0.86,
  w: r.w,
  h: r.size * 1.1,
});
const inBox = (b, pt) =>
  pt.x >= b.x && pt.x <= b.x + b.w && pt.y >= b.y && pt.y <= b.y + b.h;
/* The already-edited text or the original run under a point. */
function targetAt(page, pt) {
  const done = [...Ann.items]
    .reverse()
    .find((i) => i.type === "replace" && i.page === page && inBox(i, pt));
  if (done) return { done };
  const run = V.pages[page - 1].runs?.find((r) => inBox(runBox(r), pt));
  return run ? { run } : null;
}
function hoverText(e) {
  const svg = e.currentTarget;
  const page = +svg.dataset.page;
  const p = V.pages[page - 1];
  ensureRuns(p);
  const t = targetAt(page, pagePoint(e, svg));
  const run = t ? t.run || t.done : null;
  if (run === Ann.hover?.run) return;
  const old = Ann.hover;
  Ann.hover = run ? { page, run } : null;
  if (old) draw(old.page);
  draw(page);
}
function pickText(page, pt) {
  const p = V.pages[page - 1];
  const t = targetAt(page, pt);
  if (t && t.done) return editText(t.done, true);
  if (!t)
    return toast(
      p.runs
        ? "There’s no editable text here. Scanned pages have none."
        : "Reading the page’s text… try again in a moment.",
    );
  const { run } = t;
  const colors = sampleColors(p, run);
  snapshot();
  const box = runBox(run);
  const item = {
    id: Ann.nextId++,
    type: "replace",
    page,
    ...box,
    base: run.base,
    size: run.size,
    text: run.text,
    orig: run.text,
    color: colors.text,
    bg: colors.bg,
    font: run.font,
    bold: run.bold,
    italic: run.italic,
    fresh: true,
  };
  item.editing = true;
  Ann.items.push(item);
  Ann.hover = null;
  editText(item);
}
/* Background and ink colours of a run, read from the rendered page. */
function sampleColors(p, run, box = runBox(run)) {
  const fallback = { text: "#000000", bg: "#ffffff" };
  const canvas = p.el.querySelector("canvas");
  if (!canvas) return fallback;
  const k = canvas.width / p.w;
  const x0 = Math.max(0, Math.floor(box.x * k));
  const y0 = Math.max(0, Math.floor(box.y * k));
  const w = Math.min(canvas.width - x0, Math.ceil(box.w * k));
  const h = Math.min(canvas.height - y0, Math.ceil(box.h * k));
  if (w < 2 || h < 2) return fallback;
  const d = canvas.getContext("2d").getImageData(x0, y0, w, h).data;
  const px = (x, y) => (y * w + x) * 4;
  const counts = new Map();
  const ring = (x, y) => {
    const i = px(x, y);
    const key = (d[i] >> 3) * 1024 + (d[i + 1] >> 3) * 32 + (d[i + 2] >> 3);
    const c = counts.get(key) || { n: 0, r: d[i], g: d[i + 1], b: d[i + 2] };
    c.n++;
    counts.set(key, c);
  };
  for (let x = 0; x < w; x++) ring(x, 0), ring(x, h - 1);
  for (let y = 0; y < h; y++) ring(0, y), ring(w - 1, y);
  const bg = [...counts.values()].sort((a, b) => b.n - a.n)[0];
  let far = 0;
  let ink = [0, 0, 0];
  for (let i = 0; i < d.length; i += 4) {
    const dist =
      (d[i] - bg.r) ** 2 + (d[i + 1] - bg.g) ** 2 + (d[i + 2] - bg.b) ** 2;
    if (dist > far) (far = dist), (ink = [d[i], d[i + 1], d[i + 2]]);
  }
  const hex = (c) =>
    "#" + c.map((v) => v.toString(16).padStart(2, "0")).join("");
  return {
    bg: hex([bg.r, bg.g, bg.b]),
    text: far < 40 * 40 ? "#000000" : hex(ink),
  };
}

/* ---------- editing images that are already in the page ---------- */
const mulM = (m, n) => [
  m[0] * n[0] + m[1] * n[2],
  m[0] * n[1] + m[1] * n[3],
  m[2] * n[0] + m[3] * n[2],
  m[2] * n[1] + m[3] * n[3],
  m[4] * n[0] + m[5] * n[2] + n[4],
  m[4] * n[1] + m[5] * n[3] + n[5],
];
/* Boxes (page units) of the images a page paints, found by following the content stream's transforms. */
async function ensureImages(p) {
  if (p.imgBoxes || p.imgBusy || !V.doc) return;
  p.imgBusy = true;
  const doc = V.doc;
  try {
    const page = await doc.pdf.getPage(p.num);
    const vp = page.getViewport({ scale: 1, rotation: page.rotate });
    const { fnArray, argsArray } = await page.getOperatorList();
    const O = pdfjsLib.OPS;
    const painters = new Set(
      [
        O.paintImageXObject,
        O.paintInlineImageXObject,
        O.paintImageMaskXObject,
        O.paintJpegXObject,
      ].filter((v) => v != null),
    );
    let ctm = [1, 0, 0, 1, 0, 0];
    const stack = [];
    const boxes = [];
    fnArray.forEach((fn, i) => {
      if (fn === O.save) stack.push(ctm);
      else if (fn === O.restore) ctm = stack.pop() || ctm;
      else if (fn === O.transform) ctm = mulM(argsArray[i], ctm);
      else if (fn === O.paintFormXObjectBegin)
        stack.push(ctm), argsArray[i][0] && (ctm = mulM(argsArray[i][0], ctm));
      else if (fn === O.paintFormXObjectEnd) ctm = stack.pop() || ctm;
      else if (painters.has(fn)) {
        const pts = [
          [0, 0],
          [1, 0],
          [0, 1],
          [1, 1],
        ].map(([x, y]) =>
          vp.convertToViewportPoint(
            ctm[0] * x + ctm[2] * y + ctm[4],
            ctm[1] * x + ctm[3] * y + ctm[5],
          ),
        );
        const xs = pts.map((q) => q[0]);
        const ys = pts.map((q) => q[1]);
        const box = {
          x: Math.min(...xs),
          y: Math.min(...ys),
          w: Math.max(...xs) - Math.min(...xs),
          h: Math.max(...ys) - Math.min(...ys),
        };
        if (box.w >= 6 && box.h >= 6) boxes.push(box);
      }
    });
    p.imgBoxes = boxes;
  } catch (e) {
    console.warn("image boxes failed", e);
    p.imgBoxes = [];
  }
  p.imgBusy = false;
  if (V.doc !== doc) p.imgBoxes = null;
}
/* The smallest page image under a point. */
function imageAt(page, pt) {
  const hits = (V.pages[page - 1].imgBoxes || []).filter((b) => inBox(b, pt));
  return hits.sort((a, b) => a.w * a.h - b.w * b.h)[0] || null;
}
function hoverImage(e) {
  const svg = e.currentTarget;
  const page = +svg.dataset.page;
  ensureImages(V.pages[page - 1]);
  const box = imageAt(page, pagePoint(e, svg));
  if (box === Ann.hover?.box) return;
  const old = Ann.hover;
  Ann.hover = box ? { page, box } : null;
  if (old) draw(old.page);
  draw(page);
}
function pickImage(page, pt) {
  const box = imageAt(page, pt);
  if (!box)
    return toast(
      V.pages[page - 1].imgBoxes
        ? "There’s no image here."
        : "Reading the page… try again in a moment.",
    );
  const old = Ann.pickImg;
  Ann.pickImg = { page, box };
  if (old && old.page !== page) draw(old.page);
  draw(page);
  refreshBar();
}
/* Paints over the picked image with the page color around it. */
function coverItem(t) {
  const p = V.pages[t.page - 1];
  const around = {
    x: t.box.x - 3,
    y: t.box.y - 3,
    w: t.box.w + 6,
    h: t.box.h + 6,
  };
  const bg = sampleColors(p, null, around).bg;
  return {
    id: Ann.nextId++,
    type: "cover",
    page: t.page,
    x: t.box.x - 0.5,
    y: t.box.y - 0.5,
    w: t.box.w + 1,
    h: t.box.h + 1,
    bg,
  };
}
function removePicked() {
  const t = Ann.pickImg;
  if (!t) return toast("Click an image in the page first.");
  snapshot();
  Ann.items.push(coverItem(t));
  Ann.pickImg = null;
  draw(t.page);
  refreshBar();
}
async function replacePicked() {
  const t = Ann.pickImg;
  if (!t) return toast("Click an image in the page first.");
  const [file] = await pickFiles("image/*", false);
  if (!file) return;
  const dataUrl = await fileToDataUrl(file);
  const img = new Image();
  img.src = dataUrl;
  await img.decode();
  const ratio = img.naturalHeight / img.naturalWidth;
  const w = Math.min(t.box.w, t.box.h / ratio);
  const key = "img" + Ann.nextId;
  Ann.imgs.set(key, dataUrl);
  snapshot();
  Ann.items.push(coverItem(t));
  const item = {
    id: Ann.nextId++,
    type: "image",
    page: t.page,
    img: key,
    ratio,
    w,
    h: w * ratio,
    x: t.box.x + (t.box.w - w) / 2,
    y: t.box.y + (t.box.h - w * ratio) / 2,
  };
  Ann.items.push(item);
  Ann.pickImg = null;
  Ann.sel = item.id;
  setTool("select");
}

/* ---------- images and signatures ---------- */
async function placeImage(dataUrl) {
  if (!V.doc) return;
  const img = new Image();
  img.src = dataUrl;
  await img.decode();
  const num = V.current;
  const p = V.pages[num - 1];
  const w = Math.min(p.w * 0.35, img.naturalWidth);
  const key = "img" + Ann.nextId;
  Ann.imgs.set(key, dataUrl);
  const ratio = img.naturalHeight / img.naturalWidth;
  const box = scrollBox();
  // Centre it in the visible part of the page.
  const visibleMid =
    (box.scrollTop + box.clientHeight / 2 - V.tops[num - 1]) / V.scale;
  const item = addItem({
    type: "image",
    page: num,
    img: key,
    ratio,
    w,
    h: w * ratio,
    x: (p.w - w) / 2,
    y: clamp(visibleMid - (w * ratio) / 2, 0, p.h - w * ratio),
  });
  Ann.sel = item.id;
  if (!Ann.active) setAnnotating(true);
  setTool("select");
  draw(num);
  refreshBar();
}
async function fileToDataUrl(file) {
  const bitmap = await createImageBitmap(file);
  const k = Math.min(1, 1600 / Math.max(bitmap.width, bitmap.height));
  const c = document.createElement("canvas");
  c.width = Math.round(bitmap.width * k);
  c.height = Math.round(bitmap.height * k);
  c.getContext("2d").drawImage(bitmap, 0, 0, c.width, c.height);
  return c.toDataURL("image/png");
}
async function addImageFromFile() {
  const [file] = await pickFiles("image/*", false);
  if (file) placeImage(await fileToDataUrl(file));
}

/* ---------- mode and toolbar ---------- */
function setAnnotating(on) {
  if (on && V.rotation) rotateView(-V.rotation);
  Ann.active = on;
  $("scroll").classList.toggle("annotating", on);
  $("annbar").hidden = !on;
  if (on) buildAnnBar();
  else Ann.sel = null;
  drawAll();
  V.emit("annotating", on);
}
function setTool(t) {
  Ann.tool = t;
  Ann.hover = null;
  $("scroll").dataset.tool = t;
  if (t !== "select") Ann.sel = null;
  if (t !== "editimage") Ann.pickImg = null;
  if (t === "edittext") V.pages.forEach((p) => p.rendered && ensureRuns(p));
  if (t === "editimage") V.pages.forEach((p) => p.rendered && ensureImages(p));
  drawAll();
  refreshBar();
}
const toolBtn = (id, icon, label, hint) => {
  const b = el("button", "tl");
  b.type = "button";
  b.dataset.tip = hint || label;
  b.setAttribute("aria-label", hint || label);
  b.dataset.tool = id;
  b.append(ic(icon, 18));
  b.onclick = () => setTool(id);
  return b;
};
const actionBtn = (icon, label, hint, fn) => {
  const b = toolBtn("", icon, label, hint);
  delete b.dataset.tool;
  b.onclick = fn;
  return b;
};
const group = (label, ...kids) => {
  const g = el("div", "grp");
  g.append(el("span", "grp-label", label), ...kids);
  return g;
};
function buildAnnBar() {
  const bar = $("annbar");
  bar.replaceChildren();
  const top = el("div", "ann-row");
  const tip = (b, t) => ((b.dataset.tip = t), b.removeAttribute("title"), b);
  const [text, ...drawTools] = ADD_TOOLS.map(([id, icon, label, hint]) =>
    toolBtn(id, icon, label, hint),
  );
  top.append(
    group(
      "Add",
      text,
      actionBtn("pic", "Image", "Add image", addImageFromFile),
      actionBtn("signature", "Signature", "Add signature", openSignDialog),
      ...drawTools,
      el("span", "sep"),
      ...SHAPE_TOOLS.map(([id, icon, label]) => toolBtn(id, icon, "", label)),
    ),
    el("span", "sep"),
    group(
      "Edit",
      toolBtn("select", "cursor", "Select", "Select, move and resize"),
      toolBtn("edittext", "edit-3", "Edit text", "Edit text in the page"),
      toolBtn(
        "editimage",
        "image-edit",
        "Edit images",
        "Replace or remove images in the page",
      ),
    ),
    el("span", "grow"),
    tip(iconBtn("back-2", "Undo", undo, "tl"), "Undo (Ctrl+Z)"),
    tip(iconBtn("forward-2", "Redo", redo, "tl"), "Redo (Ctrl+Y)"),
    tip(
      iconBtn("delete-2", "Delete", removeSelected, "tl del"),
      "Delete selected (Del)",
    ),
    el("span", "sep"),
    btn("Done", null, finishEditing, "small ghost"),
    btn("Save", "download-2", saveEdited, "primary small"),
  );

  const props = el("div", "ann-row props-row");
  const hint = el("span", "ann-hint muted");
  hint.id = "annHint";
  const colors = el("div", "swatches");
  SWATCHES.forEach((c) => {
    const sw = el("button", "swatch");
    sw.type = "button";
    sw.style.background = c;
    sw.title = c;
    sw.setAttribute("aria-label", "Color " + c);
    sw.onclick = () => setColor(c);
    sw.dataset.color = c;
    colors.append(sw);
  });
  const custom = el("input", "swatch-custom");
  custom.type = "color";
  custom.title = "Custom color";
  custom.oninput = () => setColor(custom.value);
  colors.append(custom);
  const colorBox = el("div", "pgrp");
  colorBox.dataset.for = "color";
  colorBox.append(el("span", "grp-label", "Color"), colors);

  const font = el("select", "field mini font-pick");
  font.id = "annFont";
  FONTS.forEach(([name]) => {
    const o = new Option(name, name);
    o.style.fontFamily = `"${name}"`;
    font.append(o);
  });
  font.title = "Font";
  font.onchange = () => setFont(font.value);
  const size = el("select", "field mini");
  [8, 10, 12, 14, 16, 20, 24, 32, 48, 72].forEach((n) =>
    size.append(new Option(n + " pt", n)),
  );
  size.title = "Text size";
  size.id = "annSize";
  size.onchange = () => setSize(+size.value);
  const bold = iconBtn("", "Bold", () => toggleStyle("bold"), "txt sty");
  bold.dataset.style = "bold";
  bold.replaceChildren(el("b", "", "B"));
  const italic = iconBtn("", "Italic", () => toggleStyle("italic"), "txt sty");
  italic.dataset.style = "italic";
  italic.replaceChildren(el("i", "", "I"));
  const textBox = el("div", "pgrp");
  textBox.dataset.for = "text";
  textBox.append(el("span", "grp-label", "Font"), font, size, bold, italic);

  const width = el("select", "field mini");
  [1, 2, 3, 5, 8].forEach((w) => width.append(new Option(w + " px", w)));
  width.title = "Line width";
  width.id = "annWidth";
  width.onchange = () => setWidth(+width.value);
  const strokeBox = el("div", "pgrp");
  strokeBox.dataset.for = "stroke";
  strokeBox.append(el("span", "grp-label", "Line"), width);

  const imageBox = el("div", "pgrp");
  imageBox.dataset.for = "image";
  const replace = btn(
    "Replace image",
    "transfer-horizontal",
    replacePicked,
    "small",
  );
  const remove = btn("Remove image", "delete-2", removePicked, "small");
  replace.id = "annReplace";
  remove.id = "annRemove";
  imageBox.append(replace, remove);

  props.append(hint, colorBox, textBox, strokeBox, imageBox);
  bar.append(top, props);
  refreshBar();
}
/* Which property controls apply: to the selected item, or to what the current tool will create. */
function propKind() {
  const s = selected();
  if (s)
    return TEXT_KINDS.includes(s.type)
      ? "text"
      : s.type === "highlight"
        ? "color"
        : s.type === "image" || s.type === "cover"
          ? ""
          : "stroke";
  return (
    {
      text: "text",
      edittext: "",
      editimage: "image",
      highlight: "color",
      draw: "stroke",
      rect: "stroke",
      ellipse: "stroke",
      line: "stroke",
      arrow: "stroke",
    }[Ann.tool] || ""
  );
}
function refreshBar() {
  const bar = $("annbar");
  if (bar.hidden) return;
  bar
    .querySelectorAll("[data-tool]")
    .forEach((b) => b.classList.toggle("on", b.dataset.tool === Ann.tool));
  const s = selected();
  const kind = propKind();
  const show = {
    color: ["color", "text", "stroke"].includes(kind),
    text: kind === "text",
    stroke: kind === "stroke",
    image: kind === "image",
  };
  bar
    .querySelectorAll("[data-for]")
    .forEach((n) => (n.hidden = !show[n.dataset.for]));
  const hint = $("annHint");
  hint.hidden =
    Object.values(show).some(Boolean) && !(Ann.tool === "edittext" && !s);
  hint.textContent = s
    ? TEXT_KINDS.includes(s.type)
      ? "Double-click to change the words."
      : "Drag to move. Press Delete to remove."
    : HINTS[Ann.tool] || "";
  if (Ann.tool === "editimage" && !Ann.pickImg) $("annHint").hidden = false;
  const color = s ? s.color : Ann.color;
  bar
    .querySelectorAll("[data-color]")
    .forEach((b) => b.classList.toggle("on", b.dataset.color === color));
  $("annWidth").value = String(s && s.width ? s.width : Ann.width);
  const size = $("annSize");
  const want = s && s.size ? s.size : Ann.size;
  if (![...size.options].some((o) => +o.value === Math.round(want)))
    size.append(new Option(Math.round(want) + " pt", Math.round(want)));
  size.value = String(Math.round(want));
  $("annFont").value = (s && s.font) || Ann.font;
  const src = s && TEXT_KINDS.includes(s.type) ? s : Ann;
  bar
    .querySelectorAll("[data-style]")
    .forEach((b) => b.classList.toggle("on", !!src[b.dataset.style]));
  $("annReplace").disabled = $("annRemove").disabled = !Ann.pickImg;
  bar.querySelector(".del").disabled = !s;
}
function setFont(name) {
  const s = selected();
  if (s && TEXT_KINDS.includes(s.type)) {
    snapshot();
    s.font = name;
    draw(s.page);
  } else Ann.font = name;
  refreshBar();
}
function toggleStyle(key) {
  const s = selected();
  if (s && TEXT_KINDS.includes(s.type)) {
    snapshot();
    s[key] = !s[key];
    draw(s.page);
  } else Ann[key] = !Ann[key];
  refreshBar();
}
function setColor(c) {
  const s = selected();
  if (s) {
    snapshot();
    s.color = c;
    draw(s.page);
  } else Ann.color = c;
  refreshBar();
}
function setWidth(w) {
  const s = selected();
  if (s && s.width) {
    snapshot();
    s.width = w;
    draw(s.page);
  } else Ann.width = w;
}
function setSize(n) {
  const s = selected();
  if (s && TEXT_KINDS.includes(s.type)) {
    snapshot();
    s.size = n;
    draw(s.page);
  } else Ann.size = n;
}

function onAnnotationKey(e) {
  if (!Ann.active) return false;
  const typing = /^(INPUT|TEXTAREA|SELECT)$/.test(e.target.tagName);
  const mod = e.ctrlKey || e.metaKey;
  if (mod && !typing && e.key.toLowerCase() === "z")
    (e.shiftKey ? redo : undo)();
  else if (mod && !typing && e.key.toLowerCase() === "y") redo();
  else if (!typing && (e.key === "Delete" || e.key === "Backspace"))
    (Ann.pickImg ? removePicked : removeSelected)();
  else if (e.key === "Escape" && !typing) {
    if (Ann.sel != null || Ann.tool !== "select") {
      Ann.sel = null;
      setTool("select");
    } else return false;
  } else return false;
  e.preventDefault();
  return true;
}
V.on(
  "rendered",
  (p) =>
    Ann.active &&
    (Ann.tool === "edittext"
      ? ensureRuns(p)
      : Ann.tool === "editimage" && ensureImages(p)),
);
V.on("sizes", () =>
  V.pages.forEach((p) => p.svg?.setAttribute("viewBox", `0 0 ${p.w} ${p.h}`)),
);
V.on("zoom", () => Ann.active && selected() && draw(selected().page));
V.on("doc", () => {
  Ann.items = [];
  Ann.imgs.clear();
  Ann.past = [];
  Ann.future = [];
  Ann.sel = null;
  Ann.pickImg = null;
  if (Ann.active) setAnnotating(false);
});
