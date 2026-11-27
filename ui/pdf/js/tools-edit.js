/* Edit tools: edit and sign, page numbers, watermark, crop. */
"use strict";

const MM_TO_PT = 72 / 25.4;
const hexToRgb = (hex) => hexColor(hex);

defineTool({
  id: "edit", name: "Edit & sign", desc: "Add text, highlights, shapes, drawings, images and your signature.",
  icon: "edit", group: "Edit",
  async custom(files) {
    if (files.length) return openInViewer(files[0].bytes, files[0].name, { edit: true });
    const [f] = await pickFiles("application/pdf,.pdf", false);
    if (f) openFile(f, { edit: true });
  },
});

/* ---------- page numbers ---------- */
const NUMBER_POSITIONS = [
  ["bc", "Bottom center"], ["br", "Bottom right"], ["bl", "Bottom left"],
  ["tc", "Top center"], ["tr", "Top right"], ["tl", "Top left"],
];
const NUMBER_FORMATS = [["n", "1, 2, 3"], ["page", "Page 1"], ["of", "Page 1 of 10"], ["slash", "1 / 10"]];

function numberLabel(format, n, last) {
  return { n: `${n}`, page: `Page ${n}`, of: `Page ${n} of ${last}`, slash: `${n} / ${last}` }[format];
}

defineTool({
  id: "numbers", name: "Add page numbers", desc: "Number the pages and choose where and how.",
  icon: "hash", group: "Edit", accept: "pdf", action: "Add page numbers", working: "Adding page numbers…",
  options(wb, side) {
    const o = wb.opt;
    o.pos = selectControl(NUMBER_POSITIONS, o.pos?.value || "bc");
    o.format = selectControl(NUMBER_FORMATS, o.format?.value || "n");
    o.first = numberControl(1, -999, 99999);
    o.size = numberControl(11, 6, 48);
    o.margin = numberControl(28, 0, 150);
    o.range = el("input", "field");
    o.range.placeholder = `All ${wb.files[0].pages} pages`;
    o.color = Object.assign(el("input", "color"), { type: "color", value: "#333333" });
    side.append(optionRow("Position", o.pos), optionRow("Format", o.format), optionRow("First number", o.first), optionRow("Font size", o.size), optionRow("Margin (pt)", o.margin), optionRow("Color", o.color), optionRow("Pages", o.range, "Optional, e.g. 2-10. Numbering starts at the first of them."));
  },
  async run(wb) {
    const [f] = wb.files;
    const o = wb.opt;
    const { lib } = await openLib(f.bytes, f.name, f.password);
    const font = await lib.embedFont(StandardFonts.Helvetica);
    const indices = rangeIndices(parseRanges(o.range.value, f.pages));
    const size = clamp(+o.size.value || 11, 6, 48);
    const margin = Math.max(0, +o.margin.value || 0);
    const first = Math.round(+o.first.value || 1);
    const last = first + indices.length - 1;
    const pages = lib.getPages();
    indices.forEach((pi, k) => {
      const page = pages[pi];
      const frame = pageFrame(page);
      const text = numberLabel(o.format.value, first + k, last);
      const w = font.widthOfTextAtSize(text, size);
      const [v, h] = o.pos.value;
      const x = h === "l" ? margin : h === "r" ? frame.W - margin - w : (frame.W - w) / 2;
      const y = v === "b" ? margin : frame.H - margin - size * 0.72;
      const [ux, uy] = frame.pt(x, y);
      page.drawText(text, { x: ux, y: uy, size, font, color: hexToRgb(o.color.value), rotate: degrees(frame.rot) });
    });
    return [await pdfResult(lib, `${baseName(f.name)}-numbered.pdf`)];
  },
});

/* ---------- watermark ---------- */
const WATERMARK_ANGLES = [["45", "Diagonal (45°)"], ["0", "Horizontal"], ["-45", "Diagonal (−45°)"], ["90", "Vertical"]];

