// "Copy as" entries for the page right-click menu. Runs inside js/context-menu.js (filled in by
// context_menu.rs): `(element, url)` returns menu entries, each with the text to copy in `t`.
// `url` is the link or image under the pointer ("" for none). Everything is best effort: a
// selector that cannot be made is left out.
(el, url) => {
  const clean = (s) =>
    String(s || "")
      .replace(/\s+/g, " ")
      .trim();
  /* Short visible text only: a cut-off text would never match again. */
  const short = (s, n = 80) => {
    s = clean(s);
    return s.length <= n ? s : "";
  };
  const q = (s) =>
    "'" + clean(s).replace(/\\/g, "\\\\").replace(/'/g, "\\'") + "'";
  const dq = (s) => JSON.stringify(clean(s));
  const shell = (s) => "'" + String(s).replace(/'/g, "'\\''") + "'";
  const TEST_IDS = [
    "data-testid",
    "data-test-id",
    "data-test",
    "data-qa",
    "data-cy",
  ];
  const out = [];
  const add = (l, t) => t && out.push({ k: "i", l, t, e: true });

  /* ---- the address under the pointer ---- */
  if (url) {
    const ref = location.href;
    add(
      "cURL",
      `curl ${shell(url)} -H ${shell("user-agent: " + navigator.userAgent)} -H ${shell("referer: " + ref)}`,
    );
    add(
      "fetch",
      `await fetch(${JSON.stringify(url)}, ${JSON.stringify({ method: "GET", referrer: ref }, null, 2)});`,
    );
    out.push({ k: "s" });
  }

  /* ---- CSS selector: shortest unique path, preferring ids and test attributes ---- */
  const unique = (sel) => {
    try {
      return document.querySelectorAll(sel).length === 1;
    } catch (_) {
      return false;
    }
  };
  const stable = (c) =>
    c.length < 32 && !/^(css|sc|jsx|emotion|svelte)-|^_|\d{3,}/.test(c);
  const part = (n) => {
    if (n.id && unique("#" + CSS.escape(n.id)))
      return { s: "#" + CSS.escape(n.id), done: true };
    for (const a of TEST_IDS) {
      const v = n.getAttribute(a);
      if (v) {
        const s = `[${a}=${dq(v)}]`;
        if (unique(s)) return { s, done: true };
      }
    }
    let s =
      n.localName +
      [...n.classList]
        .filter(stable)
        .slice(0, 2)
        .map((c) => "." + CSS.escape(c))
        .join("");
    const p = n.parentElement;
    if (p && p.querySelectorAll(":scope > " + s).length > 1) {
      const same = [...p.children].filter((c) => c.localName === n.localName);
      s = n.localName + `:nth-of-type(${same.indexOf(n) + 1})`;
    }
    return { s };
  };
  const cssPath = () => {
    const parts = [];
    for (let n = el; n && n.nodeType === 1; n = n.parentElement) {
      const p = part(n);
      parts.unshift(p.s);
      const sel = parts.join(" > ");
      if (p.done || unique(sel)) return sel;
    }
    return parts.join(" > ");
  };

  /* ---- role and accessible name (the common cases of the ARIA mapping) ---- */
  const type = (el.getAttribute("type") || "text").toLowerCase();
  const INPUT_ROLES = {
    button: "button",
    submit: "button",
    reset: "button",
    image: "button",
    checkbox: "checkbox",
    radio: "radio",
    range: "slider",
    number: "spinbutton",
    search: "searchbox",
  };
  const TAG_ROLES = {
    button: "button",
    textarea: "textbox",
    nav: "navigation",
    main: "main",
    aside: "complementary",
    ul: "list",
    ol: "list",
    li: "listitem",
    table: "table",
    dialog: "dialog",
    img: "img",
    option: "option",
    progress: "progressbar",
    summary: "button",
  };
  const implicitRole = () => {
    const t = el.localName;
    if (t === "a") return el.hasAttribute("href") ? "link" : "";
    if (/^h[1-6]$/.test(t)) return "heading";
    if (t === "input")
      return type === "hidden" ? "" : INPUT_ROLES[type] || "textbox";
    if (t === "select")
      return el.multiple || el.size > 1 ? "listbox" : "combobox";
    return TAG_ROLES[t] || "";
  };
  const role =
    clean((el.getAttribute("role") || "").split(/\s+/)[0]) || implicitRole();
  const labelledBy = (el.getAttribute("aria-labelledby") || "")
    .split(/\s+/)
    .filter(Boolean);
  const fromContent =
    /^(link|button|heading|tab|menuitem|option|checkbox|radio|switch|cell|columnheader|listitem)$/.test(
      role,
    );
  const name =
    short(el.getAttribute("aria-label")) ||
    short(
      labelledBy
        .map((id) => (document.getElementById(id) || {}).textContent)
        .join(" "),
    ) ||
    short(el.labels && el.labels[0] && el.labels[0].textContent) ||
    (el.localName === "img" ? short(el.getAttribute("alt")) : "") ||
    (el.localName === "input" && INPUT_ROLES[type] === "button"
      ? short(el.value)
      : "") ||
    (fromContent ? short(el.innerText || el.textContent) : "") ||
    short(el.getAttribute("title"));

  const css = cssPath();
  const testId = TEST_IDS.map((a) => [a, el.getAttribute(a)]).find((x) => x[1]);
  const placeholder = short(el.getAttribute("placeholder"));
  const ownText = el.children.length === 0 ? short(el.textContent) : "";

  /* ---- Playwright: the most user-facing locator that exists ---- */
  let pw = `page.locator(${q(css)})`;
  if (testId) {
    pw =
      testId[0] === "data-testid"
        ? `page.getByTestId(${q(testId[1])})`
        : `page.locator(${q(`[${testId[0]}="${testId[1]}"]`)})`;
  } else if (role && name) {
    pw = `page.getByRole(${q(role)}, { name: ${q(name)} })`;
  } else if (el.labels && el.labels[0] && short(el.labels[0].textContent)) {
    pw = `page.getByLabel(${q(el.labels[0].textContent)})`;
  } else if (placeholder) {
    pw = `page.getByPlaceholder(${q(placeholder)})`;
  } else if (el.localName === "img" && short(el.getAttribute("alt"))) {
    pw = `page.getByAltText(${q(el.getAttribute("alt"))})`;
  } else if (ownText) {
    pw = `page.getByText(${q(ownText)})`;
  }

  /* ---- Puppeteer: aria selector when there is a role and name, else CSS ---- */
  const pp =
    role && name && !/[\]"\\]/.test(name)
      ? `page.locator(${q(`::-p-aria(${name}[role="${role}"])`)})`
      : `page.locator(${q(css)})`;

  add("CSS selector", css);
  add("Playwright locator", pw);
  add("Puppeteer locator", pp);
  if (role)
    add(
      "Role selector",
      name ? `role=${role}[name=${dq(name)}]` : `role=${role}`,
    );
  add("JS path", `document.querySelector(${q(css)})`);
  while (out.length && out[out.length - 1].k === "s") out.pop();
  return out;
};
