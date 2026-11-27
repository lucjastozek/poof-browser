/* The tools home page and the shared workbench every tool runs in. */
"use strict";

const PDF_TOOLS = [];
const TOOL_GROUPS = [
  "Organize",
  "Optimize",
  "Convert to PDF",
  "Convert from PDF",
  "Edit",
  "Security",
];
const defineTool = (t) => PDF_TOOLS.push(t);
let activeBench = null;

/* ---------- home ---------- */
function buildHome() {
  const home = $("home");
  const wrap = el("div", "home-wrap");
  const hero = el("div", "hero");
  hero.append(el("h1", "", "PDF viewer & tools"));
  hero.append(
    el(
      "p",
      "muted",
      "Open a file to read it, or pick a tool. Everything happens on this computer; your files are never uploaded.",
    ),
  );
  const row = el("div", "hero-actions");
  row.append(
    btn("Open PDF", "folder-open", () => openFromDisk(), "primary big"),
  );
  hero.append(row, el("p", "hint", "or drop files anywhere in this window"));
  wrap.append(hero);
  for (const group of TOOL_GROUPS) {
    const tools = PDF_TOOLS.filter((t) => t.group === group);
    if (!tools.length) continue;
    const sec = el("section", "group");
    sec.append(el("h2", "", group));
    const grid = el("div", "cards");
    for (const t of tools) {
      const card = el("button", "card");
      card.type = "button";
      const badge = el("span", "badge");
      badge.append(ic(t.icon, 22));
      card.append(badge, el("strong", "", t.name), el("span", "muted", t.desc));
      card.onclick = () => startTool(t);
      grid.append(card);
    }
    sec.append(grid);
    wrap.append(sec);
  }
  home.replaceChildren(wrap);
}

/* A tool chosen from the grid or the viewer's menu; `files` may carry the open document. */
function startTool(tool, files = []) {
  if (tool.custom) return tool.custom(files);
  openTool(tool, files);
}

/* ---------- workbench ---------- */
function openTool(tool, files = []) {
  showView("tool");
  const root = $("tool");
  const wb = { tool, files: [], root, opt: {}, grid: null, busy: false };
  activeBench = wb;
  const accepts = (f) =>
    tool.accept === "image" ? isImageFile(f) : isPdfFile(f);

  wb.addFiles = async (list) => {
    const fresh = [...list].filter(accepts);
    if (!fresh.length)
      return toast(
        tool.accept === "image"
          ? "Please choose image files."
          : "Please choose PDF files.",
      );
    const picked = tool.multi ? fresh : fresh.slice(0, 1);
    const added = [];
    for (const f of picked) {
      try {
        added.push(
          tool.accept === "image"
            ? { name: f.name, file: f, size: f.size, pages: 1 }
            : await describeFile(f),
        );
      } catch (e) {
        if (e.message !== "cancelled")
          toast(`Couldn’t read ${f.name}. It may be damaged.`, 5000);
      }
    }
    if (!added.length) return;
    wb.files = tool.multi ? [...wb.files, ...added] : added;
    wb.render();
  };
  wb.chooseFiles = async () =>
    wb.addFiles(
      await pickFiles(
        tool.accept === "image" ? "image/*" : "application/pdf,.pdf",
        tool.multi,
      ),
    );
  wb.updateAction = () => {
    if (wb.action) wb.action.disabled = tool.ready ? !tool.ready(wb) : false;
  };
  wb.render = () => renderBench(wb);
  wb.render();
  if (files.length) wb.addFiles(files);
}

function benchHeader(wb) {
  const head = el("header", "wb-head");
  const back = btn("All tools", "left", () => showView("home"), "ghost");
  const title = el("div", "wb-title");
  title.append(el("h1", "", wb.tool.name), el("p", "muted", wb.tool.desc));
  head.append(back, title);
  return head;
}

function renderBench(wb) {
  const { tool } = wb;
  const body = el("div", "wb-body");
  wb.root.replaceChildren(benchHeader(wb), body);
  if (!wb.files.length) {
    const drop = el("div", "dropzone");
    drop.append(
      ic(tool.icon, 40),
      btn(
        tool.multi ? "Select files" : "Select file",
        "folder-open",
        wb.chooseFiles,
        "primary big",
      ),
    );
    drop.append(
      el(
        "p",
        "muted",
        tool.accept === "image"
          ? "or drop images here"
          : tool.multi
            ? "or drop PDF files here"
            : "or drop a PDF file here",
      ),
    );
    body.append(drop);
    return;
  }
  const layout = el("div", "wb-layout");
  const main = el("div", "wb-main");
  const side = el("aside", "wb-side");
  layout.append(main, side);
  body.append(layout);
  if (tool.main) tool.main(wb, main);
  else main.append(fileList(wb));
  if (tool.options) {
    side.append(el("h2", "", "Options"));
    tool.options(wb, side);
  }
  wb.action = btn(
    tool.action || tool.name,
    tool.icon,
    () => runBench(wb),
    "primary big wide",
  );
  side.append(wb.action);
  wb.updateAction();
}

