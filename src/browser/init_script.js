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
  // Keep WebView2's native, full-width scrollbars out of the page UI. This runs
  // at document start in every frame; reattach if a site replaces its document root
  // or removes injected styles during client-side navigation.
  var scrollbarStyleText =
    ":root{--poof-space-color:__POOF_SPACE_COLOR__}" +
    "html,body,*{scrollbar-width:thin!important;scrollbar-color:color-mix(in srgb,var(--poof-space-color) 22%,currentColor) transparent!important}" +
    "html::-webkit-scrollbar,body::-webkit-scrollbar,*::-webkit-scrollbar{width:6px!important;height:6px!important;background:transparent!important}" +
    "html::-webkit-scrollbar-track,body::-webkit-scrollbar-track,*::-webkit-scrollbar-track," +
    "html::-webkit-scrollbar-corner,body::-webkit-scrollbar-corner,*::-webkit-scrollbar-corner{background:transparent!important}" +
    "html::-webkit-scrollbar-thumb,body::-webkit-scrollbar-thumb,*::-webkit-scrollbar-thumb{" +
    "background-color:color-mix(in srgb,color-mix(in srgb,var(--poof-space-color) 22%,currentColor) 48%,transparent)!important;" +
    "border:2px solid transparent!important;background-clip:content-box!important;border-radius:999px!important}" +
    "html::-webkit-scrollbar-thumb:hover,body::-webkit-scrollbar-thumb:hover,*::-webkit-scrollbar-thumb:hover{" +
    "background-color:color-mix(in srgb,color-mix(in srgb,var(--poof-space-color) 32%,currentColor) 66%,transparent)!important}" +
    "html::-webkit-scrollbar-button,body::-webkit-scrollbar-button,*::-webkit-scrollbar-button," +
    "html::-webkit-scrollbar-button:single-button,body::-webkit-scrollbar-button:single-button,*::-webkit-scrollbar-button:single-button," +
    "html::-webkit-scrollbar-button:vertical:decrement,html::-webkit-scrollbar-button:vertical:increment," +
    "html::-webkit-scrollbar-button:horizontal:decrement,html::-webkit-scrollbar-button:horizontal:increment," +
    "body::-webkit-scrollbar-button:vertical:decrement,body::-webkit-scrollbar-button:vertical:increment," +
    "body::-webkit-scrollbar-button:horizontal:decrement,body::-webkit-scrollbar-button:horizontal:increment," +
    "*::-webkit-scrollbar-button:vertical:decrement,*::-webkit-scrollbar-button:vertical:increment," +
    "*::-webkit-scrollbar-button:horizontal:decrement,*::-webkit-scrollbar-button:horizontal:increment{" +
    "display:none!important;width:0!important;height:0!important;min-width:0!important;min-height:0!important}";
  var scrollbarStyle = null;
  function installScrollbarStyle() {
    if (!document.documentElement) return;
    if (scrollbarStyle && scrollbarStyle.isConnected) return;
    scrollbarStyle = document.createElement("style");
    scrollbarStyle.id = "__poof_minimal_scrollbars";
    scrollbarStyle.textContent = scrollbarStyleText;
    (document.head || document.documentElement).appendChild(scrollbarStyle);
  }
  installScrollbarStyle();
  new MutationObserver(installScrollbarStyle).observe(document, {
    childList: true,
    subtree: true
  });
  // Sites with their own full player UI (YouTube) handle fullscreen natively.
  // Our video-only workaround would fight their layout and show a second set of controls.
  var nativePlayer = /(^|\.)(youtube\.com|youtube-nocookie\.com)$/.test(
    location.hostname,
  );
  // Only the top document controls the native window. Fullscreen changes from
  // embedded frames propagate to their iframe element in the parent document.
  // Keep the video player's F shortcut available even before its controls are clicked.
  // This runs in every frame because embedded players commonly own their video element.
  function editableTarget(el) {
    return !!(
      el &&
      (el.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName))
    );
  }
  function requestPlayerFullscreen() {
    if (document.fullscreenElement) {
      document.exitFullscreen().catch(function () {});
      return true;
    }
    var videos = Array.prototype.slice
      .call(document.querySelectorAll("video"))
      .filter(function (v) {
        return v.readyState > 0 && v.videoWidth > 0;
      });
    if (videos.length) {
      videos.sort(function (a, b) {
        return b.videoWidth * b.videoHeight - a.videoWidth * a.videoHeight;
      });
      var v = videos[0];
      // Fullscreen the video surface itself, not the site's larger player
      // container. Fullscreening the container exposes unrelated page/player
      // layers behind or around the video on several sites.
      v.__poofHadControls = v.controls;
      v.controls = true;
      if (v.requestFullscreen) {
        v.requestFullscreen().catch(function () {
          v.controls = !!v.__poofHadControls;
          delete v.__poofHadControls;
        });
        return true;
      }
    }
    return false;
  }
  addEventListener(
    "keydown",
    function (e) {
      if (
        nativePlayer ||
        !e.isTrusted ||
        e.defaultPrevented ||
        e.repeat ||
        e.ctrlKey ||
        e.altKey ||
        e.metaKey ||
        e.shiftKey ||
        e.key.toLowerCase() !== "f" ||
        editableTarget(e.target)
      )
        return;
      if (requestPlayerFullscreen()) {
        e.preventDefault();
        e.stopPropagation();
        return;
      }
      // Do not fullscreen the iframe from the top document: that exposes
      // the site's outer player layers. Cross-origin video frames handle F
      // themselves and fullscreen the video element directly.
    },
    true,
  );
  // The video element itself is the fullscreen surface. Its viewport-sized
  // box is black, while object-fit:contain keeps the complete video centered
  // without stretching or cropping. The fullscreen backdrop is also black.
  // Inject this in every frame because third-party players are often cross-origin.
  var fullscreenStyle = document.createElement("style");
  fullscreenStyle.textContent =
    ":fullscreen{background:#000!important}" +
    ":fullscreen::backdrop{background:#000!important}" +
    "video:fullscreen{" +
    "display:block!important;position:fixed!important;inset:0!important;margin:0!important;" +
    "width:100vw!important;height:100vh!important;max-width:none!important;max-height:none!important;" +
    "object-fit:contain!important;background:#000!important;border:0!important;transform:none!important}" +
    ".poof-video-only-fullscreen{background:#000!important;overflow:hidden!important}" +
    ".poof-video-only-fullscreen *{visibility:hidden!important}" +
    ".poof-video-only-fullscreen video.poof-fullscreen-video{" +
    "visibility:visible!important;display:block!important;position:fixed!important;inset:0!important;margin:0!important;" +
    "width:100vw!important;height:100vh!important;max-width:none!important;max-height:none!important;" +
    "object-fit:contain!important;background:#000!important;border:0!important;transform:none!important}";
  if (!nativePlayer) {
    (document.head || document.documentElement).appendChild(fullscreenStyle);
  }
  function clearVideoOnlyFullscreen() {
    document
      .querySelectorAll(".poof-video-only-fullscreen")
      .forEach(function (el) {
        el.classList.remove("poof-video-only-fullscreen");
      });
    document
      .querySelectorAll("video.poof-fullscreen-video")
      .forEach(function (v) {
        v.classList.remove("poof-fullscreen-video");
      });
  }
  document.addEventListener(
    "fullscreenchange",
    function () {
      if (nativePlayer) return;
      clearVideoOnlyFullscreen();
      var active = document.fullscreenElement;
      if (active && active.tagName !== "VIDEO") {
        // Some fullscreen paths request the whole player container.
        // Keep that native fullscreen request, but make its visible surface
        // strictly the video. Player chrome, overlays and colored page layers
        // are hidden; the video itself fills the viewport without cropping.
        var videos = Array.prototype.slice
          .call(active.querySelectorAll("video"))
          .filter(function (v) {
            return v.videoWidth > 0 && v.videoHeight > 0;
          });
        videos.sort(function (a, b) {
          return b.videoWidth * b.videoHeight - a.videoWidth * a.videoHeight;
        });
        if (videos.length) {
          active.classList.add("poof-video-only-fullscreen");
          videos[0].classList.add("poof-fullscreen-video");
        }
      }
      if (active) return;
      document.querySelectorAll("video").forEach(function (v) {
        if (v.__poofHadControls !== undefined) {
          v.controls = !!v.__poofHadControls;
          delete v.__poofHadControls;
        }
      });
    },
    true,
  );
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
  // Browser accelerators are handled natively by WebView2. Keep page-level
  // key handling limited to the video fullscreen shortcut above.
})();
