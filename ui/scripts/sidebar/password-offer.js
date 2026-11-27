/* ---------- "Save password?" card ----------
   The browser calls __passwordOffer({origin, username, update}) after the user submits a login,
   and __passwordOffer(null) once it was answered. The password itself never reaches this page. */
(() => {
  const GIVE_UP_MS = 2 * 60 * 1000; // an unanswered offer is dropped, and the browser forgets it
  let card = null;
  let giveUp = 0;

  const hostOf = (origin) => {
    try {
      return new URL(origin).host.replace(/^www\./, "");
    } catch (_) {
      return origin;
    }
  };

  function hide() {
    clearTimeout(giveUp);
    if (card) card.remove();
    card = null;
  }

  function answer(cmd) {
    hide();
    send(cmd);
  }

  function show({ origin, username, update }) {
    const host = hostOf(origin);
    const title =
      (update ? "Update password for " : "Save password for ") + host + "?";
    const who = username || "No username";

    card = el("div");
    card.id = "pw-offer";
    card.setAttribute("role", "group");
    card.setAttribute("aria-label", title);

    const text = el("div", "pw-text");
    text.append(el("div", "pw-title", title), el("div", "pw-user", who));
    const head = el("div", "pw-head");
    head.append(ic("lock", 18), text);

    const actions = el("div", "pw-actions");
    const yes = btn("primary sm", update ? "Update" : "Save", () =>
      answer({ cmd: "password_offer_accept" }),
    );
    const no = btn("ghost sm", "Not now", () =>
      answer({ cmd: "password_offer_dismiss", never: false }),
    );
    const never = btn("ghost sm pw-never", "Never", () =>
      answer({ cmd: "password_offer_dismiss", never: true }),
    );
    never.title = "Never offer to save passwords for " + host;
    // The compact rail shows icons only; the tooltip carries the details.
    yes.title = title + " (" + who + ")";
    no.title = "Not now";
    actions.append(yes, no, never);

    card.append(head, actions);
    $("side").append(card);
    giveUp = setTimeout(
      () => answer({ cmd: "password_offer_dismiss", never: false }),
      GIVE_UP_MS,
    );
    // A collapsed sidebar is hidden: peek it open so the question can't go unseen.
    if (S && S.sidebar_collapsed) send({ cmd: "sidebar_peek", open: true });
  }

  window.__passwordOffer = (info) => {
    hide();
    if (info) show(info);
  };
})();