/* Anchor (bottom-left of an unrotated box) for a box of w x h whose centre is (cx, cy) after turning by `deg`. */
function anchorForCentre(cx, cy, w, h, deg) {
  const t = (deg * Math.PI) / 180;
  return [cx - (w / 2) * Math.cos(t) + (h / 2) * Math.sin(t), cy - (w / 2) * Math.sin(t) - (h / 2) * Math.cos(t)];
}

defineTool({
  id: "watermark", name: "Add watermark", desc: "Stamp text or an image across the pages.",
  icon: "drop", group: "Edit", accept: "pdf", action: "Add watermark", working: "Adding watermark…",
  options(wb, side) {
    const o = wb.opt;
    o.kind = segControl([["text", "Text"], ["image", "Image"]], o.kind?.value || "text", () => show());
    o.text = Object.assign(el("input", "field"), { value: o.text?.value || "CONFIDENTIAL" });
    o.size = numberControl(o.size?.value || 64, 8, 400);
    o.color = Object.assign(el("input", "color"), { type: "color", value: o.color?.value || "#d6342a" });
    o.image = null;
    o.imageBtn = btn("Choose image…", "image", async () => {
      const [file] = await pickFiles("image/png,image/jpeg", false);
      if (!file) return;
      o.image = file;
      o.imageBtn.lastChild.textContent = file.name;
    }, "small");
    o.scale = numberControl(50, 5, 100);
    o.opacity = Object.assign(el("input", "range"), { type: "range", min: 5, max: 100, value: 30 });
    o.angle = selectControl(WATERMARK_ANGLES, o.angle?.value || "45");
    o.tile = checkControl("Repeat across the page", false);
    o.range = Object.assign(el("input", "field"), { placeholder: `All ${wb.files[0].pages} pages` });
    const textBox = el("div", "stack");
    textBox.append(optionRow("Text", o.text), optionRow("Font size", o.size), optionRow("Color", o.color));
    const imageBox = el("div", "stack");
    imageBox.append(o.imageBtn, optionRow("Width (% of page)", o.scale));
    const show = () => {
      textBox.hidden = o.kind.value !== "text";
      imageBox.hidden = o.kind.value !== "image";
    };
    side.append(o.kind, textBox, imageBox, optionRow("Opacity", o.opacity), optionRow("Rotation", o.angle), o.tile, optionRow("Pages", o.range));
    show();
  },
  async run(wb) {
    const [f] = wb.files;
    const o = wb.opt;
    const asImage = o.kind.value === "image";
    if (asImage && !o.image) throw new UserError("Choose an image first.");
    if (!asImage && !o.text.value.trim()) throw new UserError("Type the watermark text first.");
    const { lib } = await openLib(f.bytes, f.name, f.password);
    const font = await lib.embedFont(StandardFonts.HelveticaBold);
    const pic = asImage ? await embedUpload(lib, o.image) : null;
    const opacity = clamp(+o.opacity.value / 100, 0.05, 1);
    const deg = +o.angle.value;
    const color = hexToRgb(o.color.value);
    const text = o.text.value.trim().replace(/[^\x20-\x7e\xa0-\xff]/g, "?");
    const size = +o.size.value || 64;
    const pages = lib.getPages();
    for (const pi of rangeIndices(parseRanges(o.range.value, f.pages))) {
      const page = pages[pi];
      const fr = pageFrame(page);
      const w = asImage ? (fr.W * clamp(+o.scale.value, 5, 100)) / 100 : font.widthOfTextAtSize(text, size);
      const h = asImage ? (w * pic.height) / pic.width : size * 0.72;
      const centres = [];
      if (o.tile.input.checked) {
        const stepX = Math.max(w, 1) * 1.5;
        const stepY = Math.max(h, 1) * 3;
        for (let y = stepY / 2, r = 0; y < fr.H + stepY && r < 40; y += stepY, r++) {
          for (let x = (r % 2 ? stepX / 2 : 0) + stepX / 4; x < fr.W + stepX && centres.length < 400; x += stepX) centres.push([x, y]);
        }
      } else centres.push([fr.W / 2, fr.H / 2]);
      for (const [cx, cy] of centres) {
        const [ax, ay] = anchorForCentre(cx, cy, w, h, deg);
        const [ux, uy] = fr.pt(ax, ay);
        const rotate = degrees(deg + fr.rot);
        if (asImage) page.drawImage(pic, { x: ux, y: uy, width: w, height: h, rotate, opacity });
        else page.drawText(text, { x: ux, y: uy, size, font, color, rotate, opacity });
      }
    }
    return [await pdfResult(lib, `${baseName(f.name)}-watermarked.pdf`)];
  },
});

