(async () => {
  if (document.pictureInPictureElement) return;
  const uv = window.__poofUV;
  if (!uv) return;
  const live = (v) =>
    typeof MediaStream !== "undefined" && v.srcObject instanceof MediaStream;
  const vs = [...document.querySelectorAll("video")].filter(
    (v) =>
      uv.has(v) &&
      !v.paused &&
      !v.ended &&
      v.readyState > 1 &&
      (live(v) ? v.videoWidth > 0 : v.videoWidth >= 240) &&
      !v.closest(".ad-showing, .ad-interrupting"),
  );
  if (!vs.length) return;
  vs.sort(
    (a, b) => b.videoWidth * b.videoHeight - a.videoWidth * a.videoHeight,
  );
  const v = vs[0];
  // Meet turns PiP off on its tiles; the user still wants the call popped out.
  if (v.disablePictureInPicture) v.disablePictureInPicture = false;
  try {
    await v.requestPictureInPicture();
  } catch (e) {}
})();
