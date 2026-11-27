(function () {
  if (!/^https?:$/.test(location.protocol)) return;
  var cur = null;
  var last = "";
  var timer = 0;
  function post(o) {
    var s = JSON.stringify(o);
    if (s === last) return;
    last = s;
    try {
      window.ipc.postMessage(s);
    } catch (e) {}
  }
  function usable(el) {
    return (
      el &&
      el.isConnected &&
      !el.ended &&
      !el.muted &&
      el.volume > 0 &&
      !(isFinite(el.duration) && el.duration < 3)
    );
  }
  function report() {
    if (!usable(cur) || (cur.paused && !(cur.currentTime > 0))) {
      cur = null;
      clearInterval(timer);
      timer = 0;
      post({ t: "media", on: false });
      return;
    }
    var m = (navigator.mediaSession && navigator.mediaSession.metadata) || {};
    post({
      t: "media",
      on: true,
      playing: !cur.paused,
      title: String(m.title || document.title || "").slice(0, 200),
      artist: String(m.artist || "").slice(0, 200),
    });
  }
  function onEvent(e) {
    var el = e.target;
    if (!(el instanceof HTMLMediaElement)) return;
    if (e.type === "play" || e.type === "playing") {
      if (!usable(el)) return;
      cur = el;
      if (!timer) timer = setInterval(report, 2000);
    } else if (el !== cur) return;
    report();
  }
  ["play", "playing", "pause", "ended", "emptied", "volumechange"].forEach(
    function (n) {
      document.addEventListener(n, onEvent, { capture: true, passive: true });
    },
  );
  Object.defineProperty(window, "__poofMediaToggle", {
    value: function () {
      if (!cur) return;
      if (cur.paused) cur.play().catch(function () {});
      else cur.pause();
    },
    enumerable: false,
  });
  // A new page starts without media; tell the host so a stale player disappears.
  post({ t: "media", on: false });
})();
