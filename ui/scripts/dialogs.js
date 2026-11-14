/* ---------- dialog scaffold ---------- */
function dialog(v, title, body, foot) {
  const wrap = el("div", "dlg-wrap"),
    d = el("div", "dlg");
  d.setAttribute("role", "dialog");
  d.setAttribute("aria-label", title);
  const h = el("header");
  h.append(el("h2", null, title), ib("x", "Close", closeView));
  const b = el("div", "body");
  b.append(...body);
  d.append(h, b);
  if (foot) {
    const f = el("footer");
    f.append(...foot);
    d.append(f);
  }
  wrap.append(d);
  wrap.addEventListener("pointerdown", (e) => {
    if (e.target === wrap) closeView();
  });
  v.replaceChildren(wrap);
}
const field = (l, ...c) => {
  const d = el("div", "fl");
  if (l) d.append(el("span", "label", l));
  d.append(...c);
  return d;
};

/* ---------- space editor ---------- */
function buildSpace(v, mode) {
  const sp = curSpace(),
    add = mode === "add";
  const used = new Set(S.spaces.map((x) => x.color.toLowerCase()));
  let color = add
    ? PRESETS.find((c) => !used.has(c)) || PRESETS[5]
    : sp.color.toLowerCase();
  let icn = add
    ? "sparkles"
    : SPACE_ICONS[sp.icon] || sp.icon.startsWith("svg:")
      ? sp.icon
      : "house";
  let [H, SA, V] = rgb2hsv(hex2rgb(color));
  V = Math.max(V, 0.2);
  applyColor(color);

  const chip = el("span", "bigchip");
  const name = el("input", "inp");
  name.value = add ? "" : sp.name;
  name.maxLength = 40;
  name.placeholder = "Name this space";
  name.setAttribute("aria-label", "Space name");
  const nrow = el("div", "namerow");
  nrow.append(chip, name);

  const grid = el("div", "icons");
  const iconSearch = el("input", "inp");
  iconSearch.placeholder = "Search Lucide icons";
  iconSearch.setAttribute("aria-label", "Search icons");
  const upload = el("input");
  upload.type = "file";
  upload.accept = ".svg,image/svg+xml";
  upload.hidden = true;
  const uploadBtn = btn("sm ghost", "Upload SVG", () => upload.click());
  const paintIcons = () => {
    const q = iconSearch.value.trim().toLowerCase();
    const names = Object.keys(SPACE_ICONS).filter((n) => n.includes(q));
    grid.replaceChildren(
      ...names.map((n) => {
        const b = el("button", n === icn ? "on" : "");
        b.type = "button";
        b.title = n.replace(/-/g, " ");
        b.setAttribute("aria-label", n);
        b.append(sic(n, 18));
        b.onclick = () => {
          icn = n;
          sync();
        };
        return b;
      }),
    );
  };
  iconSearch.addEventListener("input", paintIcons);
  uploadBtn.onclick = () => upload.click();
  upload.addEventListener("change", async () => {
    const f = upload.files && upload.files[0];
    if (!f) return;
    if (f.size > 16384) {
      window.__toast("SVG must be 16 KB or smaller");
      upload.value = "";
      return;
    }
    try {
      const source = await f.text(),
        doc = new DOMParser().parseFromString(source, "image/svg+xml");
      if (
        doc.querySelector("parsererror") ||
        doc.documentElement.localName !== "svg"
      )
        throw Error("Invalid SVG");
      const allowed = new Set([
        "svg",
        "g",
        "path",
        "circle",
        "rect",
        "line",
        "polyline",
        "polygon",
        "ellipse",
      ]);
      const attrs = new Set([
        "viewBox",
        "xmlns",
        "d",
        "cx",
        "cy",
        "r",
        "x",
        "y",
        "width",
        "height",
        "rx",
        "ry",
        "x1",
        "y1",
        "x2",
        "y2",
        "points",
        "fill",
        "stroke",
        "stroke-width",
        "stroke-linecap",
        "stroke-linejoin",
        "fill-rule",
        "clip-rule",
      ]);
      const root = doc.documentElement;
      for (const node of [...root.querySelectorAll("*")].reverse()) {
        if (!allowed.has(node.localName)) node.remove();
        else
          for (const a of [...node.attributes])
            if (!attrs.has(a.name)) node.removeAttribute(a.name);
      }
      for (const a of [...root.attributes])
        if (!attrs.has(a.name)) root.removeAttribute(a.name);
      root.setAttribute("viewBox", root.getAttribute("viewBox") || "0 0 24 24");
      root.setAttribute("xmlns", "http://www.w3.org/2000/svg");
      const clean = new XMLSerializer().serializeToString(root);
      if (
        clean.length > 6144 ||
        /<script|javascript:|url\(|on[a-z]+=/i.test(clean)
      )
        throw Error("Unsafe or oversized SVG");
      icn = "svg:" + btoa(unescape(encodeURIComponent(clean)));
      sync();
    } catch (e) {
      window.__toast("Could not use that SVG. Use simple paths and shapes.");
    } finally {
      upload.value = "";
    }
  });

  const W = 156,
    R = W / 2;
  const wheel = el("div", "wheel"),
    shade = el("div", "shade"),
    knob = el("div", "knob");
  wheel.tabIndex = 0;
  wheel.setAttribute("role", "slider");
  wheel.setAttribute("aria-label", "Color");
  shade.className = "shade";
  wheel.append(shade, knob);
  const sw = el("div", "swatches");
  const range = el("input");
  range.type = "range";
  range.min = 20;
  range.max = 100;
  range.setAttribute("aria-label", "Brightness");
  const hex = el("input", "inp hex");
  hex.maxLength = 7;
  hex.spellcheck = false;
  hex.setAttribute("aria-label", "Hex color");

  function fromHsv() {
    color = rgb2hex(hsv2rgb(H, SA, V));
    sync();
  }
  function setHex(c) {
    color = c.toLowerCase();
    [H, SA, V] = rgb2hsv(hex2rgb(color));
    V = Math.max(V, 0.2);
    sync();
  }
  function sync(skipHex) {
    applyColor(color);
    chip.replaceChildren(sic(icn, 20));
    const a = (H * Math.PI) / 180;
    knob.style.left = R + Math.sin(a) * SA * R + "px";
    knob.style.top = R - Math.cos(a) * SA * R + "px";
    knob.style.background = color;
    shade.style.opacity = 1 - V;
    range.value = Math.round(V * 100);
    range.style.setProperty("--c0", rgb2hex(hsv2rgb(H, SA, 0.2)));
    range.style.setProperty("--c1", rgb2hex(hsv2rgb(H, SA, 1)));
    if (!skipHex) hex.value = color;
    [...sw.children].forEach((b, i) =>
      b.classList.toggle("on", PRESETS[i] === color),
    );
    [...grid.children].forEach((b) =>
      b.classList.toggle("on", b.getAttribute("aria-label") === icn),
    );
  }
  PRESETS.forEach((c) => {
    const b = el("button");
    b.type = "button";
    b.style.background = c;
    b.title = c;
    b.setAttribute("aria-label", c);
    b.onclick = () => setHex(c);
    sw.append(b);
  });
  const pick = (e) => {
    const r = wheel.getBoundingClientRect(),
      dx = e.clientX - (r.left + R),
      dy = e.clientY - (r.top + R);
    H = ((Math.atan2(dx, -dy) * 180) / Math.PI + 360) % 360;
    SA = Math.min(1, Math.hypot(dx, dy) / R);
    fromHsv();
  };
  wheel.onpointerdown = (e) => {
    wheel.setPointerCapture(e.pointerId);
    pick(e);
    wheel.onpointermove = pick;
    wheel.onpointerup = wheel.onpointercancel = () => {
      wheel.onpointermove = null;
    };
  };
  wheel.onkeydown = (e) => {
    const k = e.key;
    let used = true;
    if (k === "ArrowLeft") H = (H - 4 + 360) % 360;
    else if (k === "ArrowRight") H = (H + 4) % 360;
    else if (k === "ArrowUp") SA = Math.min(1, SA + 0.05);
    else if (k === "ArrowDown") SA = Math.max(0, SA - 0.05);
    else used = false;
    if (used) {
      e.preventDefault();
      fromHsv();
    }
  };
  range.oninput = () => {
    V = range.value / 100;
    fromHsv();
  };
  hex.oninput = () => {
    const m = hex.value.trim().replace(/^#?/, "#");
    if (/^#[0-9a-f]{6}$/i.test(m)) {
      color = m.toLowerCase();
      [H, SA, V] = rgb2hsv(hex2rgb(color));
      V = Math.max(V, 0.2);
      sync(true);
    }
  };
  hex.onblur = () => {
    hex.value = color;
  };

  const pside = el("div", "pside");
  const shRow = el("div", "shade-row");
  shRow.append(el("span", "label", "Brightness"), range);
  pside.append(sw, shRow);
  const picker = el("div", "picker");
  picker.append(wheel, pside);
  const hexRow = el("div", "row2");
  hex.style.maxWidth = "120px";
  hexRow.append(hex);

  const iconTools = el("div", "row2");
  iconTools.append(iconSearch, uploadBtn);
  const body = [
    nrow,
    field("Icon", iconTools, grid),
    field("Color", picker, hexRow),
  ];
  let prof, np;
  if (add) {
    let chosen = sp.profile_id;
    const chips = el("div", "chips");
    const paintChips = () => {
      chips.replaceChildren(
        ...S.profiles.map((p) => {
          const c = el(
            "button",
            "chip" + (p.id === chosen && !np.value ? " on" : ""),
            p.name,
          );
          c.type = "button";
          c.onclick = () => {
            chosen = p.id;
            np.value = "";
            paintChips();
          };
          return c;
        }),
      );
    };
    np = el("input", "inp");
    np.placeholder = "Or start a new profile, like Work";
    np.maxLength = 40;
    np.oninput = paintChips;
    prof = {
      get value() {
        return chosen;
      },
    };
    paintChips();
    body.push(
      field(
        "Profile",
        chips,
        np,
        el(
          "div",
          "note",
          "A profile keeps its own logins, cookies and extensions. Spaces in the same profile share favorites.",
        ),
      ),
    );
  }
  const save = () => {
    const b = { name: name.value.trim() || "Space", icon: icn, color };
    if (add)
      go({
        cmd: "add_space",
        ...b,
        profile_id: prof.value,
        new_profile: np.value,
      });
    else {
      send({ cmd: "edit_space", id: S.current, ...b });
      closeView();
    }
  };
  const foot = [];
  if (!add && S.spaces.length > 1) {
    const d = btn("ghost danger", "Delete space");
    let sure = false;
    d.onclick = () => {
      if (!sure) {
        sure = true;
        d.textContent = "Click again to confirm";
        d.classList.add("sure");
        setTimeout(() => {
          sure = false;
          d.textContent = "Delete space";
          d.classList.remove("sure");
        }, 3000);
      } else go({ cmd: "delete_space", id: S.current });
    };
    foot.push(d);
  }
  foot.push(
    el("span", "grow"),
    btn("ghost", "Cancel", closeView),
    btn("primary", add ? "Create space" : "Save", save),
  );
  dialog(v, add ? "New space" : "Edit space", body, foot);
  name.onkeydown = (e) => {
    if (e.key === "Enter") save();
  };
  paintIcons();
  sync();
  setTimeout(() => name.focus(), 30);
}
