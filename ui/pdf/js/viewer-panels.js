/* Viewer side panel (thumbnails, outline), document properties and printing. */
"use strict";

const THUMB_W = 132;
const Panels = { tab: "thumbs", thumbs: [], observer: null, rendering: false, queue: [] };

function toggleSidePanel(force) {
  const side = $("side");
  const open = force ?? side.hidden;
  side.hidden = !open;
  if (open && !side.firstChild) buildSidePanel();
  if (open) V.emit("zoom"); // the page area changed width
}

function buildSidePanel() {
  const side = $("side");
  const tabs = el("div", "side-tabs");
  const body = el("div", "side-body");
  const show = (tab) => {
    Panels.tab = tab;
    tabs.querySelectorAll("button").forEach((b) => b.classList.toggle("on", b.dataset.tab === tab));
    if (tab === "thumbs") buildThumbs(body);
    else buildOutline(body);
  };
  for (const [tab, icon, label] of [["thumbs", "pages", "Pages"], ["outline", "list", "Outline"]]) {
    const b = el("button", "tab");
    b.type = "button";
    b.dataset.tab = tab;
    b.title = label;
    b.append(ic(icon, 16), el("span", "", label));
    b.onclick = () => show(tab);
    tabs.append(b);
  }
  side.replaceChildren(tabs, body);
  Panels.show = show;
  show(Panels.tab);
}

/* ---------- thumbnails ---------- */
function buildThumbs(body) {
  Panels.observer?.disconnect();
  Panels.queue = [];
  Panels.thumbs = [];
  body.replaceChildren();
  body.classList.remove("outline");
  if (!V.doc) return;
  Panels.observer = new IntersectionObserver(
    (entries) => {
      for (const e of entries) if (e.isIntersecting && !e.target.__done) Panels.queue.push(e.target);
      drawNextThumb();
    },
    { root: body, rootMargin: "200px 0px" },
  );
  V.pages.forEach((p) => {
    const b = el("button", "thumb");
    b.type = "button";
    b.setAttribute("aria-label", "Page " + p.num);
    b.style.aspectRatio = p.w + " / " + p.h;
    const holder = el("div", "thumb-img");
    b.append(holder, el("span", "thumb-n", String(p.num)));
    b.onclick = () => goToPage(p.num);
    b.__num = p.num;
    b.__holder = holder;
    body.append(b);
    Panels.thumbs.push(b);
    Panels.observer.observe(b);
  });
  markThumb(V.current);
}
async function drawNextThumb() {
  if (Panels.rendering) return;
  const target = Panels.queue.shift();
  if (!target) return;
  Panels.rendering = true;
  const doc = V.doc;
  try {
    const page = await doc.pdf.getPage(target.__num);
    const base = page.getViewport({ scale: 1 });
    const scale = (THUMB_W * Math.min(devicePixelRatio || 1, 2)) / base.width;
    const viewport = page.getViewport({ scale });
    const canvas = document.createElement("canvas");
    canvas.width = Math.floor(viewport.width);
    canvas.height = Math.floor(viewport.height);
    await page.render({ canvasContext: canvas.getContext("2d", { alpha: false }), viewport }).promise;
    if (V.doc === doc) {
      target.__holder.replaceChildren(canvas);
      target.style.aspectRatio = base.width + " / " + base.height;
      target.__done = true;
    }
  } catch (_) {}
  Panels.rendering = false;
  drawNextThumb();
}
function markThumb(n) {
  Panels.thumbs.forEach((b) => b.classList.toggle("on", b.__num === n));
  const b = Panels.thumbs[n - 1];
  if (b && !$("side").hidden && Panels.tab === "thumbs") b.scrollIntoView({ block: "nearest" });
}

