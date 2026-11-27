// Fullscreen through the site's own player. Nothing here draws a player or restyles a video:
// the toggle presses the fullscreen button the page already has, so the site's layout,
// controls and subtitles stay as they are. Top frame only; embedded players that own
// their video handle F themselves.
(function () {
  if (window.top !== window) return;

  var BUTTONS = [
    ".ytp-fullscreen-button",
    ".vjs-fullscreen-control",
    "[data-plyr='fullscreen']",
    "button[aria-label*='ullscreen' i]",
    "button[title*='ullscreen' i]",
    "button[aria-label*='full screen' i]",
    "button[title*='full screen' i]",
    "[role='button'][aria-label*='ullscreen' i]",
    "[data-testid*='ullscreen' i]",
    "[class*='fullscreen' i][class*='button' i]",
    "[class*='fullscreen' i][class*='btn' i]",
    "[class*='fullscreen' i][class*='control' i]",
  ].join(",");

  function visible(el) {
    var r = el.getBoundingClientRect();
    var cs = getComputedStyle(el);
    return (
      r.width > 0 &&
      r.height > 0 &&
      cs.visibility !== "hidden" &&
      cs.display !== "none"
    );
  }

  function isExitControl(el) {
    var label = (
      (el.getAttribute("aria-label") || "") +
      " " +
      (el.title || "")
    ).toLowerCase();
    return /exit|leave|close/.test(label);
  }

  function biggestVideo() {
    var best = null;
    var area = 0;
    document.querySelectorAll("video").forEach(function (v) {
      var r = v.getBoundingClientRect();
      if (r.width * r.height > area && visible(v)) {
        area = r.width * r.height;
        best = v;
      }
    });
    return best;
  }

  // Players hide their controls while the pointer is idle (display:none, opacity:0 or not
  // rendered at all), so a hidden button still counts. Visible ones are preferred.
  function usable(el) {
    return !isExitControl(el) && !el.disabled;
  }

  // Looks for the control next to the video first, so pages with several players
  // press the right one, then anywhere on the page.
  function findFullscreenButton(video) {
    var scope = video;
    for (var i = 0; scope && i < 10; i++) {
      scope = scope.parentElement;
      var found = scope && pick(scope.querySelectorAll(BUTTONS));
      if (found) return found;
    }
    return pick(document.querySelectorAll(BUTTONS));
  }

  function pick(list) {
    var all = Array.prototype.filter.call(list, usable);
    return all.find(visible) || all[0] || null;
  }

  // Makes the player show its controls, as if the pointer moved over it.
  function wakeControls(video) {
    var target = video || document.body;
    var r = target.getBoundingClientRect();
    [
      "pointerover",
      "pointerenter",
      "mouseover",
      "mouseenter",
      "pointermove",
      "mousemove",
    ].forEach(function (type) {
      target.dispatchEvent(
        new MouseEvent(type, {
          bubbles: true,
          clientX: r.left + r.width / 2,
          clientY: r.top + r.height / 2,
        }),
      );
    });
  }

  // The player's own container: the highest ancestor that is still about the video's size.
  // Fullscreening it keeps the site's controls, subtitles and overlays.
  function playerContainer(video) {
    var vr = video.getBoundingClientRect();
    var best = video;
    for (
      var el = video.parentElement;
      el && el !== document.body;
      el = el.parentElement
    ) {
      var r = el.getBoundingClientRect();
      if (r.width * r.height > vr.width * vr.height * 1.25) break;
      best = el;
    }
    return best;
  }

  function visibleEmbed() {
    return Array.prototype.find.call(
      document.querySelectorAll("iframe[allowfullscreen]"),
      function (f) {
        var r = f.getBoundingClientRect();
        return visible(f) && r.width > 200 && r.height > 120;
      },
    );
  }

  function pressFullscreenKey(video) {
    var target = video || document.body;
    ["keydown", "keyup"].forEach(function (type) {
      var e = new KeyboardEvent(type, {
        key: "f",
        code: "KeyF",
        keyCode: 70,
        which: 70,
        bubbles: true,
      });
      target.dispatchEvent(e);
    });
  }

  // Runs with a user gesture (the native side evaluates it through the DevTools protocol).
  // Order: the site's own button, then its F shortcut, and only then the player container.
  window.__poofToggleFullscreen = function () {
    if (document.fullscreenElement) {
      document.exitFullscreen().catch(function () {});
      return;
    }
    var video = biggestVideo();
    var button = findFullscreenButton(video);
    if (button) {
      button.click();
      return;
    }
    wakeControls(video);
    setTimeout(function () {
      if (document.fullscreenElement) return;
      var late = findFullscreenButton(video);
      if (late) {
        late.click();
        return;
      }
      pressFullscreenKey(video);
      setTimeout(function () {
        if (document.fullscreenElement) return;
        var target = video ? playerContainer(video) : visibleEmbed();
        if (target && target.requestFullscreen)
          target.requestFullscreen().catch(function () {});
      }, 250);
    }, 200);
  };

  // F with the page focused: the site's own handler goes first. If nothing happened
  // shortly after, do what the player's button would do.
  function typing(el) {
    return !!(
      el &&
      (el.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName))
    );
  }
  addEventListener("keydown", function (e) {
    if (
      !e.isTrusted ||
      e.repeat ||
      e.ctrlKey ||
      e.altKey ||
      e.metaKey ||
      e.shiftKey
    )
      return;
    if (e.key.toLowerCase() !== "f" || typing(e.target) || e.defaultPrevented)
      return;
    var before = !!document.fullscreenElement;
    setTimeout(function () {
      if (before === !!document.fullscreenElement && biggestVideo())
        window.__poofToggleFullscreen();
    }, 150);
  });
})();
