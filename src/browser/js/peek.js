(function () {
  if (!/^https?:$/.test(location.protocol)) return;
  var on = __ON__;
  Object.defineProperty(window, "__poofPeekOn", {
    value: function (v) {
      on = !!v;
    },
    enumerable: false,
  });
  window.addEventListener(
    "click",
    function (e) {
      if (!on || !e.isTrusted || !e.altKey || e.button !== 0) return;
      var a = e.target && e.target.closest && e.target.closest("a[href]");
      if (!a || !/^https?:/i.test(a.href)) return;
      e.preventDefault();
      e.stopPropagation();
      try {
        window.ipc.postMessage(JSON.stringify({ t: "peek", u: a.href }));
      } catch (x) {}
    },
    true,
  );
})();
