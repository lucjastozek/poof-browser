/* ---------- "Continue" cards ----------
   On launch, one line per space that was in use: click it to wake just those tabs. The native
   side decides which cards exist (`S.resume`); this only draws them above the tab list. */
(() => {
  const base = window.__setState;
  let last = "";
  window.__setState = (s) => {
    base(s);
    const cards = s.resume || [];
    const key = JSON.stringify(cards);
    if (key === last) return;
    last = key;
    let host = $("resume");
    if (!host) {
      host = el("div");
      host.id = "resume";
      $("pane").before(host);
    }
    host.replaceChildren(
      ...cards.map((c) => {
        const card = el("div", "resume-card");
        card.style.setProperty("--c", c.color);
        const go = el("button", "resume-go");
        go.type = "button";
        go.title = c.name;
        go.setAttribute("aria-label", c.label + ", in " + c.name);
        go.append(el("span", "resume-dot"), el("span", "resume-txt", c.label));
        go.onclick = () => send({ cmd: "resume_space", id: c.space });
        card.append(
          go,
          ib("close", "Dismiss", () =>
            send({ cmd: "resume_dismiss", id: c.space }),
          ),
        );
        return card;
      }),
    );
  };
})();
