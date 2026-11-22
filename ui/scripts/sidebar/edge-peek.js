/* ---------- hover the left edge to reveal a collapsed sidebar (peek) ---------- */
{
  const EDGE = 6; // px from the window's left edge that triggers the peek
  const TOP = 40; // keep the title bar out of it
  let closeT = 0;
  let askedAt = 0;
  const ask = (open) => send({ cmd: "sidebar_peek", open });
  const cancelClose = () => {
    clearTimeout(closeT);
    closeT = 0;
  };
  const scheduleClose = () => {
    if (closeT || !S || !S.sidebar_peek || drag) return;
    closeT = setTimeout(() => {
      closeT = 0;
      if (!drag) ask(false);
    }, 220);
  };
  addEventListener(
    "mousemove",
    (e) => {
      if (!S || !S.sidebar_collapsed || resizingSidebar) return;
      if (!S.sidebar_peek) {
        if (
          e.clientX <= EDGE &&
          e.clientY >= TOP &&
          Date.now() - askedAt > 300
        ) {
          askedAt = Date.now();
          ask(true);
        }
        return;
      }
      if (e.clientX <= sidebarWidth + 14) cancelClose();
      else scheduleClose();
    },
    { passive: true },
  );
  document.documentElement.addEventListener("mouseleave", scheduleClose);
}
