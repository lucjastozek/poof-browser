/* ---------- settings: Boosts ----------
   Per-site custom CSS and JavaScript, and elements zapped off a page. The native side owns
   the list; edits are sent when a field loses focus. Repaints only when the list changes, so
   typing in a box is never interrupted by a state push. */
const boostHost = () => {
  try {
    return new URL(S.url).hostname.replace(/^www\./, "");
  } catch (e) {
    return "";
  }
};

function boostCode(label, value, placeholder, change) {
  const t = el("textarea", "inp boost-code");
  t.value = value;
  t.placeholder = placeholder;
  t.spellcheck = false;
  t.setAttribute("aria-label", label);
  t.onchange = () => change(t.value);
  return t;
}

function boostCard(b) {
  const save = (patch) => {
    Object.assign(b, patch);
    send({ cmd: "save_boost", ...b });
  };
  const head = el("div", "boost-head");
  head.append(
    el("span", "retention-name", b.domain),
    ib("delete-2", "Delete boost", () =>
      send({ cmd: "delete_boost", domain: b.domain }),
    ),
  );
  const zaps = el("div", "boost-zaps");
  b.zap.forEach((sel) => {
    const row = el("div", "boost-zap");
    row.append(
      el("code", null, sel),
      ib("close", "Show this element again", () =>
        save({ zap: b.zap.filter((s) => s !== sel) }),
      ),
    );
    zaps.append(row);
  });
  const card = el("div", "boost-card");
  card.append(
    head,
    switchRow(
      "On",
      "Apply this boost on " + b.domain + " and its subdomains",
      b.enabled,
      (on) => save({ enabled: on }),
    ),
    boostCode(
      "Custom CSS",
      b.css,
      "/* custom CSS */\nbody { font-size: 110%; }",
      (css) => save({ css }),
    ),
    boostCode("Custom JavaScript", b.js, "// runs after the page loads", (js) =>
      save({ js }),
    ),
  );
  if (b.zap.length) card.append(el("span", "label", "Hidden elements"), zaps);
  return card;
}

function boostsPanel() {
  const list = el("div", "boost-list");
  const signature = () => JSON.stringify([S.boosts]);
  let painted = "";
  const paint = () => {
    painted = signature();
    const boosts = S.boosts || [];
    list.replaceChildren(
      ...(boosts.length
        ? boosts.map((b) => boostCard({ ...b, zap: [...b.zap] }))
        : [el("p", "note", "No boosts yet.")]),
    );
  };
  paint();
  window.__repaintBoosts = () => {
    if (list.isConnected && signature() !== painted) paint();
  };

  const domain = el("input", "inp");
  domain.placeholder = "example.com";
  domain.value = boostHost();
  domain.spellcheck = false;
  domain.setAttribute("aria-label", "Site");
  const add = () => {
    if (!domain.value.trim()) return domain.focus();
    send({
      cmd: "save_boost",
      domain: domain.value,
      enabled: true,
      css: "",
      js: "",
      zap: [],
    });
    domain.value = "";
  };
  domain.onkeydown = (e) => {
    if (e.key === "Enter") add();
  };
  const addRow = el("div", "boost-add");
  addRow.append(domain, btn("primary sm", "Add boost", add));

  const zap = btn("sm", "Zap an element on this page…", () => {
    closeView();
    send({ cmd: "start_zap" });
  });
  return [
    field(
      "Site boosts",
      el(
        "div",
        "note",
        "Custom CSS or JavaScript for a site, and elements you hide. A boost also covers the site's subdomains. JavaScript runs after the page loads.",
      ),
      list,
    ),
    field("Add a boost", addRow),
    field("Zap", zap),
  ];
}
