/* ---------- empty states ----------
   Favorites and pinned tabs show a small ghost with a hint while they are empty. The native side
   decides when (`S.hints`): from the 3rd tab ever opened to the 49th, and never once favorites
   or pins have been used in any space. The ghost is decoration: keyboard and screen reader users
   have Alt+Arrow keys and the tab menu, so it is hidden from assistive technology. */
(() => {
  function hint(kind, idle, drag) {
    const box = el("div", "ghost-hint");
    if (!shown[kind]) box.classList.add("enter");
    box.setAttribute("aria-hidden", "true");
    const icon = ic(kind === "fav" ? "star" : "pin-2");
    box.append(icon, el("div", "gh-text", ""));
    const t = box.querySelector(".gh-text");
    t.append(
      el("b", "gh-idle", idle[0]),
      el("span", "gh-idle", idle[1]),
      el("b", "gh-drag", drag),
    );
    return box;
  }

  // A hint only animates in when it newly appears, not on every state push.
  const shown = { fav: false, pin: false };
  const base = window.__setState;
  window.__setState = (s) => {
    base(s);
    const h = s.hints || {};
    const favs = $("favs"),
      pinned = $("pinned");
    document.querySelectorAll(".ghost-hint").forEach((n) => n.remove());
    const showFav = !!h.favorites && !s.favorites.length;
    const showPin = !!h.pinned && !pinned.childElementCount;
    favs.classList.toggle("hint", showFav);
    pinned.classList.toggle("hint", showPin);
    if (showFav)
      favs.append(
        hint(
          "fav",
          ["Favourites live up here", "Drag a tab into this spot"],
          "Drop it here!",
        ),
      );
    if (showPin)
      pinned.append(
        hint(
          "pin",
          ["Pin what you always need", "Drag a tab into this spot"],
          "Pin it here!",
        ),
      );
    shown.fav = showFav;
    shown.pin = showPin;
  };
})();
