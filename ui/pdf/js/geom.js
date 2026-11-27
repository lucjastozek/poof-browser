/* PDF page geometry, page-range parsing and loading documents with pdf-lib. */
"use strict";

const {
  PDFDocument,
  StandardFonts,
  rgb,
  degrees,
  PDFName,
  PDFNumber,
  PDFRawStream,
  BlendMode,
} = PDFLib;

/*
 * Maps points of the page as displayed (x from the left, y from the bottom) to PDF user space,
 * honouring the page's own /Rotate and crop box. Anything drawn must also be rotated by `rot`.
 */
function pageFrame(page) {
  const b = page.getCropBox();
  const rot = ((page.getRotation().angle % 360) + 360) % 360;
  const w = b.width;
  const h = b.height;
  const swap = rot === 90 || rot === 270;
  const pt = (dx, dy) => {
    switch (rot) {
      case 90:
        return [b.x + (w - dy), b.y + dx];
      case 180:
        return [b.x + (w - dx), b.y + (h - dy)];
      case 270:
        return [b.x + dy, b.y + (h - dx)];
      default:
        return [b.x + dx, b.y + dy];
    }
  };
  return { W: swap ? h : w, H: swap ? w : h, rot, pt };
}

/*
 * "1-3, 5, 8-" -> [[1,3],[5,5],[8,total]] (1-based, inclusive). Empty text means every page.
 * Throws an Error whose message can be shown to the user.
 */
function parseRanges(text, total) {
  const parts = text.split(/[,;\s]+/).filter(Boolean);
  if (!parts.length) return [[1, total]];
  return parts.map((part) => {
    const m = /^(\d*)(?:-(\d*))?$/.exec(part);
    if (!m || (m[1] === "" && !m[2]))
      throw new UserError(`“${part}” is not a valid page range`);
    const a = m[1] === "" ? 1 : +m[1];
    const b = m[2] === undefined ? a : m[2] === "" ? total : +m[2];
    if (a < 1 || b > total || a > b)
      throw new UserError(`“${part}” is outside this file’s ${total} pages`);
    return [a, b];
  });
}
/* Ranges -> sorted unique 0-based page indices. */
function rangeIndices(ranges) {
  const set = new Set();
  for (const [a, b] of ranges) for (let i = a; i <= b; i++) set.add(i - 1);
  return [...set].sort((x, y) => x - y);
}

/* Loads a file with pdf-lib, asking for the password when it needs one. Returns {lib, password}. */
async function openLib(bytes, name, password) {
  let pw = password ?? "";
  for (let tries = 0; ; tries++) {
    try {
      const lib = await PDFDocument.load(bytes, {
        password: pw,
        throwOnInvalidObject: false,
      });
      return { lib, password: pw };
    } catch (e) {
      let encrypted = false;
      try {
        encrypted = (await PDFDocument.load(bytes, { ignoreEncryption: true }))
          .isEncrypted;
      } catch (_) {}
      if (!encrypted) throw e;
      pw = await askPassword(name, tries > 0);
      if (pw == null) throw new Error("cancelled");
    }
  }
}

/* A file the tools work on: {name, bytes, pages, password}. */
async function describeFile(file) {
  const bytes = file.bytes || (await readBytes(file));
  const name = file.name;
  const { lib, password } = await openLib(bytes, name, file.password);
  return { name, bytes, pages: lib.getPageCount(), password };
}

async function saveLib(lib, opts) {
  return lib.save({ useObjectStreams: true, ...opts });
}
