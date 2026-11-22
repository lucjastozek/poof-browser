(async () => {
  if (document.pictureInPictureElement) return;
  const uv = window.__poofUV;
  if (!uv) return;
  const vs = [...document.querySelectorAll('video')].filter(v =>
    uv.has(v) &&
    !v.paused && !v.ended && v.readyState > 2 && v.videoWidth >= 240 &&
    !v.disablePictureInPicture &&
    !v.closest('.ad-showing, .ad-interrupting'));
  if (!vs.length) return;
  vs.sort((a, b) => b.videoWidth * b.videoHeight - a.videoWidth * a.videoHeight);
  try { await vs[0].requestPictureInPicture(); } catch (e) {}
})()
