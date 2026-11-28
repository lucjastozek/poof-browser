// Finishes a page capture: draws the device frame, copies the picture and/or hands the framed
// picture back to native code to save. Native code calls `__captureFinish` (with a user gesture
// when it must write the clipboard).

/* Rounded-rectangle path that works without ctx.roundRect. */
function capturePath(ctx, x, y, w, h, r) {
  r = Math.min(r, w / 2, h / 2);
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + w, y, x + w, y + h, r);
  ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r);
  ctx.arcTo(x, y, x + w, y, r);
  ctx.closePath();
}

/* A phone body around the picture; a pill cut-out is added to portrait shots. */
function framePhone(img) {
  const bz = Math.max(10, Math.round(Math.min(img.width, img.height) * 0.04));
  const w = img.width + 2 * bz,
    h = img.height + 2 * bz;
  const cv = Object.assign(document.createElement("canvas"), {
    width: w,
    height: h,
  });
  const ctx = cv.getContext("2d");
  capturePath(ctx, 0, 0, w, h, bz * 2.6);
  ctx.fillStyle = "#17171a";
  ctx.fill();
  ctx.lineWidth = Math.max(1, bz * 0.12);
  ctx.strokeStyle = "#3a3a40";
  ctx.stroke();
  ctx.save();
  capturePath(ctx, bz, bz, img.width, img.height, bz * 1.7);
  ctx.clip();
  ctx.drawImage(img, bz, bz);
  ctx.restore();
  if (img.height > img.width) {
    const pw = Math.round(img.width * 0.24),
      ph = Math.round(bz * 0.95);
    capturePath(ctx, (w - pw) / 2, bz * 1.5, pw, ph, ph / 2);
    ctx.fillStyle = "#050505";
    ctx.fill();
  }
  return cv;
}

/* A desktop window: title bar with traffic lights and the site name, soft shadow around. */
function frameBrowser(img, host) {
  const k = Math.min(3, Math.max(1, img.width / 1100));
  const bar = Math.round(46 * k),
    m = Math.round(36 * k),
    r = 11 * k;
  const w = img.width + 2 * m,
    h = img.height + bar + 2 * m;
  const cv = Object.assign(document.createElement("canvas"), {
    width: w,
    height: h,
  });
  const ctx = cv.getContext("2d");
  ctx.save();
  ctx.shadowColor = "rgba(0,0,0,.35)";
  ctx.shadowBlur = 28 * k;
  ctx.shadowOffsetY = 10 * k;
  capturePath(ctx, m, m, img.width, img.height + bar, r);
  ctx.fillStyle = "#2b2b30";
  ctx.fill();
  ctx.restore();
  ctx.save();
  capturePath(ctx, m, m, img.width, img.height + bar, r);
  ctx.clip();
  ctx.fillStyle = "#2b2b30";
  ctx.fillRect(m, m, img.width, bar);
  ctx.drawImage(img, m, m + bar);
  ctx.restore();
  ["#ff5f57", "#febc2e", "#28c840"].forEach((c, i) => {
    ctx.beginPath();
    ctx.arc(m + (20 + i * 22) * k, m + bar / 2, 6 * k, 0, Math.PI * 2);
    ctx.fillStyle = c;
    ctx.fill();
  });
  const pw = Math.min(img.width * 0.5, 420 * k),
    ph = 26 * k;
  capturePath(
    ctx,
    m + (img.width - pw) / 2,
    m + (bar - ph) / 2,
    pw,
    ph,
    ph / 2,
  );
  ctx.fillStyle = "#1c1c20";
  ctx.fill();
  ctx.fillStyle = "#b9b9c0";
  ctx.font = `${Math.round(13 * k)}px system-ui, "Segoe UI", sans-serif`;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillText(host || "", m + img.width / 2, m + bar / 2 + k, pw - 24 * k);
  return cv;
}

/* The picture to deliver: framed when asked, the original when framing fails. */
async function captureBlob(b64, o) {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  const raw = new Blob([bytes], { type: "image/png" });
  if (!o.frame || o.frame === "none") return raw;
  try {
    const img = await createImageBitmap(raw);
    const cv =
      o.frame === "phone" ? framePhone(img) : frameBrowser(img, o.host);
    return await new Promise((ok, no) =>
      cv.toBlob((b) => (b ? ok(b) : no(new Error("empty"))), "image/png"),
    );
  } catch (_) {
    return raw;
  }
}

const captureBase64 = (blob) =>
  new Promise((ok, no) => {
    const r = new FileReader();
    r.onload = () => ok(String(r.result).split(",")[1] || "");
    r.onerror = () => no(r.error);
    r.readAsDataURL(blob);
  });

window.__captureFinish = async (b64, o) => {
  // The clipboard call comes first and gets the blob as a promise, so it keeps the gesture.
  const blob = captureBlob(b64, o);
  let copied = false;
  if (o.copy) {
    try {
      await navigator.clipboard.write([
        new ClipboardItem({ "image/png": blob }),
      ]);
      copied = true;
    } catch (_) {}
  }
  try {
    if (o.save) {
      send({
        cmd: "capture_save",
        png: await captureBase64(await blob),
        copied,
      });
    } else if (o.copy) {
      window.__toast(
        copied
          ? o.saved
            ? "Screenshot saved and copied"
            : "Screenshot copied"
          : "Couldn’t copy the screenshot",
      );
    }
  } catch (e) {
    window.__toast(
      "Couldn’t finish the screenshot: " + (e && e.message ? e.message : e),
    );
  }
  if (o.copy && o.tab) send({ cmd: "focus_tab", id: o.tab });
};