async function embedUpload(lib, file) {
  const bytes = await readBytes(file);
  return /png$/i.test(file.type) ? lib.embedPng(bytes) : lib.embedJpg(bytes);
}

/* ---------- crop ---------- */
defineTool({
  id: "crop", name: "Crop PDF", desc: "Trim the margins of every page.",
  icon: "crop", group: "Edit", accept: "pdf", action: "Crop PDF", working: "Cropping…",
  main(wb, main) {
    const o = wb.opt;
    o.page = { w: 595, h: 842 };
    const stage = el("div", "crop-stage");
    const holder = el("div", "crop-page");
    const mask = el("div", "crop-keep");
    holder.append(mask);
    stage.append(holder);
    main.append(stage);
    o.mask = mask;
    renderThumb(wb.files[0], 0, 0, 420).then(({ canvas, ratio }) => {
      holder.style.aspectRatio = String(ratio);
      holder.prepend(canvas);
      pdfjsFor(wb.files[0]).then((pdf) => pdf.getPage(1)).then((p) => {
        const v = p.getViewport({ scale: 1 });
        o.page = { w: v.width, h: v.height };
        updateCropPreview(wb);
      });
    }).catch(() => {});
  },
  options(wb, side) {
    const o = wb.opt;
    for (const k of ["top", "right", "bottom", "left"]) {
      o[k] = numberControl(0, 0, 500, 1);
      o[k].oninput = () => updateCropPreview(wb);
    }
    o.range = Object.assign(el("input", "field"), { placeholder: `All ${wb.files[0].pages} pages` });
    side.append(el("p", "muted", "Margins to cut off, in millimetres. The preview shows the first page."), optionRow("Top", o.top), optionRow("Right", o.right), optionRow("Bottom", o.bottom), optionRow("Left", o.left), optionRow("Pages", o.range));
    updateCropPreview(wb);
  },
  async run(wb) {
    const [f] = wb.files;
    const o = wb.opt;
    const m = Object.fromEntries(["top", "right", "bottom", "left"].map((k) => [k, Math.max(0, +o[k].value || 0) * MM_TO_PT]));
    if (!Object.values(m).some((v) => v > 0)) throw new UserError("Enter a margin to crop first.");
    const { lib } = await openLib(f.bytes, f.name, f.password);
    const pages = lib.getPages();
    for (const pi of rangeIndices(parseRanges(o.range.value, f.pages))) {
      const page = pages[pi];
      const fr = pageFrame(page);
      if (m.left + m.right >= fr.W - 10 || m.top + m.bottom >= fr.H - 10) throw new UserError(`The margins leave nothing of page ${pi + 1}.`);
      const a = fr.pt(m.left, m.bottom);
      const b = fr.pt(fr.W - m.right, fr.H - m.top);
      const [x, y] = [Math.min(a[0], b[0]), Math.min(a[1], b[1])];
      const [w, h] = [Math.abs(a[0] - b[0]), Math.abs(a[1] - b[1])];
      page.setCropBox(x, y, w, h);
      page.setMediaBox(x, y, w, h);
    }
    return [await pdfResult(lib, `${baseName(f.name)}-cropped.pdf`)];
  },
});

function updateCropPreview(wb) {
  const o = wb.opt;
  if (!o.mask || !o.top) return;
  const pct = (k, total) => clamp(((+o[k].value || 0) * MM_TO_PT * 100) / total, 0, 95);
  o.mask.style.cssText = `top:${pct("top", o.page.h)}%;bottom:${pct("bottom", o.page.h)}%;left:${pct("left", o.page.w)}%;right:${pct("right", o.page.w)}%`;
}
