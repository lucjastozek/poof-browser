/* Signature dialog: draw it, type it or use an image. The result is placed like any image. */
"use strict";

const SIGN_FONTS = [
  '"Segoe Script", "Brush Script MT", cursive',
  '"Lucida Handwriting", "Segoe Print", cursive',
  '"Ink Free", "Comic Sans MS", cursive',
];
const SIGN_W = 520;
const SIGN_H = 180;
const SIGN_INK = ["#111111", "#1c3faa", "#b3261e"];

/* Crops transparent margins so the placed signature hugs its strokes. */
function trimCanvas(canvas) {
  const { width: w, height: h } = canvas;
  const data = canvas.getContext("2d").getImageData(0, 0, w, h).data;
  let x0 = w,
    y0 = h,
    x1 = -1,
    y1 = -1;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (data[(y * w + x) * 4 + 3] > 8) {
        x0 = Math.min(x0, x);
        x1 = Math.max(x1, x);
        y0 = Math.min(y0, y);
        y1 = Math.max(y1, y);
      }
    }
  }
  if (x1 < 0) return null;
  const pad = 6;
  const out = document.createElement("canvas");
  out.width = x1 - x0 + 1 + pad * 2;
  out.height = y1 - y0 + 1 + pad * 2;
  out
    .getContext("2d")
    .drawImage(
      canvas,
      x0,
      y0,
      x1 - x0 + 1,
      y1 - y0 + 1,
      pad,
      pad,
      x1 - x0 + 1,
      y1 - y0 + 1,
    );
  return out;
}

function signaturePad(getInk) {
  const canvas = el("canvas", "sign-pad");
  canvas.width = SIGN_W * 2;
  canvas.height = SIGN_H * 2;
  const ctx = canvas.getContext("2d");
  ctx.lineWidth = 5;
  ctx.lineCap = ctx.lineJoin = "round";
  let last = null;
  const pos = (e) => {
    const r = canvas.getBoundingClientRect();
    return [
      ((e.clientX - r.left) / r.width) * canvas.width,
      ((e.clientY - r.top) / r.height) * canvas.height,
    ];
  };
  canvas.onpointerdown = (e) => {
    canvas.setPointerCapture(e.pointerId);
    last = pos(e);
    ctx.strokeStyle = ctx.fillStyle = getInk();
    ctx.beginPath();
    ctx.arc(last[0], last[1], 2.5, 0, 7);
    ctx.fill();
  };
  canvas.onpointermove = (e) => {
    if (!last) return;
    const p = pos(e);
    ctx.beginPath();
    ctx.moveTo(...last);
    ctx.lineTo(...p);
    ctx.stroke();
    last = p;
  };
  canvas.onpointerup = canvas.onpointercancel = () => (last = null);
  canvas.clear = () => ctx.clearRect(0, 0, canvas.width, canvas.height);
  ctx.fillStyle = getInk();
  return canvas;
}

async function openSignDialog() {
  let mode = "draw";
  let ink = SIGN_INK[0];
  let imageCanvas = null;
  const body = el("div", "stack sign");
  const tabs = el("div", "seg");
  const pane = el("div", "sign-pane");
  const inkRow = el("div", "swatches");
  const pad = signaturePad(() => ink);
  const typed = el("input", "field");
  typed.placeholder = "Type your name";
  const typedPreview = el("div", "sign-typed");
  let fontIndex = 0;

  const renderTyped = () => {
    typedPreview.textContent = typed.value || "Your name";
    typedPreview.style.fontFamily = SIGN_FONTS[fontIndex];
    typedPreview.style.color = ink;
  };
  typed.oninput = renderTyped;
  typedPreview.onclick = () => {
    fontIndex = (fontIndex + 1) % SIGN_FONTS.length;
    renderTyped();
  };
  typedPreview.title = "Click to change the style";

  const show = (m) => {
    mode = m;
    tabs
      .querySelectorAll("button")
      .forEach((b) => b.classList.toggle("on", b.dataset.m === m));
    pane.replaceChildren();
    if (m === "draw")
      pane.append(
        pad,
        btn("Clear", null, () => pad.clear(), "small"),
      );
    else if (m === "type") renderTyped(), pane.append(typed, typedPreview);
    else {
      const choose = btn("Choose image…", "pic", async () => {
        const [f] = await pickFiles("image/*", false);
        if (!f) return;
        const bmp = await createImageBitmap(f);
        const k = Math.min(1, 900 / Math.max(bmp.width, bmp.height));
        imageCanvas = document.createElement("canvas");
        imageCanvas.width = Math.round(bmp.width * k);
        imageCanvas.height = Math.round(bmp.height * k);
        imageCanvas
          .getContext("2d")
          .drawImage(bmp, 0, 0, imageCanvas.width, imageCanvas.height);
        imageCanvas.className = "sign-img";
        pane.replaceChildren(imageCanvas, choose);
      });
      pane.append(
        el(
          "p",
          "muted",
          "Use a photo or scan of your signature on a plain background.",
        ),
        choose,
      );
    }
  };
  [
    ["draw", "Draw"],
    ["type", "Type"],
    ["image", "Image"],
  ].forEach(([m, label]) => {
    const b = el("button", "", label);
    b.type = "button";
    b.dataset.m = m;
    b.onclick = () => show(m);
    tabs.append(b);
  });
  SIGN_INK.forEach((c) => {
    const s = el("button", "swatch");
    s.type = "button";
    s.style.background = c;
    s.setAttribute("aria-label", "Ink " + c);
    s.onclick = () => ((ink = c), renderTyped());
    inkRow.append(s);
  });
  body.append(tabs, pane, inkRow);
  show("draw");

  const choice = await dialog("Add signature", body, [
    { label: "Cancel" },
    { label: "Place on page", primary: true },
  ]);
  if (choice !== 1) return;
  let canvas = null;
  if (mode === "draw") canvas = pad;
  else if (mode === "image") canvas = imageCanvas;
  else if (typed.value.trim()) {
    canvas = document.createElement("canvas");
    canvas.width = 1400;
    canvas.height = 360;
    const ctx = canvas.getContext("2d");
    ctx.font = `200px ${SIGN_FONTS[fontIndex]}`;
    ctx.textBaseline = "middle";
    ctx.fillStyle = ink;
    ctx.fillText(typed.value.trim(), 20, 180, 1360);
  }
  const trimmed = canvas && (mode === "image" ? canvas : trimCanvas(canvas));
  if (!trimmed) return toast("Nothing to place yet.");
  placeImage(trimmed.toDataURL("image/png"));
}
