// Tells the browser when this page must not be auto-slept (unsent form input, a long scroll
// position) and keeps a snapshot of scroll position and form values so a woken tab picks up
// where it was. Event-driven and debounced; password/hidden/file fields are never captured.
(() => {
  try {
    if (window.__poofSleepGuard || !/^https?:$/.test(location.protocol)) return;
    window.__poofSleepGuard = 1;
    const SEL = "input,textarea,select";
    const dirty = new WeakSet();
    let timer = 0;
    let last = "";
    const skip = (e) =>
      e.tagName === "INPUT" &&
      /^(password|hidden|file|submit|button|reset|image)$/i.test(e.type);
    const fields = () => {
      const out = [];
      const els = document.querySelectorAll(SEL);
      for (let i = 0; i < els.length && out.length < 200; i++) {
        const e = els[i];
        if (skip(e)) continue;
        const chk = e.type === "checkbox" || e.type === "radio";
        const v = chk ? (e.checked ? "1" : "") : String(e.value || "");
        if (!v || v === (e.defaultValue || "")) continue;
        out.push([i, e.name || e.id || "", chk ? 1 : 0, v.slice(0, 5000)]);
      }
      return out;
    };
    const unsent = () => {
      for (const e of document.querySelectorAll(SEL)) {
        if (skip(e) && e.type !== "password") continue;
        if (dirty.has(e) && (e.value || e.checked)) return true;
      }
      for (const e of document.querySelectorAll("[contenteditable='true']"))
        if (dirty.has(e) && (e.textContent || "").trim()) return true;
      return false;
    };
    const send = () => {
      timer = 0;
      const y = Math.round(window.scrollY || 0);
      const hold =
        unsent() || (y > 1500 && y > 3 * (window.innerHeight || 800));
      const f = fields();
      const m = JSON.stringify({
        t: "snap",
        hold,
        u: location.href.split("#")[0].slice(0, 2048),
        y,
        f,
      });
      if (m === last) return;
      last = m;
      window.ipc.postMessage(m);
    };
    const soon = () => {
      if (!timer) timer = setTimeout(send, 600);
    };
    document.addEventListener(
      "input",
      (e) => {
        if (e.isTrusted && e.target) dirty.add(e.target);
        soon();
      },
      { capture: true, passive: true },
    );
    document.addEventListener("change", soon, { capture: true, passive: true });
    document.addEventListener(
      "submit",
      (e) => {
        for (const el of e.target.querySelectorAll(SEL)) dirty.delete(el);
        setTimeout(send, 300);
      },
      { capture: true, passive: true },
    );
    addEventListener("scroll", soon, { passive: true });

    const snap = __SNAP__;
    if (snap && snap.u === location.href.split("#")[0]) {
      const restore = () => {
        try {
          const els = document.querySelectorAll(SEL);
          for (const [i, name, chk, v] of snap.f || []) {
            const e = els[i];
            if (!e || (e.name || e.id || "") !== name || skip(e)) continue;
            if (chk) e.checked = true;
            else e.value = v;
            dirty.add(e);
          }
          let tries = 0;
          const go = () => {
            window.scrollTo(0, snap.y || 0);
            if (
              Math.abs((window.scrollY || 0) - (snap.y || 0)) > 2 &&
              ++tries < 30
            )
              setTimeout(go, 100);
          };
          if (snap.y) go();
          soon();
        } catch (_) {}
      };
      if (document.readyState === "complete") restore();
      else
        addEventListener("load", () => setTimeout(restore, 150), {
          once: true,
        });
    }
  } catch (_) {}
})();
