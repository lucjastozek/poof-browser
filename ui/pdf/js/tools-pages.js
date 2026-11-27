/* Page tools: merge, split, remove, extract, organize and rotate. */
"use strict";

/* A new document holding `indices` (0-based, repeats allowed) of `lib`, in that order. */
async function subsetDocument(lib, indices) {
  const out = await PDFDocument.create();
  const pages = await out.copyPages(lib, indices);
  pages.forEach((p) => out.addPage(p));
  return out;
}
const pdfResult = async (doc, name) => ({ name, bytes: await saveLib(doc), type: "application/pdf" });

/* ---------- merge ---------- */
defineTool({
  id: "merge", name: "Merge PDF", desc: "Combine several PDFs into one, in the order you choose.",
  icon: "merge", group: "Organize", accept: "pdf", multi: true, action: "Merge PDF", working: "Merging…",
  ready: (wb) => wb.files.length >= 2,
  options(wb, side) {
    const pages = wb.files.reduce((n, f) => n + f.pages, 0);
    side.append(el("p", "muted", wb.files.length < 2 ? "Add at least two files." : `${wb.files.length} files · ${pages} pages in total. Use the arrows to change the order.`));
  },
  async run(wb, progress) {
    const out = await PDFDocument.create();
    for (const [i, f] of wb.files.entries()) {
      progress.set(i / wb.files.length, `Adding ${f.name}`);
      const { lib } = await openLib(f.bytes, f.name, f.password);
      (await out.copyPages(lib, lib.getPageIndices())).forEach((p) => out.addPage(p));
      await frame();
    }
    return [await pdfResult(out, "merged.pdf")];
  },
});

/* ---------- split ---------- */
defineTool({
  id: "split", name: "Split PDF", desc: "Cut a PDF into several files by page ranges, size or page.",
  icon: "split", group: "Organize", accept: "pdf", action: "Split PDF", working: "Splitting…",
  options(wb, side) {
    const o = wb.opt;
    o.mode = segControl([["ranges", "Ranges"], ["every", "Every N"], ["single", "Each page"]], o.mode?.value || "ranges", () => show());
    o.ranges = el("input", "field");
    o.ranges.placeholder = "e.g. 1-3, 5, 8-";
    o.ranges.value = o.ranges.value || "";
    o.merge = checkControl("Merge all ranges into one PDF", false);
    o.every = numberControl(1, 1, wb.files[0].pages);
    const rangeBox = el("div", "stack");
    rangeBox.append(optionRow("Page ranges", o.ranges, `The file has ${wb.files[0].pages} pages. Each range becomes its own file.`), o.merge);
    const everyBox = el("div", "stack");
    everyBox.append(optionRow("Pages per file", o.every));
    const show = () => {
      rangeBox.hidden = o.mode.value !== "ranges";
      everyBox.hidden = o.mode.value !== "every";
    };
    side.append(o.mode, rangeBox, everyBox);
    show();
  },
  async run(wb) {
    const [f] = wb.files;
    const o = wb.opt;
    const { lib } = await openLib(f.bytes, f.name, f.password);
    const base = baseName(f.name);
    const total = f.pages;
    let groups; // [{label, indices}]
    if (o.mode.value === "single") groups = Array.from({ length: total }, (_, i) => ({ label: "page-" + (i + 1), indices: [i] }));
    else if (o.mode.value === "every") {
      const n = Math.max(1, Math.floor(+o.every.value || 1));
      groups = [];
      for (let s = 0; s < total; s += n) groups.push({ label: `${s + 1}-${Math.min(total, s + n)}`, indices: rangeIndices([[s + 1, Math.min(total, s + n)]]) });
    } else {
      const ranges = parseRanges(o.ranges.value, total);
      groups = o.merge.input.checked
        ? [{ label: "split", indices: ranges.flatMap(([a, b]) => rangeIndices([[a, b]])) }]
        : ranges.map(([a, b]) => ({ label: a === b ? "page-" + a : `${a}-${b}`, indices: rangeIndices([[a, b]]) }));
    }
    if (groups.length === 1 && o.mode.value !== "ranges") throw new UserError("That would produce a single file identical to the original.");
    const results = [];
    for (const g of groups) results.push(await pdfResult(await subsetDocument(lib, g.indices), `${base}-${g.label}.pdf`));
    wb.summary = `${f.name} was split into ${results.length} file${results.length === 1 ? "" : "s"}.`;
    return results;
  },
});

