/* ---------- saved passwords ----------
   Opened from Settings. The list holds site, username and date only; a password is requested
   from the browser when the user shows or copies it, and dropped from the page again after
   a few seconds. Styled by styles/views/passwords.css. */
(() => {
  const REVEAL_MS = 10000;
  const MASK = "••••••••••";

  let panel = null;
  let list = null;
  let search = null;
  let rows = [];
  let isOpen = false;
  // Passwords currently shown on screen (id -> timer), so they can be wiped.
  const shown = new Map();
  // What to do with the next password the browser sends back: "show" | "copy" per id.
  const wanted = new Map();

  const hostOf = (origin) => {
    try {
      return new URL(origin).host.replace(/^www\./, "");
    } catch (_) {
      return origin;
    }
  };

  async function copyText(value) {
    try {
      await navigator.clipboard.writeText(value);
    } catch (_) {
      const tmp = el("textarea");
      tmp.value = value;
      tmp.style.cssText = "position:fixed;opacity:0";
      document.body.append(tmp);
      tmp.select();
      document.execCommand("copy");
      tmp.remove();
    }
  }

  function siteIcon(host) {
    const box = el("span", "ico");
    box.textContent = host.charAt(0).toUpperCase();
    return box;
  }

  function forget(id) {
    clearTimeout(shown.get(id));
    shown.delete(id);
    wanted.delete(id);
  }

  function render() {
    if (!list) return;
    const query = search.value.trim().toLowerCase();
    const visible = rows.filter(
      (r) =>
        !query ||
        r.origin.toLowerCase().includes(query) ||
        r.username.toLowerCase().includes(query),
    );
    list.replaceChildren();
    if (!visible.length) {
      list.append(
        el(
          "div",
          "pw-empty",
          rows.length ? "No matching passwords." : "No saved passwords yet.",
        ),
      );
      return;
    }
    const card = el("div", "pw-card");
    visible.forEach((r) => card.append(row(r)));
    list.append(card);
  }

  function row(r) {
    const host = hostOf(r.origin);
    const secret = el("span", "pw-secret", MASK);
    secret.dataset.id = r.id;

    const site = el("div", "pw-site");
    site.append(
      el("div", "pw-host", host),
      el("div", "pw-user", r.username || "No username"),
    );

    // "eye" lives in the larger space-icon set, not the toolbar set that ib() draws from.
    const toggle = ib("close", "Show password", () => {
      if (shown.has(r.id)) {
        forget(r.id);
        hide();
      } else {
        wanted.set(r.id, "show");
        send({ cmd: "reveal_password", id: r.id });
      }
    });
    const hide = () => {
      secret.textContent = MASK;
      toggle.replaceChildren(sic("eye", 16));
      toggle.title = "Show password";
    };
    hide();
    const copy = ib("copy-2", "Copy password", () => {
      wanted.set(r.id, "copy");
      send({ cmd: "reveal_password", id: r.id });
    });
    const del = ib("delete-2", "Delete password", () => {
      // Two clicks to delete, because confirm() dialogs are not available in this WebView.
      if (del.classList.contains("sure")) {
        forget(r.id);
        send({ cmd: "delete_password", id: r.id });
        return;
      }
      del.classList.add("sure");
      del.title = "Click again to delete";
      setTimeout(() => {
        del.classList.remove("sure");
        del.title = "Delete password";
      }, 3000);
    });

    const line = el("div", "pw-row");
    line.title = r.origin;
    line.append(siteIcon(host), site, secret, toggle, copy, del);
    return line;
  }

  window.__setPasswords = (result) => {
    rows = (result && result.rows) || [];
    if (!panel) return;
    if (result && result.error) {
      list.replaceChildren(el("div", "pw-empty", result.error));
      return;
    }
    render();
  };

  window.__passwordSecret = ({ id, password }) => {
    const action = wanted.get(id);
    wanted.delete(id);
    if (action === "copy") {
      copyText(password).then(() => window.__toast("Password copied"));
    } else if (action === "show" && panel) {
      const cell = panel.querySelector('.pw-secret[data-id="' + id + '"]');
      if (!cell) return;
      cell.textContent = password;
      const button = cell.parentElement.querySelector(".ib");
      button.replaceChildren(sic("eye-close", 16));
      button.title = "Hide password";
      shown.set(
        id,
        setTimeout(() => {
          shown.delete(id);
          cell.textContent = MASK;
          button.replaceChildren(sic("eye", 16));
          button.title = "Show password";
        }, REVEAL_MS),
      );
    }
  };

  function build() {
    if (panel) return;
    panel = el("div");
    panel.id = "passwords-panel";
    panel.hidden = true;
    panel.setAttribute("role", "dialog");
    panel.setAttribute("aria-label", "Passwords");

    search = el("input");
    search.type = "text";
    search.placeholder = "Search passwords";
    search.spellcheck = false;
    search.autocomplete = "off";
    search.setAttribute("aria-label", "Search passwords");
    search.addEventListener("input", render);
    const searchField = el("label", "field");
    searchField.append(ic("search", 16), search);

    const head = el("div", "pw-headline");
    head.append(
      el("h1", null, "Passwords"),
      ib("close", "Close (Esc)", closePasswords),
    );
    const top = el("div", "pw-top");
    const topWrap = el("div", "pw-wrap");
    topWrap.append(
      head,
      el(
        "p",
        "pw-note",
        "Encrypted on this computer. The key is kept in your operating system's credential store.",
      ),
      searchField,
    );
    top.append(topWrap);

    const scroll = el("div", "pw-scroll");
    list = el("div", "pw-wrap");
    scroll.append(list);
    panel.append(top, scroll);
    document.body.append(panel);
  }

  function openPasswords() {
    build();
    search.value = "";
    panel.hidden = false;
    isOpen = true;
    if (!ov) send({ cmd: "overlay", open: true });
    send({ cmd: "list_passwords" });
    search.focus();
  }

  function closePasswords() {
    if (!isOpen) return;
    isOpen = false;
    panel.hidden = true;
    [...shown.keys()].forEach(forget);
    rows = [];
    list.replaceChildren();
    if (!ov) send({ cmd: "overlay", open: false });
  }

  window.__openPasswords = openPasswords;

  // Clicking in the sidebar (a tab, a space) leaves the panel.
  document.addEventListener(
    "pointerdown",
    (e) => {
      if (
        isOpen &&
        e.target.closest &&
        e.target.closest("#side") &&
        !e.target.closest("#side-resize")
      )
        closePasswords();
    },
    true,
  );
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && isOpen) {
      e.preventDefault();
      e.stopPropagation();
      closePasswords();
    }
  });
})();

/* The Settings section: turn saving and filling on or off, and open the list. */
function passwordSettingsField() {
  const toggle = el("button", "switch");
  toggle.type = "button";
  toggle.setAttribute("role", "switch");
  toggle.setAttribute("aria-checked", String(S.save_passwords !== false));
  const text = el("span", "t");
  text.append(
    el("span", null, "Save and fill passwords"),
    el(
      "small",
      null,
      "Offer to save logins, and fill them in on the same site",
    ),
  );
  toggle.append(text, el("span", "track"));
  toggle.onclick = () => {
    const on = toggle.getAttribute("aria-checked") !== "true";
    toggle.setAttribute("aria-checked", String(on));
    S.save_passwords = on;
    send({ cmd: "set_save_passwords", on });
  };
  const manage = btn("sm", "Manage passwords", () => window.__openPasswords());
  return field("Passwords", toggle, manage);
}
