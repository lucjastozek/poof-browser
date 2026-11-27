(function () {
  var boosts = __BOOSTS__;
  var host = location.hostname.toLowerCase().replace(/^www\./, "");
  if (!/^https?:$/.test(location.protocol)) return;
  var sheet = null;
  var style = null;

  function matches(d) {
    return host === d || host.slice(-d.length - 1) === "." + d;
  }
  function validSelector(s) {
    try {
      document.createDocumentFragment().querySelector(s);
      return true;
    } catch (e) {
      return false;
    }
  }
  function cssText() {
    var out = "";
    boosts.forEach(function (b) {
      if (!matches(b.d)) return;
      out += (b.css || "") + "\n";
      (b.zap || []).forEach(function (s) {
        if (validSelector(s)) out += s + "{display:none!important}\n";
      });
    });
    return out;
  }
  function paint() {
    var text = cssText();
    try {
      if (!sheet) sheet = new CSSStyleSheet();
      sheet.replaceSync(text);
      var cur = document.adoptedStyleSheets;
      if (cur.indexOf(sheet) < 0)
        document.adoptedStyleSheets = cur.concat(sheet);
      return;
    } catch (e) {}
    if (!style) {
      style = document.createElement("style");
    }
    style.textContent = text;
    var root = document.head || document.documentElement;
    if (root && style.parentNode !== root) root.appendChild(style);
  }
  function post(o) {
    try {
      window.ipc.postMessage(JSON.stringify(o));
    } catch (e) {}
  }
  function wantsJs() {
    return boosts.some(function (b) {
      return b.js && matches(b.d);
    });
  }
  function runJs() {
    if (wantsJs()) post({ t: "boost_run" });
  }

  Object.defineProperty(window, "__poofBoosts", {
    value: function (list) {
      boosts = list;
      paint();
    },
    enumerable: false,
  });
  paint();
  // Some sites replace adopted style sheets or <head>; paint again once the page is parsed.
  var loaded = function () {
    paint();
    runJs();
  };
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", loaded, { once: true });
  } else {
    loaded();
  }

  // ----- element picker ("zap") -----
  var IDENT = /^[A-Za-z_][\w-]*$/;
  function count(sel) {
    try {
      return document.querySelectorAll(sel).length;
    } catch (e) {
      return 0;
    }
  }
  function selectorFor(el) {
    var tag = el.localName;
    if (el.id && IDENT.test(el.id) && count("#" + el.id) === 1)
      return "#" + el.id;
    var classes = [].filter.call(el.classList, function (c) {
      return IDENT.test(c) && c.length < 40;
    });
    if (classes.length) {
      var cls = tag + "." + classes.join(".");
      if (count(cls) === 1) return cls;
    }
    var parts = [];
    for (
      var n = el;
      n && n.nodeType === 1 && n !== document.documentElement;

    ) {
      if (n.id && IDENT.test(n.id) && count("#" + n.id) === 1) {
        parts.unshift("#" + n.id);
        break;
      }
      var t = n.localName;
      var p = n.parentElement;
      if (p) {
        var same = [].filter.call(p.children, function (c) {
          return c.localName === t;
        });
        if (same.length > 1) t += ":nth-of-type(" + (same.indexOf(n) + 1) + ")";
      }
      parts.unshift(t);
      n = p;
    }
    return parts.join(" > ");
  }

  var zapping = null;
  function stopZap() {
    if (!zapping) return;
    zapping.stop();
    zapping = null;
  }
  Object.defineProperty(window, "__poofZap", {
    value: function () {
      if (zapping || !document.documentElement) return;
      var box = document.createElement("div");
      box.style.cssText =
        "position:fixed;pointer-events:none;z-index:2147483647;border:2px solid #e5484d;" +
        "background:rgba(229,72,77,.18);border-radius:3px;display:none;transition:all .05s";
      var tip = document.createElement("div");
      tip.textContent = "Click an element to hide it · Esc to cancel";
      tip.style.cssText =
        "position:fixed;left:50%;top:12px;transform:translateX(-50%);z-index:2147483647;" +
        "pointer-events:none;font:13px system-ui,sans-serif;color:#fff;background:#222;" +
        "padding:6px 12px;border-radius:999px;box-shadow:0 2px 10px rgba(0,0,0,.4)";
      document.documentElement.append(box, tip);
      var oldCursor = document.documentElement.style.cursor;
      document.documentElement.style.cursor = "crosshair";
      var target = null;
      function pick(e) {
        var el = document.elementFromPoint(e.clientX, e.clientY);
        return el && el !== document.documentElement && el !== document.body
          ? el
          : null;
      }
      function move(e) {
        target = pick(e);
        if (!target) {
          box.style.display = "none";
          return;
        }
        var r = target.getBoundingClientRect();
        box.style.display = "block";
        box.style.left = r.left + "px";
        box.style.top = r.top + "px";
        box.style.width = r.width + "px";
        box.style.height = r.height + "px";
      }
      function swallow(e) {
        e.preventDefault();
        e.stopPropagation();
        e.stopImmediatePropagation();
      }
      function click(e) {
        swallow(e);
        var el = pick(e) || target;
        stopZap();
        if (el) post({ t: "boost_zap", s: selectorFor(el) });
      }
      function key(e) {
        if (e.key === "Escape") {
          swallow(e);
          stopZap();
        }
      }
      var evs = [
        "mousedown",
        "mouseup",
        "pointerdown",
        "pointerup",
        "auxclick",
        "contextmenu",
        "dblclick",
      ];
      window.addEventListener("mousemove", move, true);
      window.addEventListener("click", click, true);
      window.addEventListener("keydown", key, true);
      evs.forEach(function (n) {
        window.addEventListener(n, swallow, true);
      });
      zapping = {
        stop: function () {
          window.removeEventListener("mousemove", move, true);
          window.removeEventListener("click", click, true);
          window.removeEventListener("keydown", key, true);
          evs.forEach(function (n) {
            window.removeEventListener(n, swallow, true);
          });
          box.remove();
          tip.remove();
          document.documentElement.style.cursor = oldCursor;
        },
      };
    },
    enumerable: false,
  });
})();
