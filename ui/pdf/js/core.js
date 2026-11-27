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

/* ---------- icons: MingCute; `ICONS` is generated into icons.js by tools/icons.rs ---------- */
function ic(name, size = 18) {
  const s = el("span", "ic");
  s.innerHTML =
    `<svg viewBox="0 0 24 24" width="${size}" height="${size}" fill="currentColor" aria-hidden="true">` +
    `${ICONS[name] || ICONS.file}</svg>`;
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
