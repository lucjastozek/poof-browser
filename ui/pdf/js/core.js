/* Shared helpers: DOM, icons, dialogs, toasts, files. Everything here is global. */
"use strict";

const $ = (id) => document.getElementById(id);
function el(tag, cls, text) {
  const n = document.createElement(tag);
  if (cls) n.className = cls;
  if (text != null) n.textContent = text;
  return n;
}
const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
/* Lets the browser paint between heavy steps. */
const frame = () => new Promise((r) => requestAnimationFrame(() => r()));

/* ---------- icons (24px grid, stroke) ---------- */
const ICONS = {
  open: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
  download: "M12 4v11m0 0l-4-4m4 4l4-4M5 20h14",
  print:
    "M7 9V4h10v5M7 17H5a1 1 0 0 1-1-1v-5a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v5a1 1 0 0 1-1 1h-2M7 14h10v6H7z",
  search: "M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zm9 16l-4-4",
  plus: "M12 5v14M5 12h14",
  minus: "M5 12h14",
  up: "M6 15l6-6 6 6",
  down: "M6 9l6 6 6-6",
  left: "M15 6l-6 6 6 6",
  right: "M9 6l6 6-6 6",
  close: "M6 6l12 12M18 6L6 18",
  sidebar: "M4 5h16v14H4zM9 5v14",
  list: "M8 6h12M8 12h12M8 18h12M4 6h.01M4 12h.01M4 18h.01",
  rotate: "M20 11a8 8 0 1 0-2.3 5.7M20 4v7h-7",
  rotateL: "M4 11a8 8 0 1 1 2.3 5.7M4 4v7h7",
  moon: "M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z",
  grid: "M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z",
  merge: "M5 5v4a4 4 0 0 0 4 4h6a4 4 0 0 0 4-4V5M12 13v7M9 17l3 3 3-3",
  split: "M12 4v7M12 11l-6 8M12 11l6 8M9 7l3-3 3 3",
  trash: "M5 7h14M10 7V4h4v3M7 7l1 13h8l1-13M10 11v6M14 11v6",
  extract: "M6 3h8l4 4v6M6 3v18h6M15 17h6m0 0l-3-3m3 3l-3 3",
  pages: "M8 3h9v14H8zM4 7v14h10",
  compress: "M8 4l4 4 4-4M8 20l4-4 4 4M5 12h14",
  wrench:
    "M14.7 6.3a4 4 0 0 0-5.4 5.4L3 18l3 3 6.3-6.3a4 4 0 0 0 5.4-5.4l-2.5 2.5-2.4-.6-.6-2.4z",
  image: "M4 5h16v14H4zM4 16l5-5 4 4 3-3 4 4M9 9.5h.01",
  text: "M6 3h8l4 4v14H6zM14 3v4h4M9 12h6M9 16h6",
  hash: "M5 9h14M5 15h14M10 4L8 20M16 4l-2 16",
  drop: "M12 3s6 6.5 6 11a6 6 0 0 1-12 0c0-4.5 6-11 6-11z",
  crop: "M6 2v14a2 2 0 0 0 2 2h14M2 6h14a2 2 0 0 1 2 2v14",
  lock: "M6 11h12v9H6zM8 11V8a4 4 0 0 1 8 0v3",
  unlock: "M6 11h12v9H6zM8 11V8a4 4 0 0 1 7.5-2",
  edit: "M4 20l4-1L19 8l-3-3L5 16zM14 7l3 3",
  sign: "M3 17c3-6 4-9 6-9s0 8 3 8 2-5 4-5 2 3 5 3M3 21h18",
  highlight: "M9 14l6-9 4 3-6 9zM9 14l-1 4 4-1M4 21h8",
  rect: "M5 6h14v12H5z",
  ellipse: "M12 5c4.4 0 8 3.1 8 7s-3.6 7-8 7-8-3.1-8-7 3.6-7 8-7z",
  line: "M5 19L19 5",
  arrow: "M5 19L19 5M10 5h9v9",
  type: "M5 6V4h14v2M12 4v16M9 20h6",
  cursor: "M5 3l14 7-6 2-2 6z",
  pen: "M3 17c3 0 4-8 7-8s1 6 4 6 3-9 7-9",
  undo: "M9 14L4 9l5-5M4 9h10a6 6 0 0 1 0 12h-3",
  redo: "M15 14l5-5-5-5M20 9H10a6 6 0 0 0 0 12h3",
  check: "M5 12l5 5 9-10",
  file: "M6 3h8l4 4v14H6zM14 3v4h4",
  fitW: "M4 12h16M7 9l-3 3 3 3M17 9l3 3-3 3",
  fitP: "M8 4H4v4M16 4h4v4M8 20H4v-4M16 20h4v-4",
  info: "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 11v5M12 8h.01",
  blank: "M6 3h8l4 4v14H6zM14 3v4h4M12 11v6M9 14h6",
  copy: "M8 8h11v12H8zM5 16V4h11",
  more: "M5 12h.01M12 12h.01M19 12h.01",
  dup: "M8 8h11v12H8zM5 16V4h11",
  addText: "M4 6V4h11v2M9.5 4v13M7 17h5M18 14v6M15 17h6",
  editText: "M3 6V4h11v2M8.5 4v11M6 15h5M14 21l.7-3 5.6-5.6 2.3 2.3L17 20.3z",
  editImage:
    "M4 5h12v5M4 5v14h8M4 16l5-5 3 3M16 21l.7-3 4.3-4.3 2.3 2.3L19 20.3z",
  swap: "M7 7h12l-3-3M17 17H5l3 3",
};
function ic(name, size = 18) {
  const s = el("span", "ic");
  s.innerHTML =
    `<svg viewBox="0 0 24 24" width="${size}" height="${size}" fill="none" stroke="currentColor" ` +
    `stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">` +
    `<path d="${ICONS[name] || ICONS.file}"/></svg>`;
  return s;
}
function btn(label, icon, onClick, cls = "") {
  const b = el("button", "btn " + cls);
  b.type = "button";
  if (icon) b.append(ic(icon, 16));
  if (label) b.append(el("span", "", label));
  if (onClick) b.onclick = onClick;
  return b;
}
function iconBtn(icon, title, onClick, cls = "") {
  const b = el("button", "ib " + cls);
  b.type = "button";
  b.title = title;
  b.setAttribute("aria-label", title);
  b.append(ic(icon, 18));
  if (onClick) b.onclick = onClick;
  return b;
}

