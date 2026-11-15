(function () {
  function post(o) {
    try {
      window.ipc.postMessage(JSON.stringify(o));
    } catch (e) {}
  }
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
  (document.head || document.documentElement).appendChild(fullscreenStyle);
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
      clearVideoOnlyFullscreen();
      var active = document.fullscreenElement;
      if (active && active.tagName !== "VIDEO") {
        // Some YouTube fullscreen paths request the whole player container.
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
  var scrollbarStyle = document.createElement("style");
  scrollbarStyle.textContent =
    "html{scrollbar-width:thin!important;scrollbar-color:color-mix(in srgb,currentColor 24%,transparent) transparent!important}::-webkit-scrollbar{width:6px!important;height:6px!important}::-webkit-scrollbar-track{background:transparent!important}::-webkit-scrollbar-thumb{background:color-mix(in srgb,currentColor 28%,transparent)!important;border-radius:99px!important;border:2px solid transparent!important;background-clip:padding-box!important}::-webkit-scrollbar-thumb:hover{background:color-mix(in srgb,currentColor 42%,transparent)!important;background-clip:padding-box!important}::-webkit-scrollbar-button{display:none!important;width:0!important;height:0!important}";
  (document.head || document.documentElement).appendChild(scrollbarStyle);
  var last = "";
  function icon() {
    try {
      var pageUrl = new URL(location.href);

      // Authentication gateways often replace the app URL with a login URL.
      // Recover the intended app from standard return/continue parameters so
      // its favicon does not become the identity provider's generic logo.
      function appUrlFromRedirect(url) {
        var keys = [
          "continue", "redirect_uri", "redirect", "return", "return_to",
          "returnurl", "returnUrl", "service_url", "target"
        ];
        for (var i = 0; i < keys.length; i++) {
          var value = url.searchParams.get(keys[i]);
          if (!value) continue;
          try {
            var candidate = new URL(value, url.href);
            if (
              (candidate.protocol === "https:" || candidate.protocol === "http:") &&
              candidate.hostname.toLowerCase() !== url.hostname.toLowerCase()
            ) return candidate;
          } catch (e) {}
        }
        return null;
      }

      var redirectAppUrl = appUrlFromRedirect(pageUrl);
      var appUrl = redirectAppUrl || pageUrl;
      var appHost = appUrl.hostname.toLowerCase();
      var links = Array.prototype.slice
        .call(document.querySelectorAll('link[rel~="icon" i]'))
        .map(function (link) {
          var href = link.getAttribute("href");
          if (!href) return null;
          try {
            var url = new URL(href, pageUrl.href);
            if (url.protocol !== "http:" && url.protocol !== "https:") return null;
            return {
              url: url,
              sizes: link.getAttribute("sizes") || "",
              type: link.getAttribute("type") || "",
            };
          } catch (e) {
            return null;
          }
        })
        .filter(Boolean);

      // A declared icon is relevant only when it belongs to the intended app
      // host (or its CDN). Do not accept the login page's generic favicon.
      links.sort(function (a, b) {
        var aExactHost = a.url.hostname.toLowerCase() === appHost ? 1 : 0;
        var bExactHost = b.url.hostname.toLowerCase() === appHost ? 1 : 0;
        if (aExactHost !== bExactHost) return bExactHost - aExactHost;

        var aSameOrigin = a.url.origin === appUrl.origin ? 1 : 0;
        var bSameOrigin = b.url.origin === appUrl.origin ? 1 : 0;
        if (aSameOrigin !== bSameOrigin) return bSameOrigin - aSameOrigin;

        var aSized = a.sizes && a.sizes.toLowerCase() !== "any" ? 1 : 0;
        var bSized = b.sizes && b.sizes.toLowerCase() !== "any" ? 1 : 0;
        if (aSized !== bSized) return bSized - aSized;

        var aVector = /svg|png|webp/i.test(a.type + " " + a.url.pathname) ? 1 : 0;
        var bVector = /svg|png|webp/i.test(b.type + " " + b.url.pathname) ? 1 : 0;
        return bVector - aVector;
      });

      var selected = links.length ? links[0].url : null;
      var selectedHost = selected && selected.hostname.toLowerCase();
      var selectedBelongsToApp =
        selectedHost &&
        (selectedHost === appHost || selectedHost.endsWith("." + appHost));

      if (selected && (!redirectAppUrl || selectedBelongsToApp)) return selected.href;

      // Some services expose their product icon through a canonical favicon
      // host rather than the web-app hostname. Gmail is one such case.
      if (appHost === "mail.google.com") {
        return "https://gmail.google.com/favicon.ico";
      }

      // Generic fallback for any app subdomain, including pages redirected
      // to an identity provider. The browser follows favicon endpoint redirects.
      return new URL("/favicon.ico", appUrl.origin).href;
    } catch (e) {
      return null;
    }
  }
  function meta(force) {
    var u = location.href,
      f = icon(),
      k = u + "|" + f;
    if (!force && k === last) return;
    last = k;
    post({ t: "meta", u: u, f: f });
  }
  addEventListener("DOMContentLoaded", function () {
    meta(true);
  });
  addEventListener("load", function () {
    meta(true);
  });
  setInterval(function () {
    meta(false);
  }, 1500);
  addEventListener(
    "keydown",
    function (e) {
      if (!e.isTrusted || !e.ctrlKey) return;
      var key = e.key.toLowerCase(),
        k = null;
      if (e.altKey && e.key === "ArrowRight") k = "space_next";
      else if (e.altKey && e.key === "ArrowLeft") k = "space_prev";
      else if (!e.altKey && !e.shiftKey) {
        if (key === "t") k = "new_tab";
        else if (key === "w") k = "close";
        else if (key === "l") k = "focus_url";
      }
      if (k) {
        e.preventDefault();
        post({ t: "key", k: k });
      }
    },
    true,
  );
})();
