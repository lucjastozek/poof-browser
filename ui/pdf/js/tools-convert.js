/* Conversions: images to PDF, PDF to images, PDF to text. */
"use strict";

const PX_TO_PT = 0.75; // 96 dpi screens to 72 dpi points
const PAGE_SIZES = { a4: [595.28, 841.89], letter: [612, 792] };
const MARGINS = { none: 0, small: 20, big: 50 };

/* Decodes an image file and re-encodes it for embedding (PNG keeps transparency, the rest JPEG). */
async function imageForPdf(file) {
  const bmp = await createImageBitmap(file);
  const canvas = document.createElement("canvas");
  canvas.width = bmp.width;
  canvas.height = bmp.height;
  const ctx = canvas.getContext("2d");
  const png = /png|gif|webp|avif/i.test(file.type);
  if (!png) {
    ctx.fillStyle = "#fff";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
  }
  ctx.drawImage(bmp, 0, 0);
  const blob = await canvasBlob(canvas, png ? "image/png" : "image/jpeg", 0.95);
  const bytes = new Uint8Array(await blob.arrayBuffer());
  const out = { bytes, png, w: bmp.width, h: bmp.height };
  bmp.close();
  canvas.width = canvas.height = 0;
  return out;
}

defineTool({
  id: "img2pdf",
  name: "Images to PDF",
  desc: "Turn JPG, PNG and other images into one PDF.",
  icon: "pic",
  group: "Convert to PDF",
  accept: "image",
  multi: true,
  action: "Create PDF",
  working: "Creating PDF…",
  options(wb, side) {
    const o = wb.opt;
    o.size = selectControl(
      [
        ["fit", "Same as image"],
        ["a4", "A4"],
        ["letter", "US Letter"],
      ],
      o.size?.value || "fit",
    );
    o.orient = segControl(
      [
        ["auto", "Auto"],
        ["portrait", "Portrait"],
        ["landscape", "Landscape"],
      ],
      o.orient?.value || "auto",
    );
    o.margin = segControl(
      [
        ["none", "None"],
        ["small", "Small"],
        ["big", "Big"],
      ],
      o.margin?.value || "none",
    );
    side.append(
      optionRow("Page size", o.size),
      optionRow("Orientation", o.orient),
      optionRow("Margin", o.margin),
    );
  },
  async run(wb, progress) {
    const o = wb.opt;
    const out = await PDFDocument.create();
    for (const [i, f] of wb.files.entries()) {
      progress.set(i / wb.files.length, `Adding ${f.name}`);
      const img = await imageForPdf(f.file);
      const embedded = img.png
        ? await out.embedPng(img.bytes)
        : await out.embedJpg(img.bytes);
      let [pw, ph] =
        o.size.value === "fit"
          ? [img.w * PX_TO_PT, img.h * PX_TO_PT]
          : PAGE_SIZES[o.size.value];
      if (o.size.value !== "fit") {
        const landscape =
          o.orient.value === "landscape" ||
          (o.orient.value === "auto" && img.w > img.h);
        if (landscape !== pw > ph) [pw, ph] = [ph, pw];
      }
      const m = o.size.value === "fit" ? 0 : MARGINS[o.margin.value];
      const k = Math.min((pw - 2 * m) / img.w, (ph - 2 * m) / img.h);
      const w = img.w * k;
      const h = img.h * k;
      out.addPage([pw, ph]).drawImage(embedded, {
        x: (pw - w) / 2,
        y: (ph - h) / 2,
        width: w,
        height: h,
      });
      await frame();
    }
    return [await pdfResult(out, "images.pdf")];
  },
});

defineTool({
  id: "pdf2img",
  name: "PDF to images",
  desc: "Save every page as a JPG or PNG picture.",
  icon: "pic",
  group: "Convert from PDF",
  accept: "pdf",
  action: "Convert to images",
  working: "Converting…",
  options(wb, side) {
    const o = wb.opt;
    o.format = segControl(
      [
        ["jpg", "JPG"],
        ["png", "PNG"],
      ],
      o.format?.value || "jpg",
    );
    o.dpi = selectControl(
      [
        ["72", "Screen (72 dpi)"],
        ["150", "Standard (150 dpi)"],
        ["300", "High (300 dpi)"],
      ],
      o.dpi?.value || "150",
    );
    o.pages = el("input", "field");
    o.pages.placeholder = `All ${wb.files[0].pages} pages`;
    side.append(
      optionRow("Format", o.format),
      optionRow("Quality", o.dpi),
      optionRow("Pages", o.pages, "Optional, e.g. 1-3, 5"),
    );
  },
  async run(wb, progress) {
    const [f] = wb.files;
    const o = wb.opt;
    const jpg = o.format.value === "jpg";
    const indices = rangeIndices(parseRanges(o.pages.value, f.pages));
    const scale = +o.dpi.value / 72;
    const pdf = await pdfjsFor(f);
    const results = [];
    for (const [n, i] of indices.entries()) {
      progress.set(
        n / indices.length,
        `Page ${i + 1} (${n + 1} of ${indices.length})`,
      );
      const page = await pdf.getPage(i + 1);
      const viewport = page.getViewport({ scale });
      const canvas = document.createElement("canvas");
      canvas.width = Math.floor(viewport.width);
      canvas.height = Math.floor(viewport.height);
      await page.render({
        canvasContext: canvas.getContext("2d", { alpha: false }),
        viewport,
      }).promise;
      const blob = await canvasBlob(
        canvas,
        jpg ? "image/jpeg" : "image/png",
        0.92,
      );
      results.push({
        name: `${baseName(f.name)}-page-${i + 1}.${o.format.value}`,
        bytes: new Uint8Array(await blob.arrayBuffer()),
        type: blob.type,
      });
      canvas.width = canvas.height = 0;
    }
    return results;
  },
});

defineTool({
  id: "pdf2txt",
  name: "PDF to text",
  desc: "Extract the text of a PDF into a .txt file.",
  icon: "text",
  group: "Convert from PDF",
  accept: "pdf",
  action: "Extract text",
  working: "Reading text…",
  options(wb, side) {
    wb.opt.marks = checkControl("Mark where each page starts", true);
    side.append(
      wb.opt.marks,
      el(
        "p",
        "muted",
        "Scanned pages hold pictures instead of text and produce nothing.",
      ),
    );
  },
  async run(wb, progress) {
    const [f] = wb.files;
    const pdf = await pdfjsFor(f);
    const parts = [];
    for (let i = 1; i <= f.pages; i++) {
      progress.set((i - 1) / f.pages, `Page ${i} of ${f.pages}`);
      const content = await (await pdf.getPage(i)).getTextContent();
      let text = "";
      let lastY = null;
      for (const it of content.items) {
        if (typeof it.str !== "string") continue;
        const y = it.transform[5];
        if (
          lastY !== null &&
          Math.abs(y - lastY) > (it.height || 10) * 0.6 &&
          !text.endsWith("\n")
        )
          text += "\n";
        text += it.str + (it.hasEOL ? "\n" : "");
        lastY = y;
      }
      parts.push(
        (wb.opt.marks.input.checked ? `--- Page ${i} ---\n` : "") + text.trim(),
      );
    }
    const bytes = new TextEncoder().encode(parts.join("\n\n"));
    return [{ name: baseName(f.name) + ".txt", bytes, type: "text/plain" }];
  },
});
