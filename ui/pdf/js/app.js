/* App shell: views, toolbar, opening files, drag and drop, keyboard. */
"use strict";

const ZOOM_STEPS = [0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4];
let view = "home";

function showView(name) {
  view = name;
  for (const id of ["home", "viewer", "tool"]) $(id).hidden = id !== name;
  document.title =
    name === "viewer" && V.doc
      ? V.doc.name
      : name === "tool"
        ? "PDF tools"
        : "PDF viewer & tools";
  renderBar();
  if (name === "viewer") {
    relayout(true);
    rerenderAll();
    scrollBox().focus({ preventScroll: true });
  }
}

/* ---------- opening ---------- */
async function openInViewer(bytes, name, opts = {}) {
  if (!(await settleEdits())) return;
  const ok = await withBusy("Opening " + name, async () => {
    await openDocument(bytes, name, opts.source);
    return true;
  });
  if (!ok) return;
  showView("viewer");
  if (opts.edit) setAnnotating(true);
}
async function openFile(file, opts) {
  const bytes = await withBusy("Reading " + file.name, () => readBytes(file));
  if (bytes) openInViewer(bytes, file.name, opts);
}
async function openFromDisk() {
  const files = await pickFiles("application/pdf,.pdf", true);
  routeDroppedFiles(files);
}
async function openFromAddress(address) {
  const name =
    decodeURIComponent(
      address.split(/[?#]/)[0].split("/").pop() || "document.pdf",
    ).replace(/^\s+|\s+$/g, "") || "document.pdf";
  const bytes = await withBusy("Opening " + name, async () => {
    const res = await fetch("/fetch?u=" + encodeURIComponent(address), {
      headers: { "x-poof-pdf": "1" },
    });
    if (!res.ok)
      throw new UserError(
        "Couldn’t open this PDF: " + (await res.text()).slice(0, 140),
      );
    return new Uint8Array(await res.arrayBuffer());
  });
  if (!bytes) return showView("home");
  await openInViewer(bytes, name, { source: address });
}

/* One PDF opens in the viewer; several go to Merge; only images go to Images to PDF. */
function routeDroppedFiles(files) {
  if (!files.length) return;
  if (view === "tool" && activeBench) return activeBench.addFiles(files);
  const pdfs = files.filter(isPdfFile);
  const images = files.filter(isImageFile);
  if (pdfs.length === 1) openFile(pdfs[0]);
  else if (pdfs.length > 1)
    openTool(
      PDF_TOOLS.find((t) => t.id === "merge"),
      pdfs,
    );
  else if (images.length)
    openTool(
      PDF_TOOLS.find((t) => t.id === "img2pdf"),
      images,
    );
  else toast("Drop PDF or image files.");
}

/* Pending edits are never lost silently. Returns whether to go on. */
async function settleEdits() {
  if (!V.doc || !Ann.items.length) return true;
  const body = el(
    "p",
    "",
    "You have edits that are not saved into the file yet.",
  );
  const choice = await dialog("Save your edits?", body, [
    { label: "Cancel" },
    { label: "Discard edits" },
    { label: "Save and continue", primary: true },
  ]);
  if (choice === 2) {
    await saveEdited();
    return !Ann.items.length;
  }
  if (choice === 1) {
    Ann.items = [];
    drawAll();
    return true;
  }
  return false;
}
async function finishEditing() {
  if (Ann.items.length && !(await settleEdits())) return;
  setAnnotating(false);
}
const currentFile = () => ({
  name: V.doc.name.replace(/\.pdf$/i, "") + ".pdf",
  type: "application/pdf",
  bytes: V.doc.bytes,
  password: V.doc.password,
});

async function useToolOnDocument(tool) {
  if (!(await settleEdits())) return;
  startTool(tool, [currentFile()]);
}
async function downloadDocument() {
  if (!V.doc) return;
  if (Ann.items.length) return saveEdited();
  saveBytes(V.doc.bytes, V.doc.name.replace(/\.pdf$/i, "") + ".pdf");
}

/* ---------- toolbar ---------- */
function openMenu(anchor, items) {
  document.querySelectorAll(".menu").forEach((m) => m.remove());
  const menu = el("div", "menu");
  menu.setAttribute("role", "menu");
  for (const item of items) {
    if (item === "sep") {
      menu.append(el("div", "menu-sep"));
      continue;
    }
    const b = el("button", "menu-item" + (item.on ? " on" : ""));
    b.type = "button";
    b.setAttribute("role", "menuitem");
    b.append(ic(item.icon || "file", 16), el("span", "", item.label));
    b.onclick = () => {
      menu.remove();
      item.run();
    };
    menu.append(b);
  }
  const r = anchor.getBoundingClientRect();
  menu.style.top = r.bottom + 6 + "px";
  menu.style.right = Math.max(8, innerWidth - r.right) + "px";
  $("layer").append(menu);
  const close = (e) => {
    if (e.type === "keydown" && e.key !== "Escape") return;
    if (e.type === "pointerdown" && menu.contains(e.target)) return;
    menu.remove();
    document.removeEventListener("pointerdown", close, true);
    document.removeEventListener("keydown", close, true);
  };
  setTimeout(() => {
    document.addEventListener("pointerdown", close, true);
    document.addEventListener("keydown", close, true);
  });
  menu.querySelector("button")?.focus();
}
const toolsMenu = (anchor) =>
  openMenu(
    anchor,
    PDF_TOOLS.map((t) => ({
      label: t.name,
      icon: t.icon,
      run: () => useToolOnDocument(t),
    })).concat([
      "sep",
      { label: "All tools", icon: "grid", run: () => showView("home") },
    ]),
  );

function renderBar() {
  const bar = $("bar");
  bar.replaceChildren();
  if (view === "viewer") return viewerBar(bar);
  const left = el("div", "bar-left");
  const brand = el("div", "brand");
  brand.append(ic("file", 18), el("span", "", "PDF"));
  left.append(brand);
  if (V.doc)
    left.append(
      btn(V.doc.name, "left", () => showView("viewer"), "ghost back-doc"),
    );
  const right = el("div", "bar-right");
  right.append(btn("Open PDF", "folder-open", openFromDisk, "small"));
  bar.append(left, el("div", "bar-mid"), right);
}

let pageInput;
let zoomSelect;
function viewerBar(bar) {
  const left = el("div", "bar-left");
  const side = iconBtn(
    "sidebar-left",
    "Pages and outline",
    () => toggleSidePanel(),
    "toggle",
  );
  const name = el("button", "doc-name", V.doc.name);
  name.type = "button";
  name.title = V.doc.name + " — properties";
  name.onclick = showProperties;
  left.append(side, name);

  const mid = el("div", "bar-mid");
  pageInput = el("input", "field page-input");
  pageInput.value = V.current;
  pageInput.setAttribute("aria-label", "Page number");
  pageInput.onkeydown = (e) => {
    if (e.key === "Enter") goToPage(parseInt(pageInput.value, 10) || V.current);
    e.stopPropagation();
  };
  pageInput.onfocus = () => pageInput.select();
  pageInput.onblur = () => (pageInput.value = V.current);
  zoomSelect = el("select", "field zoom-select");
  zoomSelect.setAttribute("aria-label", "Zoom");
  [
    ["width", "Fit width"],
    ["page", "Fit page"],
    ...ZOOM_STEPS.map((z) => [String(z), Math.round(z * 100) + "%"]),
  ].forEach(([v, l]) => zoomSelect.append(new Option(l, v)));
  zoomSelect.onchange = () => {
    const v = zoomSelect.value;
    setZoom(v === "width" || v === "page" ? v : +v);
    zoomSelect.blur();
  };
  mid.append(
    iconBtn("up", "Previous page", () => goToPage(V.current - 1)),
    pageInput,
    el("span", "muted of", "/ " + V.doc.n),
    iconBtn("down", "Next page", () => goToPage(V.current + 1)),
    el("span", "sep"),
    iconBtn("minimize", "Zoom out", () => zoomBy(1 / 1.2)),
    zoomSelect,
    iconBtn("add", "Zoom in", () => zoomBy(1.2)),
    iconBtn(
      V.zoomMode === "width" ? "fullscreen" : "unfold-horizontal",
      "Toggle fit width / fit page",
      () => setZoom(V.zoomMode === "width" ? "page" : "width"),
    ),
  );

  const right = el("div", "bar-right");
  const edit = btn(
    "Edit",
    "edit-2",
    () => setAnnotating(!Ann.active),
    "small" + (Ann.active ? " on" : ""),
  );
  edit.id = "editBtn";
  const tools = btn("Tools", "grid", () => toolsMenu(tools), "small");
  const more = iconBtn("more-1", "More", () =>
    openMenu(more, [
      {
        label: "Rotate view left",
        icon: "anticlockwise",
        run: () => rotateView(-90),
      },
      {
        label: "Rotate view right",
        icon: "clockwise",
        run: () => rotateView(90),
      },
      {
        label: "Dark pages",
        icon: "moon",
        on: $("scroll").classList.contains("dark-pages"),
        run: () => $("scroll").classList.toggle("dark-pages"),
      },
      "sep",
      { label: "Print…", icon: "print", run: printDocument },
      {
        label: "Document properties",
        icon: "information",
        run: showProperties,
      },
      { label: "Open another PDF…", icon: "folder-open", run: openFromDisk },
    ]),
  );
  right.append(
    iconBtn("search", "Find (Ctrl+F)", openFind),
    edit,
    tools,
    iconBtn("download-2", "Download (Ctrl+S)", downloadDocument),
    more,
  );
  bar.append(left, mid, right);
  syncZoomSelect();
}
function syncZoomSelect() {
  if (!zoomSelect || view !== "viewer") return;
  const mode = V.zoomMode;
  zoomSelect.querySelector(".custom")?.remove();
  if (typeof mode === "string") return void (zoomSelect.value = mode);
  const match = ZOOM_STEPS.find((z) => Math.abs(z - mode) < 0.005);
  if (match) return void (zoomSelect.value = String(match));
  const o = new Option(Math.round(mode * 100) + "%", String(mode));
  o.className = "custom";
  zoomSelect.append(o);
  zoomSelect.value = String(mode);
}
V.on("page", (n) => {
  if (pageInput && document.activeElement !== pageInput) pageInput.value = n;
});
V.on("zoom", syncZoomSelect);
V.on("annotating", (on) => $("editBtn")?.classList.toggle("on", on));
V.on("doc", () => view === "viewer" && renderBar());

/* ---------- drag and drop ---------- */
let dragDepth = 0;
function initDrop() {
  const hasFiles = (e) => [...(e.dataTransfer?.types || [])].includes("Files");
  addEventListener("dragenter", (e) => {
    if (!hasFiles(e)) return;
    dragDepth++;
    $("drop").hidden = false;
  });
  addEventListener("dragleave", (e) => {
    if (!hasFiles(e)) return;
    if (--dragDepth <= 0) ($("drop").hidden = true), (dragDepth = 0);
  });
  addEventListener("dragover", (e) => hasFiles(e) && e.preventDefault());
  addEventListener("drop", (e) => {
    if (!hasFiles(e)) return;
    e.preventDefault();
    dragDepth = 0;
    $("drop").hidden = true;
    routeDroppedFiles([...e.dataTransfer.files]);
  });
}

/* ---------- keyboard ---------- */
function initKeys() {
  addEventListener("keydown", (e) => {
    if (document.querySelector(".scrim")) return;
    if (onAnnotationKey(e)) return;
    const mod = e.ctrlKey || e.metaKey;
    const key = e.key.toLowerCase();
    const typing = /^(INPUT|TEXTAREA|SELECT)$/.test(e.target.tagName);
    if (mod && key === "o") e.preventDefault(), openFromDisk();
    else if (view !== "viewer") return;
    else if (mod && key === "f") e.preventDefault(), openFind();
    else if (mod && key === "s") e.preventDefault(), downloadDocument();
    else if (mod && key === "p") e.preventDefault(), printDocument();
    else if (mod && (key === "=" || key === "+"))
      e.preventDefault(), zoomBy(1.2);
    else if (mod && key === "-") e.preventDefault(), zoomBy(1 / 1.2);
    else if (mod && key === "0") e.preventDefault(), setZoom("width");
    else if ((key === "f3" || (mod && key === "g")) && Find.q)
      e.preventDefault(), stepFind(e.shiftKey ? -1 : 1);
    else if (!typing && key === "home") e.preventDefault(), goToPage(1);
    else if (!typing && key === "end") e.preventDefault(), goToPage(V.doc.n);
    else if (!typing && !mod && key === "n") goToPage(V.current + 1);
    else if (!typing && !mod && key === "p") goToPage(V.current - 1);
  });
}

/* ---------- boot ---------- */
function boot() {
  initViewerInput();
  initDrop();
  initKeys();
  buildHome();
  const source = new URLSearchParams(location.search).get("u");
  if (source) openFromAddress(source);
  else showView("home");
}
boot();
