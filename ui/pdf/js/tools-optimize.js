/* Optimize and secure: compress, repair, protect, unlock. */
"use strict";

const { PDFArray } = PDFLib;
const COMPRESS_LEVELS = {
  low: { quality: 0.85, maxSide: 2600 },
  recommended: { quality: 0.7, maxSide: 1800 },
  extreme: { quality: 0.5, maxSide: 1200 },
};
/* Pictures smaller than this are left alone: re-encoding them gains almost nothing. */
const COMPRESS_MIN_IMAGE_BYTES = 30 * 1024;

/* 3 for RGB, 1 for gray, null for anything the browser can't be trusted to re-encode (CMYK, indexed…). */
function jpegComponents(lib, dict) {
  const cs = dict.get(PDFName.of("ColorSpace"));
  if (cs === PDFName.of("DeviceRGB")) return 3;
  if (cs === PDFName.of("DeviceGray")) return 1;
  if (cs instanceof PDFArray && cs.get(0) === PDFName.of("ICCBased")) {
    const icc = lib.context.lookup(cs.get(1));
    const n = icc && icc.dict && icc.dict.get(PDFName.of("N"));
    return n instanceof PDFNumber && [1, 3].includes(n.asNumber()) ? n.asNumber() : null;
  }
  return null;
}

/* Re-encodes the JPEG pictures of a document smaller; text and vectors are untouched. */
async function recompressImages(lib, level, progress) {
  const jobs = lib.context.enumerateIndirectObjects().filter(([, obj]) => {
    if (!(obj instanceof PDFRawStream)) return false;
    const d = obj.dict;
    return d.get(PDFName.of("Subtype")) === PDFName.of("Image") && d.get(PDFName.of("Filter")) === PDFName.of("DCTDecode") &&
      !d.has(PDFName.of("Decode")) && !d.has(PDFName.of("Mask")) && obj.contents.length >= COMPRESS_MIN_IMAGE_BYTES && jpegComponents(lib, d);
  });
  let done = 0;
  for (const [ref, obj] of jobs) {
    progress.set(done++ / Math.max(jobs.length, 1), `Optimizing picture ${done} of ${jobs.length}`);
    try {
      const bmp = await createImageBitmap(new Blob([obj.contents], { type: "image/jpeg" }));
      const k = Math.min(1, level.maxSide / Math.max(bmp.width, bmp.height));
      const w = Math.max(1, Math.round(bmp.width * k));
      const h = Math.max(1, Math.round(bmp.height * k));
      const canvas = document.createElement("canvas");
      canvas.width = w;
      canvas.height = h;
      const ctx = canvas.getContext("2d");
      ctx.fillStyle = "#fff";
      ctx.fillRect(0, 0, w, h);
      ctx.drawImage(bmp, 0, 0, w, h);
      bmp.close();
      const bytes = new Uint8Array(await (await canvasBlob(canvas, "image/jpeg", level.quality)).arrayBuffer());
      canvas.width = canvas.height = 0;
      if (bytes.length > obj.contents.length * 0.92) continue;
      const dict = obj.dict.clone();
      dict.set(PDFName.of("Width"), PDFNumber.of(w));
      dict.set(PDFName.of("Height"), PDFNumber.of(h));
      dict.set(PDFName.of("ColorSpace"), PDFName.of("DeviceRGB"));
      dict.set(PDFName.of("BitsPerComponent"), PDFNumber.of(8));
      dict.set(PDFName.of("Length"), PDFNumber.of(bytes.length));
      lib.context.assign(ref, PDFRawStream.of(dict, bytes));
    } catch (e) {
      console.warn("skipped a picture", e);
    }
    if (done % 4 === 0) await frame();
  }
  return jobs.length;
}

defineTool({
  id: "compress", name: "Compress PDF", desc: "Make the file smaller by optimizing the pictures inside it.",
  icon: "compress", group: "Optimize", accept: "pdf", action: "Compress PDF", working: "Compressing…",
  options(wb, side) {
    wb.opt.level = segControl([["low", "Less"], ["recommended", "Recommended"], ["extreme", "Extreme"]], wb.opt.level?.value || "recommended");
    side.append(wb.opt.level, el("p", "muted", "Text stays sharp and selectable. Less keeps picture quality high, Extreme shrinks the most."));
  },
  async run(wb, progress) {
    const [f] = wb.files;
    const { lib } = await openLib(f.bytes, f.name, f.password);
    const count = await recompressImages(lib, COMPRESS_LEVELS[wb.opt.level.value], progress);
    progress.set(1, "Saving…");
    const bytes = await saveLib(lib);
    const before = f.bytes.length;
    if (bytes.length >= before) {
      wb.summary = "This file is already well optimized, so it was not made any smaller. You can try the Extreme level.";
      return [{ name: f.name, bytes: f.bytes, type: "application/pdf" }];
    }
    const saved = Math.round((1 - bytes.length / before) * 100);
    wb.summary = `${fmtBytes(before)} → ${fmtBytes(bytes.length)} (${saved}% smaller${count ? `, ${count} picture${count === 1 ? "" : "s"} optimized` : ""}).`;
    return [{ name: baseName(f.name) + "-compressed.pdf", bytes, type: "application/pdf" }];
  },
});

