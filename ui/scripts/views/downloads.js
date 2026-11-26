/* ---------- downloads (Ctrl+J) ----------
   A toolbar button with a progress ring (shown once there is something to show) and a
   popover that floats over the page you are on (the page stays live, so a download can be dragged
   straight onto it). The native side pushes the complete list with
   `window.__setDownloads(rows)`: at most a few times a second while something downloads, and
   never otherwise. Uses the helpers from scripts/core (el, ib, ic, $, send, hostOf, dayLabel,
   formatBytes, formatTimeLeft), the global `ov` and `S`, and the tokens from
   styles/foundation/theme.css. */
(() => {
  const NS = "http://www.w3.org/2000/svg";
  const RING_RADIUS = 12;
  const RING_LENGTH = 2 * Math.PI * RING_RADIUS;
  const FILE_TYPES = {
    document: new Set("doc docm docx dot dotx odt ott pages rtf wpd wps".split(" ")),
    pdf: new Set("pdf xps oxps".split(" ")),
    spreadsheet: new Set("csv ods ots numbers tsv xls xlsb xlsm xlsx xlt xltm xltx".split(" ")),
    presentation: new Set("key odp otp pot potm potx pps ppsm ppsx ppt pptm pptx".split(" ")),
    image: new Set("ai apng avif bmp cdr dib emf eps gif heic heif ico jfif jif jpe jpeg jpg png psd raw svg tif tiff webp".split(" ")),
    video: new Set("3g2 3gp asf avi flv m2ts m2v m4v mkv mov mp4 mpe mpeg mpg mts ogv rm swf ts vob webm wmv".split(" ")),
    audio: new Set("aac aif aiff alac amr ape au dss flac m4a m4b mid midi mka mp3 mpc oga ogg opus ra ram wav wma".split(" ")),
    archive: new Set("7z ace alz arj bz2 cab cpio gz iso lha lz lzh lzma lzo rar sit sitx tar tbz tbz2 tgz txz xz z zip zst".split(" ")),
    code: new Set("asm bash c cc clj cljc cljs cmake cjs cpp cs css cxx dart diff dockerfile ex go h hpp hs htm html java js json jsx kt lua m mjs ml php pl py r rb rs sass scss sh sql svelte swift toml ts tsx vb vue wasm xml yaml yml".split(" ")),
    package: new Set("app apk appx bat bin cmd com deb dll dmg exe ipa jar msi pkg rpm so".split(" ")),
    font: new Set("eot fon otf pfb pfm ttc ttf woff woff2".split(" ")),
    database: new Set("accdb db db3 dbf mdb mdf ndjson sqlite sqlite3 sqlitedb".split(" ")),
    design: new Set("3ds blend c4d dae dcm dds dwg dxf fbx glb gltf max obj stl step stp".split(" ")),
    ebook: new Set("azw azw3 azw4 cbr cbz epub fb2 lit mobi pdb".split(" ")),
    email: new Set("eml emlx msg oft ost pst vcf".split(" ")),
    subtitle: new Set("ass srt ssa sub vtt".split(" ")),
    config: new Set("cfg conf editorconfig env gitignore ini lock log properties".split(" ")),
    certificate: new Set("cer crt der key p12 p7b p7c pem pfx".split(" ")),
    torrent: new Set(["torrent"]),
  };
  const FILE_ICONS = {
    document: "file-document",
    pdf: "file-pdf",
    spreadsheet: "file-spreadsheet",
    presentation: "file-presentation",
    image: "file-image",
    video: "file-video",
    audio: "file-audio",
    archive: "file-archive",
    code: "file-code",
    package: "file-package",
    font: "file-font",
    database: "file-database",
    design: "file-design",
    ebook: "file-ebook",
    email: "file-email",
    subtitle: "file-subtitle",
    config: "file-config",
    certificate: "file-certificate",
    torrent: "file-torrent",
  };
  const DRAG_START_PX = 6; // pointer travel with the button held before a drag starts
  const CLICK_SHIELD_MS = 400; // a click this soon after a drag is the drag ending, not a click
  /* Pause and play are not part of the generated icon set. */
  const EXTRA_ICONS = {
    pause:
      '<path fill="currentColor" d="M8 4.5h-1a1 1 0 0 0-1 1v13a1 1 0 0 0 1 1h1a1 1 0 0 0 1-1v-13a1 1 0 0 0-1-1Zm9 0h-1a1 1 0 0 0-1 1v13a1 1 0 0 0 1 1h1a1 1 0 0 0 1-1v-13a1 1 0 0 0-1-1Z"/>',
    play: '<path fill="currentColor" d="M8 5.6v12.8a1 1 0 0 0 1.52.85l10.2-6.4a1 1 0 0 0 0-1.7L9.52 4.75A1 1 0 0 0 8 5.6Z"/>',
  };
  const extraIcon = (name, size) => svg(EXTRA_ICONS, name, size);

  let rows = [];
  let shown = "";
  let panel = null;
  let list = null;
  let clearButton = null;
  let isOpen = false;
  let openedActive = null;
  let reported = "";
  let resizeWatch = null;
  let unseen = false;
  let known = null;
  let dragEndedAt = -CLICK_SHIELD_MS;
  const live = new Map();

  /* ----- toolbar button ----- */
  const button = el("button", "ib");
  button.id = "dlBtn";
  button.type = "button";
  button.hidden = true;
  const ring = document.createElementNS(NS, "svg");
  ring.setAttribute("class", "ring");
  ring.setAttribute("viewBox", "0 0 28 28");
  ring.setAttribute("aria-hidden", "true");
  const track = document.createElementNS(NS, "circle");
  const bar = document.createElementNS(NS, "circle");
  for (const [circle, cls] of [
    [track, "track"],
    [bar, "bar"],
  ]) {
    circle.setAttribute("class", cls);
    circle.setAttribute("cx", "14");
    circle.setAttribute("cy", "14");
    circle.setAttribute("r", String(RING_RADIUS));
    ring.append(circle);
  }
  bar.style.strokeDasharray = String(RING_LENGTH);
  bar.style.strokeDashoffset = String(RING_LENGTH);
  button.append(ring, ic("download", 16), el("span", "dot"));
  const toolbar = $("pageToolbar");
  toolbar.insertBefore(button, toolbar.querySelector(".urlwrap").nextSibling);
  button.onclick = () => toggle();

  const isLive = (d) => d.status === "active" || d.status === "paused";

  function updateButton() {
    const running = rows.filter(isLive);
    let received = 0;
    let total = 0;
    for (const d of running) {
      if (d.total > 0) {
        received += d.received;
        total += d.total;
      }
    }
    const fraction = total > 0 ? Math.min(1, received / total) : 0;
    button.hidden = rows.length === 0;
    button.classList.toggle("busy", running.length > 0);
    button.classList.toggle("indeterminate", running.length > 0 && total === 0);
    button.classList.toggle("unseen", unseen && !isOpen);
    bar.style.strokeDashoffset = String(RING_LENGTH * (1 - fraction));
    const label = running.length
      ? "Downloading " +
        running.length +
        (running.length === 1 ? " file" : " files") +
        (total > 0 ? " · " + Math.round(fraction * 100) + "%" : "")
      : "Downloads (Ctrl+J)";
    button.title = label;
    button.setAttribute("aria-label", label);
  }

  /* A finished download the user has not looked at yet gets a dot on the button. */
  function trackFinished() {
    const next = new Map(rows.map((d) => [d.id, d.status]));
    if (known) {
      for (const [id, status] of next) {
        const was = known.get(id);
        if (status === "completed" && (was === "active" || was === "paused"))
          unseen = true;
      }
    }
    known = next;
  }

  /* ----- rows ----- */
  function extBadge(name) {
    const base = name.split(/[\\/]/).pop() || "";
    const i = base.lastIndexOf(".");
    const ext = i > 0 ? base.slice(i + 1).toLowerCase() : "";
    const type = Object.keys(FILE_TYPES).find((key) => FILE_TYPES[key].has(ext));
    const box = el("span", "d-ico");
    box.append(ic(type ? FILE_ICONS[type] : "file-default", 16));
    return box;
  }

  function progressText(d) {
    const parts = [];
    parts.push(
      d.total > 0
        ? formatBytes(d.received) + " of " + formatBytes(d.total)
        : formatBytes(d.received) + " downloaded",
    );
    if (d.status === "paused") return "Paused · " + parts[0];
    if (d.speed > 0) {
      parts.push(formatBytes(d.speed) + "/s");
      if (d.total > 0) parts.push(formatTimeLeft((d.total - d.received) / d.speed));
    }
    return parts.filter(Boolean).join(" · ");
  }

  function metaText(d) {
    if (isLive(d)) return progressText(d);
    if (d.status === "completed") {
      if (d.missing) return "File moved or deleted";
      const from = /^https?:/i.test(d.url) ? hostOf(d.url) : "";
      return [formatBytes(d.total || d.received), from].filter(Boolean).join(" · ");
    }
    return d.status === "cancelled" ? "Cancelled" : "Failed";
  }

  function paintProgress(refs, d) {
    refs.meta.textContent = metaText(d);
    refs.meta.classList.toggle("err", d.status === "failed" || (d.status === "completed" && d.missing));
    if (!isLive(d)) return;
    const hasTotal = d.total > 0;
    refs.bar.classList.toggle("unknown", !hasTotal && d.status === "active");
    refs.bar.classList.toggle("paused", d.status === "paused");
    refs.fill.style.width = hasTotal
      ? Math.min(100, (d.received / d.total) * 100) + "%"
      : "";
  }

  function actionsFor(d) {
    const out = [];
    const tool = (icon, tip, cmd) => {
      const b = ib("x", tip, () => send({ cmd, id: d.id }));
      b.replaceChildren(typeof icon === "string" ? ic(icon) : icon);
      return b;
    };
    if (d.status === "active")
      out.push(tool(extraIcon("pause", 16), "Pause", "pause_download"));
    if (d.status === "paused")
      out.push(tool(extraIcon("play", 16), "Resume", "resume_download"));
    if (isLive(d)) out.push(tool("x", "Cancel download", "cancel_download"));
    else {
      if (d.status === "completed")
        out.push(tool("folder", "Show in folder", "show_download"));
      out.push(tool("x", "Remove from list", "remove_download"));
    }
    return out;
  }

  /* A finished download can be dragged out of the list onto a page (upload) or into another
     app. Native code runs the drag, which takes over the pointer until the button is released,
     so this only decides when it starts. The row's buttons keep working as buttons. */
  function armDrag(row, d) {
    row.onpointerdown = (e) => {
      if (e.button !== 0 || e.target.closest(".d-actions")) return;
      const x0 = e.clientX;
      const y0 = e.clientY;
      const stop = () => {
        document.removeEventListener("pointermove", move, true);
        document.removeEventListener("pointerup", stop, true);
        document.removeEventListener("pointercancel", stop, true);
      };
      const move = (m) => {
        if (!(m.buttons & 1)) return stop();
        if (Math.hypot(m.clientX - x0, m.clientY - y0) < DRAG_START_PX) return;
        stop();
        dragEndedAt = performance.now();
        send({ cmd: "drag_download", id: d.id });
      };
      document.addEventListener("pointermove", move, true);
      document.addEventListener("pointerup", stop, true);
      document.addEventListener("pointercancel", stop, true);
    };
    // No browser-style drag of the row's text or icon competing with the native drag.
    row.ondragstart = (e) => e.preventDefault();
  }

  function buildRow(d) {
    const row = el("div", "d-row" + (isLive(d) ? " live" : "") + (d.status === "completed" ? " done" : ""));
    row.tabIndex = 0;
    row.dataset.id = d.id;
    const name = el("div", "d-name", d.name);
    name.title = d.path || d.name;
    if (d.status === "completed" && d.missing) row.classList.add("gone");
    const meta = el("div", "d-meta");
    const fill = el("i");
    const barEl = el("div", "d-bar");
    barEl.append(fill);
    barEl.hidden = !isLive(d);
    const body = el("div", "d-body");
    body.append(name, meta, barEl);
    const actions = el("div", "d-actions");
    actions.append(...actionsFor(d));
    row.append(extBadge(d.name), body, actions);
    const refs = { meta, bar: barEl, fill };
    paintProgress(refs, d);
    if (d.status === "completed") {
      const open = () => {
        if (performance.now() - dragEndedAt < CLICK_SHIELD_MS) return;
        send({ cmd: "open_download", id: d.id });
      };
      row.onclick = open;
      if (!d.missing) armDrag(row, d);
      row.onkeydown = (e) => {
        if (e.key === "Enter" && e.target === row) {
          e.preventDefault();
          open();
        }
      };
    }
    live.set(d.id, refs);
    return row;
  }

  function render() {
    if (!list) return;
    list.replaceChildren();
    live.clear();
    clearButton.disabled = !rows.some((d) => !isLive(d));
    if (!rows.length) {
      list.append(el("div", "d-empty", "No downloads yet."));
      return;
    }
    let card = null;
    let last = "";
    for (const d of rows) {
      const label = dayLabel(d.ts);
      if (label !== last) {
        last = label;
        list.append(el("div", "d-day", label));
        card = el("div", "d-card");
        list.append(card);
      }
      card.append(buildRow(d));
    }
  }

  /* Same rows in the same states: only the numbers moved, so update them in place. */
  const signature = () => rows.map((d) => d.id + d.status + d.missing).join("|");

  window.__setDownloads = (incoming) => {
    rows = Array.isArray(incoming) ? incoming : [];
    trackFinished();
    updateButton();
    if (!isOpen) return;
    const now = signature();
    if (now !== shown) {
      shown = now;
      render();
      return;
    }
    for (const d of rows) {
      const refs = live.get(d.id);
      if (refs && isLive(d)) paintProgress(refs, d);
    }
  };

  /* ----- view ----- */
  function build() {
    if (panel) return;
    panel = el("div");
    panel.id = "downloads-panel";
    panel.hidden = true;
    panel.setAttribute("role", "dialog");
    panel.setAttribute("aria-label", "Downloads");
    clearButton = el("button", "btn sm", "Clear completed");
    clearButton.type = "button";
    clearButton.onclick = () => send({ cmd: "clear_downloads" });
    const tools = el("div", "d-tools");
    tools.append(clearButton, ib("x", "Close downloads (Esc)", () => close()));
    const head = el("div", "d-head");
    head.append(el("h1", null, "Downloads"), tools);
    const top = el("div", "d-top");
    const topWrap = el("div", "d-wrap");
    topWrap.append(head);
    top.append(topWrap);
    const scroll = el("div", "d-scroll");
    list = el("div", "d-wrap");
    scroll.append(list);
    panel.append(top, scroll);
    document.body.append(panel);
  }

  /* Native page views always draw above this one, so the popover can only show over the page
     by having the page cut a hole where it sits: tell native code the box, whenever it moves. */
  function report() {
    if (!isOpen || !panel) return;
    const r = panel.getBoundingClientRect();
    const key = [r.left, r.top, r.width, r.height].map(Math.round).join(",");
    if (key === reported) return;
    reported = key;
    send({ cmd: "downloads_panel", open: true, x: r.left, y: r.top, w: r.width, h: r.height });
  }

  function open() {
    build();
    openedActive = S ? S.active : null;
    unseen = false;
    panel.hidden = false;
    isOpen = true;
    shown = signature();
    render();
    updateButton();
    reported = "";
    report();
    if (window.ResizeObserver) {
      resizeWatch = new ResizeObserver(report);
      resizeWatch.observe(panel);
    }
    send({ cmd: "list_downloads" });
    panel.querySelector(".d-row, .btn, .ib")?.focus();
  }

  function close() {
    if (!isOpen) return;
    isOpen = false;
    panel.hidden = true;
    if (resizeWatch) resizeWatch.disconnect();
    resizeWatch = null;
    reported = "";
    updateButton();
    send({ cmd: "downloads_panel", open: false });
  }

  function toggle() {
    if (isOpen) {
      close();
      return;
    }
    // The history view is full-area and hides the page: swap it out first.
    const history = $("history-panel");
    if (history && !history.hidden && window.__toggleHistory) window.__toggleHistory();
    open();
  }
  window.__toggleDownloads = toggle;
  window.__closeDownloads = close;

  /* Opening history while this view is up replaces it. */
  const baseToggleHistory = window.__toggleHistory;
  if (baseToggleHistory) {
    window.__toggleHistory = () => {
      close();
      baseToggleHistory();
    };
  }

  // Switching tabs closes it; navigating inside the same tab does not (the page stays usable).
  const baseSetState = window.__setState;
  window.__setState = (s) => {
    baseSetState(s);
    if (isOpen && s.active !== openedActive) close();
  };
  // The popover's x depends on the window width.
  addEventListener("resize", report);

  // A press anywhere else in the browser chrome dismisses it (presses on the page never reach
  // this view). The toolbar button toggles it by itself.
  document.addEventListener(
    "pointerdown",
    (e) => {
      if (!isOpen || !e.target.closest) return;
      if (e.target.closest("#downloads-panel") || e.target.closest("#dlBtn")) return;
      close();
    },
    true,
  );
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && isOpen) {
      e.preventDefault();
      close();
    }
  });
})();