/* ---------- theme: follows the system ---------- */
const colorQuery = matchMedia("(prefers-color-scheme: dark)");
const applyScheme = () =>
  (document.documentElement.dataset.theme = colorQuery.matches
    ? "dark"
    : "light");
colorQuery.addEventListener("change", applyScheme);
applyScheme();

/* ---------- toast, dialog, busy ---------- */
let toastTimer = 0;
function toast(msg, ms = 3200) {
  let t = $("toast");
  if (!t) {
    t = el("div", "toast");
    t.id = "toast";
    t.setAttribute("role", "status");
    $("layer").append(t);
  }
  t.textContent = msg;
  t.classList.add("on");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.classList.remove("on"), ms);
}

/* Modal dialog. Buttons: [{label, primary, run}]; `run` returns false to keep it open. */
function dialog(title, body, buttons = [{ label: "Close" }]) {
  return new Promise((resolve) => {
    const back = el("div", "scrim");
    const box = el("div", "dlg");
    box.setAttribute("role", "dialog");
    box.setAttribute("aria-label", title);
    const foot = el("div", "dlg-foot");
    const close = (v) => {
      back.remove();
      document.removeEventListener("keydown", onKey, true);
      resolve(v);
    };
    buttons.forEach((b, i) => {
      const x = btn(
        b.label,
        null,
        async () => {
          if (b.run && (await b.run(box)) === false) return;
          close(i);
        },
        b.primary ? "primary" : "",
      );
      foot.append(x);
    });
    const onKey = (e) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        close(-1);
      } else if (e.key === "Enter" && e.target.tagName !== "TEXTAREA") {
        const p = foot.querySelector(".primary");
        if (p) {
          e.preventDefault();
          p.click();
        }
      }
    };
    document.addEventListener("keydown", onKey, true);
    back.onmousedown = (e) => e.target === back && close(-1);
    box.append(el("h2", "", title), body, foot);
    back.append(box);
    $("layer").append(back);
    const first = box.querySelector("input,textarea,select");
    (first || foot.querySelector(".primary") || foot.lastChild)?.focus();
  });
}