/* ---------- repair ---------- */
/* Last resort for files pdf-lib cannot read: every page is drawn and saved as a picture. */
async function rebuildFromPictures(f, progress) {
  const pdf = await pdfjsFor(f);
  const out = await PDFDocument.create();
  for (let i = 1; i <= pdf.numPages; i++) {
    progress.set((i - 1) / pdf.numPages, `Rebuilding page ${i} of ${pdf.numPages}`);
    const page = await pdf.getPage(i);
    const viewport = page.getViewport({ scale: 2 });
    const canvas = document.createElement("canvas");
    canvas.width = viewport.width;
    canvas.height = viewport.height;
    await page.render({ canvasContext: canvas.getContext("2d", { alpha: false }), viewport }).promise;
    const jpg = await out.embedJpg(new Uint8Array(await (await canvasBlob(canvas, "image/jpeg", 0.9)).arrayBuffer()));
    const base = page.getViewport({ scale: 1 });
    out.addPage([base.width, base.height]).drawImage(jpg, { x: 0, y: 0, width: base.width, height: base.height });
  }
  return out;
}

defineTool({
  id: "repair", name: "Repair PDF", desc: "Rebuild a damaged PDF so it opens again.",
  icon: "wrench", group: "Optimize", accept: "pdf", action: "Repair PDF", working: "Repairing…",
  options(wb, side) {
    side.append(el("p", "muted", "The file is rebuilt from the parts that can still be read. If that isn’t possible, the pages are redrawn as pictures."));
  },
  async run(wb, progress) {
    const [f] = wb.files;
    const name = baseName(f.name) + "-repaired.pdf";
    try {
      const { lib } = await openLib(f.bytes, f.name, f.password);
      wb.summary = "The file was rebuilt.";
      return [await pdfResult(lib, name)];
    } catch (e) {
      if (e.message === "cancelled") throw e;
      wb.summary = "The structure could not be repaired, so each page was redrawn as a picture (text is no longer selectable).";
      return [await pdfResult(await rebuildFromPictures(f, progress), name)];
    }
  },
});

/* ---------- protect / unlock ---------- */
const randomHex = (n) => [...crypto.getRandomValues(new Uint8Array(n))].map((b) => b.toString(16).padStart(2, "0")).join("");

defineTool({
  id: "protect", name: "Protect PDF", desc: "Lock the file with a password (AES-256).",
  icon: "lock", group: "Security", accept: "pdf", action: "Protect PDF", working: "Encrypting…",
  ready: (wb) => !!wb.opt.pw && wb.opt.pw.value.length > 0 && wb.opt.pw.value === wb.opt.pw2.value,
  options(wb, side) {
    const o = wb.opt;
    o.pw = secretInput("Password");
    o.pw2 = secretInput("Repeat password");
    o.print = checkControl("Allow printing", true);
    o.copy = checkControl("Allow copying text", true);
    o.edit = checkControl("Allow editing", false);
    o.pw.oninput = o.pw2.oninput = () => wb.updateAction();
    side.append(optionRow("Password", o.pw), optionRow("Repeat password", o.pw2), o.print, o.copy, o.edit, el("p", "muted", "Whoever has the password can open the file. Keep it safe: it cannot be recovered."));
  },
  async run(wb) {
    const [f] = wb.files;
    const o = wb.opt;
    const { lib } = await openLib(f.bytes, f.name, f.password);
    lib.encrypt({
      userPassword: o.pw.value,
      ownerPassword: randomHex(16),
      permissions: {
        printing: o.print.input.checked ? "highResolution" : false,
        copying: o.copy.input.checked,
        modifying: o.edit.input.checked,
        annotating: o.edit.input.checked,
        fillingForms: o.edit.input.checked,
        contentAccessibility: true,
        documentAssembly: o.edit.input.checked,
      },
    });
    return [await pdfResult(lib, baseName(f.name) + "-protected.pdf")];
  },
});

defineTool({
  id: "unlock", name: "Unlock PDF", desc: "Remove a password and the restrictions from a PDF you can open.",
  icon: "unlock", group: "Security", accept: "pdf", action: "Unlock PDF", working: "Unlocking…",
  options(wb, side) {
    side.append(el("p", "muted", "You’re asked for the password when you add the file. Only use this on files you are allowed to change."));
  },
  async run(wb) {
    const [f] = wb.files;
    const probe = await PDFDocument.load(f.bytes, { ignoreEncryption: true });
    if (!probe.isEncrypted) throw new UserError("This file isn’t protected.");
    const { lib } = await openLib(f.bytes, f.name, f.password);
    return [await pdfResult(lib, baseName(f.name) + "-unlocked.pdf")];
  },
});
