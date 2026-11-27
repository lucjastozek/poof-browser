// Tells the browser when this page is doing live work that must survive in the background
// (camera/microphone/screen capture, WebRTC calls, open WebSockets: Meet, Teams, Zoom, Discord...). Event-driven,
// no timers; the native side then never puts the tab to sleep or trims its memory.
(() => {
  try {
    if (window.__poofLive) return;
    window.__poofLive = 1;
    const tracks = new Set();
    const peers = new Set();
    const sockets = new Set();
    let sent = false;
    const alive = () => {
      for (const t of tracks) if (t.readyState === "ended") tracks.delete(t);
      for (const p of peers) {
        if (p.connectionState === "closed" || p.connectionState === "failed")
          peers.delete(p);
      }
      for (const w of sockets) if (w.readyState > 1) sockets.delete(w);
      return tracks.size > 0 || peers.size > 0 || sockets.size > 0;
    };
    const report = () => {
      try {
        const on = alive();
        if (on === sent) return;
        sent = on;
        window.ipc.postMessage(JSON.stringify({ t: "live", on }));
      } catch (_) {}
    };
    const watchStream = (stream) => {
      try {
        for (const t of stream.getTracks()) {
          tracks.add(t);
          t.addEventListener("ended", report);
        }
        report();
      } catch (_) {}
      return stream;
    };
    const md = navigator.mediaDevices;
    if (md) {
      for (const name of ["getUserMedia", "getDisplayMedia"]) {
        const original = md[name];
        if (typeof original !== "function") continue;
        md[name] = function (...args) {
          return original.apply(this, args).then(watchStream);
        };
      }
    }
    const stop = MediaStreamTrack.prototype.stop;
    MediaStreamTrack.prototype.stop = function () {
      stop.call(this);
      setTimeout(report, 0);
    };
    const NativeSocket = window.WebSocket;
    if (NativeSocket) {
      window.WebSocket = new Proxy(NativeSocket, {
        construct(target, args, newTarget) {
          const ws = Reflect.construct(target, args, newTarget);
          ws.addEventListener("open", () => {
            sockets.add(ws);
            report();
          });
          ws.addEventListener("close", () => setTimeout(report, 0));
          return ws;
        },
      });
    }
    const Native = window.RTCPeerConnection;
    if (Native) {
      const close = Native.prototype.close;
      Native.prototype.close = function () {
        close.call(this);
        setTimeout(report, 0);
      };
      window.RTCPeerConnection = new Proxy(Native, {
        construct(target, args, newTarget) {
          const pc = Reflect.construct(target, args, newTarget);
          pc.addEventListener("connectionstatechange", () => {
            if (pc.connectionState === "connected") peers.add(pc);
            report();
          });
          return pc;
        },
      });
    }
  } catch (_) {}
})();