function fileList(wb) {
  const box = el("div", "files");
  wb.files.forEach((f, i) => {
    const row = el("div", "file");
    row.append(ic(f.file ? "pic" : "file", 20));
    const info = el("div", "file-info");
    info.append(
      el("strong", "", f.name),
      el(
        "span",
        "muted",
        (f.file ? "" : f.pages + (f.pages === 1 ? " page · " : " pages · ")) +
          fmtBytes(f.size ?? f.bytes.length),
      ),
    );
    row.append(info);
    if (wb.tool.multi) {
      row.append(
        Object.assign(
          iconBtn("up", "Move up", () => moveFile(wb, i, -1)),
          { disabled: i === 0 },
        ),
        Object.assign(
          iconBtn("down", "Move down", () => moveFile(wb, i, 1)),
          { disabled: i === wb.files.length - 1 },
        ),
      );
    }
    row.append(
      iconBtn("close", "Remove", () => {
        wb.files.splice(i, 1);
        wb.render();
      }),
    );
    box.append(row);
  });
  box.append(
    btn(
      wb.tool.multi ? "Add more files" : "Choose another file",
      "add",
      wb.chooseFiles,
      "ghost",
    ),
  );
  return box;
}
function moveFile(wb, i, d) {
  const j = i + d;
  [wb.files[i], wb.files[j]] = [wb.files[j], wb.files[i]];
  wb.render();
}

/* ---------- running and results ---------- */
async function runBench(wb) {
  if (wb.busy) return;
  wb.busy = true;
  const results = await withBusy(wb.tool.working || "Working…", (b) =>
    wb.tool.run(wb, b),
  );
  wb.busy = false;
  if (results && results.length && activeBench === wb) showResults(wb, results);
}

function showResults(wb, results) {
  const body = wb.root.querySelector(".wb-body");
  const card = el("div", "result");
  const badge = el("div", "result-badge");
  badge.append(ic("check", 28));
  card.append(
    badge,
    el(
      "h2",
      "",
      results.length === 1
        ? "Your file is ready"
        : `${results.length} files are ready`,
    ),
  );
  if (wb.summary) card.append(el("p", "muted", wb.summary));
  const list = el("div", "files");
  for (const r of results) {
    const row = el("div", "file");
    row.append(
      ic(/pdf/i.test(r.type || "application/pdf") ? "file" : "pic", 20),
    );
    const info = el("div", "file-info");
    info.append(
      el("strong", "", r.name),
      el("span", "muted", fmtBytes(r.bytes.length)),
    );
    row.append(
      info,
      btn(
        "Download",
        "download-2",
        () => saveBytes(r.bytes, r.name, r.type),
        "small",
      ),
    );
    if (!r.type || r.type === "application/pdf")
      row.append(
        btn(
          "Open",
          "folder-open",
          () => openInViewer(r.bytes, r.name),
          "small ghost",
        ),
      );
    list.append(row);
  }
  card.append(list);
  const actions = el("div", "result-actions");
  if (results.length > 1) {
    actions.append(
      btn(
        "Download all (.zip)",
        "download-2",
        () =>
          saveBytes(
            makeZip(results.map((r) => ({ name: r.name, bytes: r.bytes }))),
            baseName(results[0].name) + ".zip",
            "application/zip",
          ),
        "primary",
      ),
    );
  } else if (!results[0].type || results[0].type === "application/pdf") {
    actions.append(
      btn(
        "Download",
        "download-2",
        () => saveBytes(results[0].bytes, results[0].name),
        "primary",
      ),
    );
  }
  actions.append(
    btn(
      "Change options",
      "left",
      () => ((wb.summary = ""), wb.render()),
      "ghost",
    ),
    btn("Start over", null, () => openTool(wb.tool), "ghost"),
  );
  card.append(actions);
  body.replaceChildren(card);
}

/* Small shared form controls for tool options. */
function optionRow(label, control, hint) {
  const row = el("label", "opt");
  row.append(el("span", "opt-label", label), control);
  if (hint) row.append(el("span", "muted small", hint));
  return row;
}
function selectControl(items, value) {
  const s = el("select", "field");
  items.forEach(([v, label]) => s.append(new Option(label, v)));
  s.value = value;
  return s;
}
function numberControl(value, min, max, step = 1) {
  const i = el("input", "field");
  i.type = "number";
  Object.assign(i, { min, max, step, value });
  return i;
}
function checkControl(label, checked) {
  const row = el("label", "check");
  const i = el("input");
  i.type = "checkbox";
  i.checked = !!checked;
  row.append(i, el("span", "", label));
  row.input = i;
  return row;
}
function segControl(items, value, onChange) {
  const seg = el("div", "seg");
  const set = (v, notify = true) => {
    seg.value = v;
    seg
      .querySelectorAll("button")
      .forEach((b) => b.classList.toggle("on", b.dataset.v === v));
    if (notify) onChange?.(v);
  };
  items.forEach(([v, label]) => {
    const b = el("button", "", label);
    b.type = "button";
    b.dataset.v = v;
    b.onclick = () => set(v);
    seg.append(b);
  });
  set(value, false);
  return seg;
}
