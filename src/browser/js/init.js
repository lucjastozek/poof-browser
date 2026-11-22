(function () {
  // Leave anti-bot and auth iframes (Cloudflare Turnstile, Google sign-in embeds)
  // completely untouched: no injected styles, no listeners. Top-level pages on these
  // hosts still get the script so the sidebar keeps receiving URL updates.
  if (
    window.top !== window &&
    /^(challenges\.cloudflare\.com|accounts\.google\.com|www\.google\.com)$/.test(
      location.hostname,
    )
  ) {
    return;
  }
  function post(o) {
    try {
      window.ipc.postMessage(JSON.stringify(o));
    } catch (e) {}
  }
  // Fullscreen in a cross-origin iframe is isolated from the top document.
  // Relay its state up the frame tree with postMessage; only the top document
  // talks to native code. This is the signal that makes the APP window itself
  // enter OS-level borderless fullscreen, rather than merely filling the WebView.
  var fullscreenRelay = "__poof_native_fullscreen__";
  document.addEventListener(
    "fullscreenchange",
    function () {
      var active = !!document.fullscreenElement;
      if (window.top === window) {
        post({ t: "fullscreen", active: active });
      } else {
        try {
          window.parent.postMessage(
            { type: fullscreenRelay, active: active },
            "*",
          );
        } catch (e) {}
      }
    },
    true,
  );
  if (window.top !== window) {
    addEventListener(
      "message",
      function (e) {
        if (!e.data || e.data.type !== fullscreenRelay) return;
        if (window.parent !== window) {
          try {
            window.parent.postMessage(e.data, "*");
          } catch (err) {}
        } else {
          post({ t: "fullscreen", active: !!e.data.active });
        }
      },
      false,
    );
    return;
  }
  // A player may forward a shortcut/fullscreen request from a nested frame.
  addEventListener(
    "message",
    function (e) {
      if (e.data && e.data.type === fullscreenRelay) {
        post({ t: "fullscreen", active: !!e.data.active });
      }
    },
    false,
  );
  var scrollbarStyle = document.createElement("style");
  scrollbarStyle.textContent =
    "html{scrollbar-width:thin!important;scrollbar-color:color-mix(in srgb,currentColor 24%,transparent) transparent!important}::-webkit-scrollbar{width:6px!important;height:6px!important}::-webkit-scrollbar-track{background:transparent!important}::-webkit-scrollbar-thumb{background:color-mix(in srgb,currentColor 28%,transparent)!important;border-radius:99px!important;border:2px solid transparent!important;background-clip:padding-box!important}::-webkit-scrollbar-thumb:hover{background:color-mix(in srgb,currentColor 42%,transparent)!important;background-clip:padding-box!important}::-webkit-scrollbar-button{display:none!important;width:0!important;height:0!important}";
  (document.head || document.documentElement).appendChild(scrollbarStyle);

  // URL reporting. Event-driven instead of a permanent 1.5s timer, so idle and background
  // tabs run no JavaScript at all: SPA navigations are caught by wrapping the History API
  // and listening for popstate/hashchange/Navigation API events. A slow safety-net poll runs
  // only while the page is visible (it catches anything the hooks miss).
  var last = "";
  function meta(force) {
    var u = location.href;
    if (!force && u === last) return;
    last = u;
    post({ t: "meta", u: u });
  }
  var queued = false;
  function queueMeta() {
    if (queued) return;
    queued = true;
    setTimeout(function () {
      queued = false;
      meta(false);
    }, 0);
  }
  ["pushState", "replaceState"].forEach(function (name) {
    try {
      var original = history[name];
      if (typeof original !== "function") return;
      history[name] = function () {
        var result = original.apply(this, arguments);
        queueMeta();
        return result;
      };
    } catch (e) {}
  });
  addEventListener("popstate", queueMeta);
  addEventListener("hashchange", queueMeta);
  addEventListener("pageshow", function () {
    meta(true);
  });
  try {
    if (window.navigation && navigation.addEventListener) {
      navigation.addEventListener("navigatesuccess", queueMeta);
      navigation.addEventListener("currententrychange", queueMeta);
    }
  } catch (e) {}
  addEventListener("DOMContentLoaded", function () {
    meta(true);
  });
  addEventListener("load", function () {
    meta(true);
  });

  var poll = 0;
  function updatePoll() {
    if (document.visibilityState === "visible") {
      if (!poll) {
        meta(false);
        poll = setInterval(function () {
          meta(false);
        }, 5000);
      }
    } else if (poll) {
      clearInterval(poll);
      poll = 0;
    }
  }
  document.addEventListener("visibilitychange", updatePoll);
  updatePoll();
  // Browser accelerators are handled natively by WebView2; the F shortcut lives in fullscreen.js.
})();
