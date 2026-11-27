/* ---------- "Update available" card ----------
   The browser calls __updateOffer({version}) when a newer release exists at startup, and
   __updateOffer(null) once it was answered. Shares its look with the password card. */
(() => {
  let card = null;

  function hide() {
    if (card) card.remove();
    card = null;
  }

  function answer(cmd) {
    hide();
    send({ cmd });
  }

  function show({ version }) {
    const title = "poof " + version + " is available";
    card = el("div");
    card.id = "update-offer";
    card.setAttribute("role", "group");
    card.setAttribute("aria-label", title);

    const text = el("div", "pw-text");
    text.append(
      el("div", "pw-title", title),
      el("div", "pw-user", "Restarts poof to update"),
    );
    const head = el("div", "pw-head");
    head.append(ic("download-2", 18), text);

    const actions = el("div", "pw-actions");
    const now = btn("primary sm", "Update now", () => answer("update_accept"));
    const later = btn("ghost sm", "Later", () => answer("update_later"));
    now.title = title + ": download, install and restart";
    later.title = "Install when poof closes";
    actions.append(now, later);

    card.append(head, actions);
    $("side").append(card);
    // A collapsed sidebar is hidden: peek it open so the prompt can't go unseen.
    if (S && S.sidebar_collapsed) send({ cmd: "sidebar_peek", open: true });
  }

  window.__updateOffer = (info) => {
    hide();
    if (info) show(info);
  };
})();
