/* Writes the editing layer into a copy of the PDF with pdf-lib. */
"use strict";

const { LineCapStyle } = PDFLib;
/* Standard PDF fonts per family: regular, bold, italic, bold italic. */
const EXPORT_FONTS = {
  sans: [
    "Helvetica",
    "HelveticaBold",
    "HelveticaOblique",
    "HelveticaBoldOblique",
  ],
  serif: [
    "TimesRoman",
    "TimesRomanBold",
    "TimesRomanItalic",
    "TimesRomanBoldItalic",
  ],
  mono: ["Courier", "CourierBold", "CourierOblique", "CourierBoldOblique"],
};

function hexColor(hex) {
  const n = parseInt(hex.slice(1), 16);
  return rgb(((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255);
}

/* Returns the edited file's bytes. Items are in displayed coordinates (top-left origin). */
async function exportAnnotated(bytes, name, password) {
  const { lib } = await openLib(bytes, name, password);
  const fonts = new Map();
  const fontFor = async (family = "sans", bold = false, italic = false) => {
    const key = family + bold + italic;
    if (!fonts.has(key)) {
      const names = EXPORT_FONTS[family][(bold ? 1 : 0) + (italic ? 2 : 0)];
      const font = await lib.embedFont(StandardFonts[names]);
      fonts.set(key, { font, encodable: new Set(font.getCharacterSet()) });
    }
    return fonts.get(key);
  };
  const pages = lib.getPages();
  let lossy = false;
  /* Characters the standard fonts lack become "?". */
  const safe = (raw, enc) =>
    [...raw]
      .map((c) => (enc.has(c.codePointAt(0)) ? c : ((lossy = true), "?")))
      .join("");
  const embedded = new Map();

  for (const item of Ann.items) {
    const page = pages[item.page - 1];
    if (!page) continue;
    const f = pageFrame(page);
    const at = (x, y) => f.pt(x, f.H - y); // displayed top-left coords -> user space
    const rotate = degrees(f.rot);
    const color = hexColor(item.color || "#000000");
    const line = (a, b, thickness) => {
      const [x1, y1] = at(a[0], a[1]);
      const [x2, y2] = at(b[0], b[1]);
      page.drawLine({
        start: { x: x1, y: y1 },
        end: { x: x2, y: y2 },
        thickness,
        color,
        lineCap: LineCapStyle.Round,
      });
    };
    switch (item.type) {
      case "replace": {
        const [cx, cy] = at(item.x - 1, item.y + item.h);
        page.drawRectangle({
          x: cx,
          y: cy,
          width: item.w + 2,
          height: item.h,
          color: hexColor(item.bg),
          rotate,
        });
        const { font, encodable } = await fontFor(
          GENERIC[item.font] || "sans",
          item.bold,
          item.italic,
        );
        const text = safe(item.text, encodable);
        if (!text.trim()) break;
        const [x, y] = at(item.x, item.base);
        page.drawText(text, { x, y, size: item.size, font, color, rotate });
        break;
      }
      case "text": {
        const { font, encodable } = await fontFor(
          GENERIC[item.font] || "sans",
          item.bold,
          item.italic,
        );
        textLines(item).forEach((raw, i) => {
          const text = safe(raw, encodable);
          if (!text.trim()) return;
          const [x, y] = at(
            item.x,
            item.y + item.size * (TEXT_BASELINE + i * LINE_HEIGHT),
          );
          page.drawText(text, { x, y, size: item.size, font, color, rotate });
        });
        break;
      }
      case "cover": {
        const [x, y] = at(item.x, item.y + item.h);
        page.drawRectangle({
          x,
          y,
          width: item.w,
          height: item.h,
          color: hexColor(item.bg),
          rotate,
        });
        break;
      }
      case "highlight": {
        const [x, y] = at(item.x, item.y + item.h);
        page.drawRectangle({
          x,
          y,
          width: item.w,
          height: item.h,
          color,
          opacity: 0.35,
          blendMode: BlendMode.Multiply,
          rotate,
        });
        break;
      }
      case "rect": {
        const [x, y] = at(item.x, item.y + item.h);
        page.drawRectangle({
          x,
          y,
          width: item.w,
          height: item.h,
          borderColor: color,
          borderWidth: item.width,
          rotate,
        });
        break;
      }
      case "ellipse": {
        const [x, y] = at(item.x + item.w / 2, item.y + item.h / 2);
        page.drawEllipse({
          x,
          y,
          xScale: item.w / 2,
          yScale: item.h / 2,
          borderColor: color,
          borderWidth: item.width,
          rotate,
        });
        break;
      }
      case "line":
      case "arrow":
        line([item.x1, item.y1], [item.x2, item.y2], item.width);
        if (item.type === "arrow")
          for (const tip of arrowHead(item))
            line([item.x2, item.y2], tip, item.width);
        break;
      case "draw":
        for (let i = 1; i < item.pts.length; i++)
          line(item.pts[i - 1], item.pts[i], item.width);
        break;
      case "image": {
        const src = Ann.imgs.get(item.img);
        if (!embedded.has(item.img)) {
          const data = await (await fetch(src)).arrayBuffer();
          embedded.set(item.img, await lib.embedPng(data));
        }
        const [x, y] = at(item.x, item.y + item.h);
        page.drawImage(embedded.get(item.img), {
          x,
          y,
          width: item.w,
          height: item.h,
          rotate,
        });
        break;
      }
    }
  }
  if (lossy)
    toast(
      "Some characters can’t be written with the standard font and were replaced by “?”.",
      6000,
    );
  return saveLib(lib);
}

/* Toolbar “Save”: write the edits into the file, download it and keep working on the result. */
async function saveEdited() {
  if (!V.doc) return;
  if (!Ann.items.length)
    return toast("Nothing to save yet. Add text, shapes or a signature first.");
  const doc = V.doc;
  const out = await withBusy("Saving…", () =>
    exportAnnotated(doc.bytes, doc.name, doc.password),
  );
  if (!out) return;
  const name = baseName(doc.name) + "-edited.pdf";
  saveBytes(out, name);
  setAnnotating(false);
  await openDocument(out, name, doc.source);
  toast("Saved as " + name);
}