/* ---------- remove / extract ---------- */
function selectTool({ id, name, desc, icon, tone, action, hint }) {
  defineTool({
    id, name, desc, icon, group: "Organize", accept: "pdf", action, working: "Working…",
    ready: (wb) => !!wb.grid && wb.grid.selected.size > 0,
    main(wb, main) {
      wb.grid?.destroy();
      wb.grid = pageGrid(wb, main, {
        mode: "select",
        tone,
        onChange: (g) => {
          wb.opt.text.value = indicesToText(g.selected);
          wb.updateAction();
        },
      });
    },
    options(wb, side) {
      const o = wb.opt;
      o.text = el("input", "field");
      o.text.placeholder = "e.g. 1-3, 5, 8-";
      o.text.oninput = () => {
        try {
          wb.grid.setSelection(new Set(rangeIndices(parseRanges(o.text.value, wb.files[0].pages))));
          o.text.classList.remove("bad");
        } catch (_) {
          o.text.classList.add("bad");
        }
        if (!o.text.value.trim()) wb.grid.setSelection(new Set());
        wb.updateAction();
      };
      side.append(el("p", "muted", hint), optionRow("Pages", o.text));
      if (id === "extract") {
        o.separate = checkControl("Save each page as its own file", false);
        side.append(o.separate);
      }
    },
    async run(wb) {
      const [f] = wb.files;
      const chosen = [...wb.grid.selected].sort((a, b) => a - b);
      const keep = tone === "remove" ? [...Array(f.pages).keys()].filter((i) => !wb.grid.selected.has(i)) : chosen;
      if (!keep.length) throw new UserError("That would remove every page.");
      const { lib } = await openLib(f.bytes, f.name, f.password);
      const base = baseName(f.name);
      if (wb.opt.separate?.input.checked) {
        const files = [];
        for (const i of keep) files.push(await pdfResult(await subsetDocument(lib, [i]), `${base}-page-${i + 1}.pdf`));
        return files;
      }
      return [await pdfResult(await subsetDocument(lib, keep), `${base}-${tone === "remove" ? "edited" : "extract"}.pdf`)];
    },
  });
}
selectTool({ id: "remove", name: "Remove pages", desc: "Delete the pages you don’t need.", icon: "trash", tone: "remove", action: "Remove pages", hint: "Click the pages to delete, or type their numbers." });
selectTool({ id: "extract", name: "Extract pages", desc: "Keep only the pages you choose.", icon: "extract", tone: "keep", action: "Extract pages", hint: "Click the pages to keep, or type their numbers." });

/* ---------- organize / rotate ---------- */
function organizeTool({ id, name, desc, icon, action, suffix, hint }) {
  defineTool({
    id, name, desc, icon, group: id === "rotate" ? "Edit" : "Organize", accept: "pdf", action, working: "Working…",
    main(wb, main) {
      wb.grid?.destroy();
      wb.grid = pageGrid(wb, main, { mode: "organize" });
    },
    options(wb, side) {
      const row = el("div", "btn-row");
      row.append(btn("Rotate all", "rotateL", () => wb.grid.rotateAll(-90), "small"), btn("Rotate all", "rotate", () => wb.grid.rotateAll(90), "small"));
      side.append(el("p", "muted", hint), row);
      if (id === "organize") side.append(btn("Add blank page", "blank", () => wb.grid.addBlank(), "small"));
      side.append(btn("Reset", null, () => wb.render(), "small ghost"));
    },
    async run(wb) {
      const [f] = wb.files;
      const { lib } = await openLib(f.bytes, f.name, f.password);
      const items = wb.grid.items.filter((i) => !i.del);
      if (!items.length) throw new UserError("Every page is marked for deletion.");
      const out = await PDFDocument.create();
      const real = items.filter((i) => i.src >= 0);
      const copies = await out.copyPages(lib, real.map((i) => i.src));
      const byItem = new Map(real.map((item, k) => [item, copies[k]]));
      let size = [595.28, 841.89];
      for (const item of items) {
        let page = byItem.get(item);
        if (page) {
          page.setRotation(degrees((page.getRotation().angle + item.rot + 360) % 360));
          size = [page.getWidth(), page.getHeight()];
          out.addPage(page);
        } else out.addPage(size);
      }
      return [await pdfResult(out, `${baseName(f.name)}-${suffix}.pdf`)];
    },
  });
}
organizeTool({ id: "organize", name: "Organize pages", desc: "Reorder, rotate, duplicate or delete pages, and add blank ones.", icon: "pages", action: "Save PDF", suffix: "organized", hint: "Drag pages to reorder them. Hover a page for its tools." });
organizeTool({ id: "rotate", name: "Rotate PDF", desc: "Turn every page, or only some, by 90° steps.", icon: "rotate", action: "Save rotated PDF", suffix: "rotated", hint: "Rotate all pages at once, or hover a page to turn just that one." });
