window.__toast = (m) => {
  const t = $("toast");
  t.classList.remove("undo");
  t.setAttribute("aria-live", "polite");
  t.textContent = m;
  t.classList.add("show");
  clearTimeout(t._h);
  t._h = setTimeout(() => t.classList.remove("show"), 4200);
};
/* A toast with an Undo button (the native side runs the undo). It is announced here once, in
   full, so the tab-list announcements stay quiet for a moment. */
window.__undoToast = (m) => {
  const t = $("toast");
  t.classList.add("undo", "show");
  t.setAttribute("aria-live", "off");
  const msg = el("span", "toast-msg", m);
  const btn = el("button", "toast-undo", "Undo");
  btn.type = "button";
  btn.append(el("kbd", "", "Ctrl Z"));
  btn.onclick = () => {
    t.classList.remove("show");
    send({ cmd: "undo" });
  };
  t.replaceChildren(msg, btn);
  clearTimeout(t._h);
  t._h = setTimeout(() => t.classList.remove("show"), 7000);
  window.__quietUntil = Date.now() + 700;
  window.__announce && window.__announce(m + ". Press Control Z to undo.");
};
