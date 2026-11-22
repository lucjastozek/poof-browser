/* ---------- light / dark ---------- */
const mq = matchMedia("(prefers-color-scheme: dark)");
let pref = "system";
function applyMode(p) {
  pref = p;
  document.documentElement.dataset.theme =
    p === "system" ? (mq.matches ? "dark" : "light") : p;
}
mq.addEventListener("change", () => applyMode(pref));
applyMode("system");
