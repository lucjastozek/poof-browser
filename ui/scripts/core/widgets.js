const ib = (n, tip, fn) => {
  const b = el("button", "ib");
  b.title = tip;
  b.setAttribute("aria-label", tip);
  b.append(ic(n));
  b.onclick = (e) => {
    e.stopPropagation();
    fn();
  };
  return b;
};
const btn = (cls, txt, fn) => {
  const b = el("button", "btn " + cls, txt);
  if (fn) b.onclick = fn;
  return b;
};