/* Text input that never looks like a login form to the browser's password manager. */
function secretInput(placeholder) {
  const i = el("input", "field secret");
  i.type = "text";
  i.placeholder = placeholder || "";
  i.autocomplete = "off";
  i.spellcheck = false;
  return i;
}
async function askPassword(name, wrong) {
  const body = el("div", "stack");
  body.append(
    el(
      "p",
      "muted",
      wrong
        ? "That password was not accepted. Try again."
        : `“${name}” is protected.`,
    ),
  );
  const input = secretInput("Password");
  body.append(input);
  const r = await dialog("Password required", body, [
    { label: "Cancel" },
    { label: "Open", primary: true },
  ]);
  return r === 1 ? input.value : null;
}

/* Modal progress. `set(fraction, text)` updates it; `close()` removes it. */
function busy(text) {
  const back = el("div", "scrim busy");
  const box = el("div", "dlg busy-box");
  const label = el("div", "", text);
  const bar = el("div", "bar-track ind");
  const fill = el("div", "bar-fill");
  bar.append(fill);
  box.append(label, bar);
  back.append(box);
  $("layer").append(back);
  return {
    set(f, t) {
      bar.classList.remove("ind");
      if (t) label.textContent = t;
      fill.style.width = clamp(f, 0, 1) * 100 + "%";
    },
    close: () => back.remove(),
  };
}
/* Runs `fn(progress)` with a progress dialog and reports failures instead of throwing. */
async function withBusy(text, fn) {
  const b = busy(text);
  await frame();
  try {
    return await fn(b);
  } catch (e) {
    console.error(e);
    toast(friendlyError(e), 6000);
    return null;
  } finally {
    b.close();
  }
}
/* An error whose message is meant for the user as written. */
class UserError extends Error {}
function friendlyError(e) {
  if (e instanceof UserError) return e.message;
  const m = String((e && e.message) || e);
  if (m === "cancelled") return "Cancelled.";
  if (/password/i.test(m))
    return "This file is password protected or the password is wrong.";
  return "Something went wrong: " + m.slice(0, 160);
}

/* ---------- files ---------- */
const fmtBytes = (n) =>
  n < 1024
    ? n + " B"
    : n < 1048576
      ? (n / 1024).toFixed(0) + " KB"
      : (n / 1048576).toFixed(1) + " MB";
const baseName = (name) => name.replace(/\.[^.]+$/, "");
const isPdfFile = (f) => /pdf$/i.test(f.type) || /\.pdf$/i.test(f.name);
const isImageFile = (f) =>
  /^image\//.test(f.type) || /\.(jpe?g|png|webp|gif|bmp|avif)$/i.test(f.name);

function pickFiles(accept, multiple) {
  return new Promise((resolve) => {
    const i = document.createElement("input");
    i.type = "file";
    i.accept = accept;
    i.multiple = !!multiple;
    i.onchange = () => resolve([...i.files]);
    i.oncancel = () => resolve([]);
    i.click();
  });
}
async function readBytes(file) {
  return new Uint8Array(await file.arrayBuffer());
}
/* Saves through the browser's download list. */
function saveBytes(bytes, name, type = "application/pdf") {
  const url = URL.createObjectURL(
    bytes instanceof Blob ? bytes : new Blob([bytes], { type }),
  );
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 60000);
}
const canvasBlob = (canvas, type, q) =>
  new Promise((r) => canvas.toBlob(r, type, q));