/* ---------- outline ---------- */
async function buildOutline(body) {
  Panels.observer?.disconnect();
  body.replaceChildren();
  body.classList.add("outline");
  if (!V.doc) return;
  const doc = V.doc;
  const items = await doc.pdf.getOutline().catch(() => null);
  if (V.doc !== doc || Panels.tab !== "outline") return;
  if (!items || !items.length) return body.append(el("p", "muted pad", "This document has no outline."));
  body.append(outlineList(items, 0));
}
function outlineList(items, depth) {
  const ul = el("ul", "tree");
  for (const item of items) {
    const li = el("li");
    const row = el("div", "tree-row");
    const kids = item.items && item.items.length;
    const twist = el("button", "twist");
    twist.type = "button";
    twist.setAttribute("aria-label", "Expand");
    if (kids) twist.append(ic("right", 14));
    const link = el("button", "tree-link", item.title || "Untitled");
    link.type = "button";
    link.onclick = () => (item.url && /^https?:/i.test(item.url) ? window.open(item.url) : goToDestination(item.dest));
    row.append(twist, link);
    li.append(row);
    if (kids) {
      const sub = outlineList(item.items, depth + 1);
      sub.hidden = true;
      li.append(sub);
      twist.onclick = () => {
        sub.hidden = !sub.hidden;
        twist.classList.toggle("open", !sub.hidden);
      };
    }
    ul.append(li);
  }
  return ul;
}

/* ---------- properties ---------- */
async function showProperties() {
  if (!V.doc) return;
  const { info } = await V.doc.pdf.getMetadata().catch(() => ({ info: {} }));
  const page = await V.doc.pdf.getPage(1);
  const v = page.getViewport({ scale: 1 });
  const mm = (pt) => Math.round((pt / 72) * 25.4);
  const rows = [
    ["File", V.doc.name],
    ["Size", fmtBytes(V.doc.bytes.length)],
    ["Pages", V.doc.n],
    ["Page size", `${mm(v.width)} × ${mm(v.height)} mm`],
    ["Title", info.Title],
    ["Author", info.Author],
    ["Subject", info.Subject],
    ["Created with", info.Creator],
    ["PDF producer", info.Producer],
    ["PDF version", info.PDFFormatVersion],
  ].filter((r) => r[1]);
  const dl = el("dl", "props");
  rows.forEach(([k, val]) => dl.append(el("dt", "", k), el("dd", "", String(val))));
  dialog("Document properties", dl);
}

/* ---------- print ---------- */
async function printDocument() {
  if (!V.doc) return;
  const area = $("printArea");
  const urls = [];
  const doc = V.doc;
  await withBusy("Preparing to print…", async (b) => {
    area.replaceChildren();
    for (let i = 1; i <= doc.n; i++) {
      b.set((i - 1) / doc.n, `Preparing page ${i} of ${doc.n}`);
      const page = await doc.pdf.getPage(i);
      const viewport = page.getViewport({ scale: 2 });
      const canvas = document.createElement("canvas");
      canvas.width = viewport.width;
      canvas.height = viewport.height;
      await page.render({ canvasContext: canvas.getContext("2d", { alpha: false }), viewport }).promise;
      const url = URL.createObjectURL(await canvasBlob(canvas, "image/jpeg", 0.92));
      urls.push(url);
      const img = new Image();
      img.src = url;
      area.append(img);
      canvas.width = canvas.height = 0;
    }
  });
  if (!area.children.length) return;
  await Promise.all([...area.children].map((i) => i.decode().catch(() => {})));
  const cleanup = () => {
    area.replaceChildren();
    urls.forEach(URL.revokeObjectURL);
  };
  window.addEventListener("afterprint", cleanup, { once: true });
  window.print();
}

V.on("doc", () => {
  const side = $("side");
  if (side.hidden) side.replaceChildren(); // rebuilt for the new file when it is opened
  else Panels.show?.(Panels.tab);
});
V.on("page", markThumb);
V.on("sizes", () => Panels.thumbs.forEach((b, i) => !b.__done && (b.style.aspectRatio = V.pages[i].w + " / " + V.pages[i].h)));
