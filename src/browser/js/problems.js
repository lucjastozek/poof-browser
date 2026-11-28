// Counts the page's problems for the tab row's red dot: console errors, uncaught errors and
// rejections, scripts and stylesheets that fail to load, and failed requests (network errors,
// 5xx, and 4xx to the page's own origin). Event-driven; reports the total at most every 400 ms.
(function () {
  if (window.top !== window) return;
  if (
    /^(challenges\.cloudflare\.com|accounts\.google\.com)$/.test(
      location.hostname,
    )
  )
    return;
  var n = 0,
    sent = -1,
    timer = 0;
  function post() {
    timer = 0;
    if (n === sent) return;
    sent = n;
    try {
      window.ipc.postMessage(JSON.stringify({ t: "problems", c: n }));
    } catch (e) {}
  }
  function bump() {
    if (n < 999) n++;
    if (!timer) timer = setTimeout(post, 400);
  }
  function bad(status, url) {
    if (status >= 500) return true;
    if (status < 400) return false;
    try {
      return new URL(url, location.href).origin === location.origin;
    } catch (e) {
      return false;
    }
  }
  post(); // a new page starts at zero, which also clears the count of the one before

  try {
    var ce = console.error;
    console.error = function () {
      bump();
      return ce.apply(this, arguments);
    };
  } catch (e) {}
  addEventListener(
    "error",
    function (e) {
      var t = e.target;
      if (!t || t === window) return bump();
      if (
        t.tagName === "SCRIPT" ||
        (t.tagName === "LINK" && /stylesheet/i.test(t.rel))
      )
        bump();
    },
    true,
  );
  addEventListener("unhandledrejection", bump);
  try {
    var f = window.fetch;
    if (f)
      window.fetch = function () {
        var p = f.apply(this, arguments);
        try {
          p.then(
            function (r) {
              if (bad(r.status, r.url)) bump();
            },
            function (err) {
              if (!err || err.name !== "AbortError") bump();
            },
          );
        } catch (e) {}
        return p;
      };
    var send = XMLHttpRequest.prototype.send;
    XMLHttpRequest.prototype.send = function () {
      try {
        var x = this,
          aborted = false;
        x.addEventListener("abort", function () {
          aborted = true;
        });
        x.addEventListener("loadend", function () {
          if (aborted) return;
          if (x.status === 0 || bad(x.status, x.responseURL)) bump();
        });
      } catch (e) {}
      return send.apply(this, arguments);
    };
  } catch (e) {}
})();
