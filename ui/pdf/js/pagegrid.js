/*
 * Page thumbnails for the page tools. Two modes:
 *  - "select": click pages to mark them (remove / extract).
 *  - "organize": reorder by dragging, rotate, delete, duplicate, insert blank pages.
 */
"use strict";

const GRID_THUMB = 150;

/* One pdf.js document per loaded file, shared by everything that draws its pages. */
function pdfjsFor(file) {
  if (!file._pdf) {
    file._pdf = pdfjsLib.getDocument({
      data: file.bytes.slice(),
      password: file.password || undefined,
      isEvalSupported: false,
    }).promise;
  }
  return file._pdf;
}

/* Renders one page into a new canvas about `width` CSS pixels wide. */
async function renderThumb(file, index, rotation, width) {
  const pdf = await pdfjsFor(file);
  const page = await pdf.getPage(index + 1);
  const turn = (page.rotate + rotation) % 360;
  const base = page.getViewport({ scale: 1, rotation: turn });
  const viewport = page.getViewport({
    scale: (width * Math.min(devicePixelRatio || 1, 2)) / base.width,
    rotation: turn,
  });
  const canvas = document.createElement("canvas");
  canvas.width = Math.floor(viewport.width);
  canvas.height = Math.floor(viewport.height);
  await page.render({
    canvasContext: canvas.getContext("2d", { alpha: false }),
    viewport,
  }).promise;
  return { canvas, ratio: base.width / base.height };
}

/* [0,1,2,4] -> "1-3, 5" */
function indicesToText(set) {
  const list = [...set].sort((a, b) => a - b);
  const parts = [];
  for (let i = 0; i < list.length; ) {
    let j = i;
    while (j + 1 < list.length && list[j + 1] === list[j] + 1) j++;
    parts.push(j > i ? `${list[i] + 1}-${list[j] + 1}` : String(list[i] + 1));
    i = j + 1;
  }
  return parts.join(", ");
}

function pageGrid(wb, host, { mode, tone = "keep", onChange }) {
  const file = wb.files[0];
  const grid = { mode, items: [], selected: new Set(), tiles: [] };
  for (let i = 0; i < file.pages; i++)
    grid.items.push({ src: i, rot: 0, del: false });
  const box = el("div", "pgrid " + mode + " " + tone);
  host.append(box);
  const queue = [];
  let painting = false;

  const io = new IntersectionObserver(
    (entries) => {
      for (const e of entries)
        if (e.isIntersecting && !e.target.__painted) queue.push(e.target);
      paintNext();
    },
    { rootMargin: "400px 0px" },
  );
  async function paintNext() {
    if (painting) return;
    const tile = queue.shift();
    if (!tile) return;
    painting = true;
    try {
      await tile.paint();
    } catch (_) {}
    painting = false;
    paintNext();
  }

  function makeTile(item) {
    const tile = el("div", "ptile");
    const img = el("div", "ptile-img");
    const label = el("span", "ptile-n");
    tile.append(img, label);
    tile.item = item;
    tile.label = label;
    tile.paint = async () => {
      if (item.src < 0) return (tile.__painted = true);
      const { canvas, ratio } = await renderThumb(
        file,
        item.src,
        item.rot,
        GRID_THUMB,
      );
      img.style.aspectRatio = String(ratio);
      img.replaceChildren(canvas);
      tile.__painted = true;
    };
    tile.repaint = () => {
      tile.__painted = false;
      queue.push(tile);
      paintNext();
    };
    if (mode === "select") {
      tile.tabIndex = 0;
      tile.setAttribute("role", "checkbox");
      const toggle = () => {
        const k = item.src;
        grid.selected.has(k) ? grid.selected.delete(k) : grid.selected.add(k);
        refresh();
        onChange?.(grid);
      };
      tile.onclick = toggle;
      tile.onkeydown = (e) =>
        (e.key === " " || e.key === "Enter") && (e.preventDefault(), toggle());
    } else {
      tile.draggable = true;
      const tools = el("div", "ptile-tools");
      tools.append(
        iconBtn("anticlockwise", "Rotate left", () => rotate(item, -90)),
        iconBtn("clockwise", "Rotate right", () => rotate(item, 90)),
        iconBtn("copy-2", "Duplicate page", () => duplicate(item)),
        iconBtn(
          "delete-2",
          "Delete page",
          () => ((item.del = !item.del), refresh()),
        ),
      );
      tile.append(tools);
      tile.ondragstart = (e) => {
        e.dataTransfer.effectAllowed = "move";
        e.dataTransfer.setData("text/plain", "page");
        grid.dragging = item;
        tile.classList.add("dragging");
      };
      tile.ondragend = () => {
        tile.classList.remove("dragging");
        box
          .querySelectorAll(".over")
          .forEach((n) => n.classList.remove("over"));
        grid.dragging = null;
      };
      tile.ondragover = (e) => {
        if (!grid.dragging) return;
        e.preventDefault();
        tile.classList.add("over");
      };
      tile.ondragleave = () => tile.classList.remove("over");
      tile.ondrop = (e) => {
        e.preventDefault();
        e.stopPropagation();
        tile.classList.remove("over");
        const from = grid.items.indexOf(grid.dragging);
        const to = grid.items.indexOf(item);
        if (from < 0 || from === to) return;
        const [moved] = grid.items.splice(from, 1);
        grid.items.splice(to, 0, moved);
        order();
      };
    }
    return tile;
  }
  function rotate(item, deg) {
    item.rot = (item.rot + deg + 360) % 360;
    grid.tiles.find((t) => t.item === item)?.repaint();
  }
  function duplicate(item) {
    const copy = { ...item };
    grid.items.splice(grid.items.indexOf(item) + 1, 0, copy);
    const tile = makeTile(copy);
    grid.tiles.push(tile);
    io.observe(tile);
    order();
  }
  /* Puts the tile elements in item order without redrawing them. */
  function order() {
    const byItem = new Map(grid.tiles.map((t) => [t.item, t]));
    grid.items.forEach((item, i) => {
      const t = byItem.get(item);
      t.label.textContent = String(i + 1);
      box.append(t);
    });
    refresh();
  }
  function refresh() {
    for (const t of grid.tiles) {
      const { item } = t;
      if (mode === "select") {
        const on = grid.selected.has(item.src);
        t.classList.toggle("sel", on);
        t.setAttribute("aria-checked", String(on));
      } else t.classList.toggle("del", item.del);
    }
  }

  grid.addBlank = () => {
    const item = { src: -1, rot: 0, del: false };
    grid.items.push(item);
    const tile = makeTile(item);
    grid.tiles.push(tile);
    tile.classList.add("blank");
    io.observe(tile);
    order();
    tile.scrollIntoView({ block: "nearest" });
  };
  grid.rotateAll = (deg) => grid.items.forEach((item) => rotate(item, deg));
  grid.setSelection = (set) => {
    grid.selected = set;
    refresh();
  };
  grid.destroy = () => io.disconnect();

  grid.items.forEach((item) => {
    const tile = makeTile(item);
    grid.tiles.push(tile);
    io.observe(tile);
  });
  order();
  return grid;
}
