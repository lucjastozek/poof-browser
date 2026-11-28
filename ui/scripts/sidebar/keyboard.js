/* ---------- keyboard, focus and screen-reader layer ----------
   The tab list is ONE tab stop (roving tabindex). Inside it:
     Up/Down/Home/End move, Left/Right on tiles and folders, Enter/Space open, Delete closes,
     Shift+F10 / Menu key opens the menu, F2 renames a folder, Alt+Up/Down (Alt+Left/Right on
     favorites) reorders, and typing jumps to a tab. Escape returns to the page.
   Also here: live announcements, focus trapping and return for overlays, and labels for
   icon-only buttons (so Voice Control and screen readers can name them). */
(() => {
  const ITEM =
    "#favs .tile, #pinned .row, #pinned .folder-head, #pinned .live-item, #tabs .row, #tabs .folder-head, #tabs .live-item";
  const side = $("side");

  /* ----- live region ----- */
  const live = el("div", "sr-only");
  live.id = "sr-live";
  live.setAttribute("aria-live", "polite");
  live.setAttribute("aria-atomic", "true");
  document.body.append(live);
  let lastMsg = "",
    clearTimer = 0;
  window.__announce = (msg) => {
    if (!msg) return;
    clearTimeout(clearTimer);
    // Repeating the same text would not be read again, so alternate an invisible suffix.
    live.textContent = msg === lastMsg ? msg + "\u00a0" : msg;
    lastMsg = live.textContent;
    clearTimer = setTimeout(() => {
      live.textContent = "";
      lastMsg = "";
    }, 6000);
  };

  /* ----- landmarks ----- */
  side.setAttribute("role", "navigation");
  side.setAttribute("aria-label", "Tabs and spaces");
  $("pageToolbar").setAttribute("role", "toolbar");
  $("pageToolbar").setAttribute("aria-label", "Page controls");
  $("sidebarToolbar").setAttribute("role", "toolbar");
  $("sidebarToolbar").setAttribute("aria-label", "Sidebar controls");
  $("url").setAttribute("aria-label", "Address and search bar");
  [
    ["favs", "Favorites"],
    ["pinned", "Pinned tabs"],
    ["tabs", "Tabs"],
  ].forEach(([id, label]) => {
    const n = $(id);
    n.setAttribute("role", "tree");
    n.setAttribute("aria-label", label);
    n.setAttribute("aria-orientation", "vertical");
  });

  /* ----- helpers ----- */
  const real = (n) => !n.closest("#swipe");
  const navItems = () =>
    [...document.querySelectorAll(ITEM)].filter(
      (n) =>
        real(n) &&
        (n.classList.contains("folder-head") ||
          !n.closest(".tab-folder.collapsed")),
    );
  const keyOf = (n) =>
    n.dataset.id
      ? "t:" + n.dataset.id
      : n.dataset.folderId
        ? "f:" + n.dataset.folderId
        : "l:" + (n.querySelector(".ttl")?.textContent || "");
  const itemOf = (n) => (n && n.closest ? n.closest(ITEM) : null);
  const findTab = (id) =>
    [...S.favorites, ...S.pinned, ...S.tabs].find((t) => t.id === id);
  const nameOf = (n) =>
    n.getAttribute("aria-label") || n.textContent.trim() || "item";
  window.__navKeyOf = (n) => {
    const it = itemOf(n);
    return it ? keyOf(it) : null;
  };
  window.__focusNavKey = (key) => {
    const it = navItems().find((n) => keyOf(n) === key);
    if (!it) return false;
    it.focus({ preventScroll: true });
    return true;
  };

  /* ----- roles, names and the single tab stop ----- */
  let roving = null; // key of the item that holds tabindex 0
  function setRoving(it) {
    roving = keyOf(it);
    navItems().forEach((n) => (n.tabIndex = n === it ? 0 : -1));
  }
  function decorate() {
    if (!S) return;
    const items = navItems();
    for (const n of items) {
      n.setAttribute("role", "treeitem");
      n.tabIndex = -1;
      n.querySelectorAll(".acts button, .folder-actions button").forEach(
        (b) => (b.tabIndex = -1),
      );
      if (n.dataset.id) {
        const t = findTab(n.dataset.id) || {};
        const title = t.title || pretty(t.url) || "New tab";
        const fav = n.classList.contains("tile");
        n.setAttribute(
          "aria-label",
          title +
            (fav ? ", favorite" : "") +
            (t.loaded === false ? ", asleep" : ""),
        );
        n.setAttribute("aria-selected", String(S.active === n.dataset.id));
        n.setAttribute("aria-level", n.closest(".folder-contents") ? "2" : "1");
      } else if (n.classList.contains("folder-head")) {
        const name = n.querySelector(".folder-name,.folder-rename");
        n.setAttribute(
          "aria-label",
          (name && (name.value || name.textContent)) || "Folder",
        );
        n.setAttribute("aria-level", "1");
      } else n.setAttribute("aria-level", "2");
    }
    document
      .querySelectorAll(".folder-contents")
      .forEach((c) => c.setAttribute("role", "group"));
    document
      .querySelectorAll(".tab-folder, .split-pair")
      .forEach((c) => c.setAttribute("role", "none"));
    const keep =
      items.find((n) => keyOf(n) === roving) ||
      items.find((n) => n.classList.contains("on")) ||
      items[0];
    if (keep) setRoving(keep);
  }
  side.addEventListener("focusin", (e) => {
    const it = itemOf(e.target);
    if (it && it === e.target && real(it)) setRoving(it);
  });

  /* ----- keep focus through re-renders; announce what changed ----- */
  let movedNote = false;
  const base = window.__setState;
  window.__setState = (s) => {
    const prev = S;
    const cur = itemOf(document.activeElement);
    const had =
      cur && document.activeElement === cur
        ? { key: keyOf(cur), idx: navItems().indexOf(cur) }
        : null;
    base(s);
    decorate();
    if (had) {
      const items = navItems();
      const it =
        items.find((n) => keyOf(n) === had.key) ||
        items[Math.min(Math.max(had.idx, 0), items.length - 1)];
      if (it) {
        it.focus({ preventScroll: true });
        setRoving(it);
        if (movedNote && keyOf(it) === had.key) {
          movedNote = false;
          const group = items.filter(
            (n) =>
              n.closest("#favs,#pinned,#tabs") ===
                it.closest("#favs,#pinned,#tabs") &&
              !!n.closest(".folder-contents") ===
                !!it.closest(".folder-contents"),
          );
          window.__announce(
            nameOf(it) +
              ", position " +
              (group.indexOf(it) + 1) +
              " of " +
              group.length,
          );
        }
      }
    }
    if (prev && S && prev !== S) announceChanges(prev, S);
  };
  const flat = (st) => {
    const m = new Map();
    st.favorites.forEach((t) => m.set(t.id, ["fav", t]));
    st.pinned.forEach((t) => m.set(t.id, ["pin", t]));
    st.tabs.forEach((t) => m.set(t.id, ["tab", t]));
    return m;
  };
  const folderName = (st, id) =>
    (st.folders || []).find((f) => f.id === id)?.name || "folder";
  function announceChanges(a, b) {
    if (Date.now() < (window.__quietUntil || 0)) return; // an undo toast already said it
    if (a.current !== b.current) {
      const sp = b.spaces.find((x) => x.id === b.current);
      const n = b.pinned.length + b.tabs.length;
      return window.__announce(
        "Space " +
          (sp ? sp.name : "") +
          ", " +
          n +
          (n === 1 ? " tab" : " tabs"),
      );
    }
    const before = flat(a),
      after = flat(b);
    const said = [];
    const gone = [...before.keys()].filter((id) => !after.has(id));
    const born = [...after.keys()].filter((id) => !before.has(id));
    const titleOf = (t) => t.title || pretty(t.url) || "New tab";
    if (gone.length === 1)
      said.push("Closed " + titleOf(before.get(gone[0])[1]));
    else if (gone.length > 1) said.push("Closed " + gone.length + " tabs");
    if (born.length === 1 && !gone.length)
      said.push("Opened " + titleOf(after.get(born[0])[1]));
    for (const [id, [kind, t]] of after) {
      const old = before.get(id);
      if (!old) continue;
      if (old[0] !== kind) {
        said.push(
          kind === "pin"
            ? "Pinned " + titleOf(t)
            : kind === "fav"
              ? titleOf(t) + " added to favorites"
              : old[0] === "pin"
                ? "Unpinned " + titleOf(t)
                : titleOf(t) + " removed from favorites",
        );
      } else if ((old[1].folder_id || null) !== (t.folder_id || null)) {
        said.push(
          t.folder_id
            ? "Moved " + titleOf(t) + " to " + folderName(b, t.folder_id)
            : "Moved " + titleOf(t) + " out of folder",
        );
      } else if (old[1].loaded && t.loaded === false)
        said.push(titleOf(t) + " is asleep");
    }
    if (said.length) window.__announce(said.slice(0, 2).join(". "));
  }

  /* ----- reordering without a pointer ----- */
  const unitsOf = (list) => {
    const out = [];
    for (const c of list.children) {
      if (c.classList.contains("split-pair"))
        c.querySelectorAll(":scope > .row").forEach((r) => out.push(r));
      else out.push(c);
    }
    return out;
  };
  const edge = () => window.__announce("Can't move further");
  const mv = (cmd) => {
    movedNote = true;
    send(cmd);
  };
  function reorder(it, dir, horizontal) {
    if (it.classList.contains("tile")) {
      if (!horizontal) return;
      const ids = S.favorites.map((t) => t.id);
      const q = ids.indexOf(it.dataset.id) + dir;
      if (q < 0 || q >= ids.length) return edge();
      return mv({ cmd: "move_tab", id: it.dataset.id, to: "fav", index: q });
    }
    if (horizontal) return;
    if (it.classList.contains("live-item")) return;
    const listEl = it.closest("#pinned,#tabs");
    if (!listEl) return;
    const inPinned = listEl.id === "pinned";
    if (it.classList.contains("folder-head")) {
      const group = it.closest(".tab-folder");
      const units = unitsOf(listEl);
      const p = units.indexOf(group),
        q = p + dir;
      if (q < 0)
        return inPinned
          ? edge()
          : mv({
              cmd: "move_folder",
              id: it.dataset.folderId,
              to: "pinned",
              index: unitsOf($("pinned")).length,
            });
      if (q >= units.length)
        return inPinned
          ? mv({
              cmd: "move_folder",
              id: it.dataset.folderId,
              to: "tab",
              index: 0,
            })
          : edge();
      return mv({
        cmd: "move_folder",
        id: it.dataset.folderId,
        to: inPinned ? "pinned" : "tab",
        index: q,
      });
    }
    const inside = it.closest(".folder-contents");
    if (inside) {
      const group = it.closest(".tab-folder");
      const rows = [...inside.querySelectorAll(":scope > .row")];
      const q = rows.indexOf(it) + dir;
      if (q < 0 || q >= rows.length) return edge();
      return mv({
        cmd: "move_tab_to_folder",
        id: it.dataset.id,
        folder_id: group.dataset.folderId,
        index: q,
      });
    }
    const units = unitsOf(listEl);
    const p = units.indexOf(it),
      q = p + dir;
    if (q < 0)
      return inPinned
        ? edge()
        : mv({ cmd: "move_tab", id: it.dataset.id, to: "pinned", index: 9999 });
    if (q >= units.length)
      return inPinned
        ? mv({ cmd: "move_tab", id: it.dataset.id, to: "tab", index: 0 })
        : edge();
    const next = units[q];
    if (next.classList.contains("tab-folder")) {
      // The folder steps over the tab instead: its order is a position in the mixed list.
      return mv({
        cmd: "move_folder",
        id: next.dataset.folderId,
        to: inPinned ? "pinned" : "tab",
        index: p,
      });
    }
    const peers = (inPinned ? S.pinned.filter((t) => !t.folder_id) : S.tabs)
      .map((t) => t.id)
      .filter((id) => id !== it.dataset.id);
    const at = peers.indexOf(next.dataset.id);
    mv({
      cmd: "move_tab",
      id: it.dataset.id,
      to: inPinned ? "pinned" : "tab",
      index: dir > 0 ? at + 1 : at,
    });
  }

  /* ----- menus from the keyboard ----- */
  const folderMenu = el("div", "folder-context-menu");
  function showFolderMenu(e, folder, head) {
    e.preventDefault();
    e.stopPropagation();
    rowMenu.remove();
    folderContextMenu.remove();
    folderMenu.remove();
    const item = (label, fn, danger) => {
      const b = el(
        "button",
        "folder-context-item" + (danger ? " danger" : ""),
        label,
      );
      b.type = "button";
      b.onclick = () => {
        folderMenu.remove();
        fn();
      };
      return b;
    };
    const items = [];
    const name = head.querySelector(".folder-name");
    if (!folder.live && name)
      items.push(item("Rename", () => renameFolderInline(name, folder)));
    items.push(
      item(folder.collapsed ? "Expand" : "Collapse", () =>
        send({ cmd: "toggle_folder_collapsed", id: folder.id }),
      ),
      el("div", "folder-context-sep"),
      item(
        "Delete folder",
        () => send({ cmd: "delete_folder", id: folder.id }),
        true,
      ),
    );
    placeMenu(folderMenu, items, e);
  }
  const folderOf = (head) =>
    (S.folders || []).find((f) => f.id === head.dataset.folderId);
  function menuFor(it, at) {
    const r = it.getBoundingClientRect();
    const e = {
      preventDefault() {},
      stopPropagation() {},
      clientX: at ? at.x : r.left + 28,
      clientY: at ? at.y : r.bottom - 4,
    };
    if (it.classList.contains("folder-head")) {
      const f = folderOf(it);
      if (f) showFolderMenu(e, f, it);
    } else if (it.dataset.id) {
      const t = findTab(it.dataset.id);
      if (t) showRowMenu(e, t);
    }
  }
  side.addEventListener("contextmenu", (e) => {
    const head = e.target.closest && e.target.closest(".folder-head");
    if (head && real(head) && !e.target.closest(".folder-rename")) {
      const f = folderOf(head);
      if (f) showFolderMenu(e, f, head);
    }
  });
  document.addEventListener("pointerdown", (e) => {
    if (!folderMenu.contains(e.target)) folderMenu.remove();
  });
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") folderMenu.remove();
    menuKeys(folderMenu, e);
  });

  /* ----- the key handler ----- */
  let typed = "",
    typedTimer = 0;
  side.addEventListener("keydown", (e) => {
    const it = itemOf(e.target);
    if (!it || it !== e.target || !real(it)) return;
    const items = navItems();
    const i = items.indexOf(it);
    const focus = (n) => {
      if (!n) return;
      e.preventDefault();
      n.focus();
    };
    const tile = it.classList.contains("tile");
    const head = it.classList.contains("folder-head");
    const k = e.key;

    if (e.altKey && !e.ctrlKey && !e.metaKey && /^Arrow/.test(k)) {
      e.preventDefault();
      const horizontal = k === "ArrowLeft" || k === "ArrowRight";
      return reorder(
        it,
        k === "ArrowUp" || k === "ArrowLeft" ? -1 : 1,
        horizontal,
      );
    }
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    if ((k === "F10" && e.shiftKey) || k === "ContextMenu") {
      e.preventDefault();
      return menuFor(it);
    }
    if (e.shiftKey) return;

    if (k === "ArrowDown" || k === "ArrowUp") {
      const d = k === "ArrowDown" ? 1 : -1;
      if (tile) {
        const r = it.getBoundingClientRect();
        const tiles = items.filter((n) => n.classList.contains("tile"));
        const cand = tiles.filter((n) => {
          const b = n.getBoundingClientRect();
          return d > 0 ? b.top > r.bottom - 4 : b.bottom < r.top + 4;
        });
        if (cand.length) {
          const tops = cand.map((n) => n.getBoundingClientRect().top);
          const want = d > 0 ? Math.min(...tops) : Math.max(...tops);
          const rowOf = cand.filter(
            (n) => Math.abs(n.getBoundingClientRect().top - want) < 4,
          );
          const cx = (n) => n.getBoundingClientRect().left + n.offsetWidth / 2;
          return focus(
            rowOf.reduce((a, b) =>
              Math.abs(cx(b) - cx(it)) < Math.abs(cx(a) - cx(it)) ? b : a,
            ),
          );
        }
        if (d < 0) return e.preventDefault();
        return focus(items.find((n) => !n.classList.contains("tile")));
      }
      return focus(items[i + d] || (d < 0 ? null : null));
    }
    if (k === "Home") return focus(items[0]);
    if (k === "End") return focus(items[items.length - 1]);
    if (k === "ArrowRight" || k === "ArrowLeft") {
      const d = k === "ArrowRight" ? 1 : -1;
      if (tile)
        return focus(
          items[i + d] && items[i + d].classList.contains("tile")
            ? items[i + d]
            : null,
        );
      if (head) {
        const f = folderOf(it);
        if (!f) return;
        if (d > 0) {
          if (f.collapsed) {
            e.preventDefault();
            return toggleFolderNow(
              it.closest(".tab-folder"),
              it,
              it.querySelector(".folder-chevron"),
              it.querySelector(".folder-glyph"),
              f,
            );
          }
          return focus(
            items[i + 1] &&
              items[i + 1].closest(".tab-folder") === it.closest(".tab-folder")
              ? items[i + 1]
              : null,
          );
        }
        if (!f.collapsed) {
          e.preventDefault();
          return toggleFolderNow(
            it.closest(".tab-folder"),
            it,
            it.querySelector(".folder-chevron"),
            it.querySelector(".folder-glyph"),
            f,
          );
        }
        return;
      }
      if (d < 0 && it.closest(".folder-contents"))
        return focus(it.closest(".tab-folder").querySelector(".folder-head"));
      return;
    }
    if (k === "Enter" || k === " ") {
      if (head) return; // the folder handles these itself
      e.preventDefault();
      return it.click();
    }
    if (k === "Delete") {
      e.preventDefault();
      if (tile) send({ cmd: "unfav_tab", id: it.dataset.id });
      else if (it.dataset.id) window.__closeTab(it.dataset.id);
      return;
    }
    if (k === "F2" && head) {
      e.preventDefault();
      const f = folderOf(it);
      const name = it.querySelector(".folder-name");
      if (f && !f.live && name) renameFolderInline(name, f);
      return;
    }
    if (k === "Escape") {
      if (S && S.active) {
        e.preventDefault();
        send({ cmd: "focus_tab", id: S.active });
      }
      return;
    }
    if (k.length === 1 && /\S/.test(k)) {
      typed += k.toLowerCase();
      clearTimeout(typedTimer);
      typedTimer = setTimeout(() => (typed = ""), 700);
      const order = [
        ...items.slice(i + (typed.length > 1 ? 0 : 1)),
        ...items.slice(0, i + 1),
      ];
      const hit = order.find((n) => nameOf(n).toLowerCase().startsWith(typed));
      if (hit) focus(hit);
    }
  });

  /* Ctrl+Z undoes the last close, delete or removal (not while typing in a field). */
  document.addEventListener("keydown", (e) => {
    if (
      e.key.toLowerCase() !== "z" ||
      !(e.ctrlKey || e.metaKey) ||
      e.shiftKey ||
      e.altKey
    )
      return;
    const t = e.target;
    if (
      t &&
      (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName))
    )
      return;
    e.preventDefault();
    send({ cmd: "undo" });
  });

  /* F6 moves between the address bar, the tab list and the page. */
  document.addEventListener(
    "keydown",
    (e) => {
      if (e.key !== "F6" || e.ctrlKey || e.altKey) return;
      if (document.querySelector(".tabsw:not([hidden])")) return;
      e.preventDefault();
      const inUrl = document.activeElement === $("url");
      const inList = !!itemOf(document.activeElement);
      const list = () =>
        (
          navItems().find((n) => n.tabIndex === 0) ||
          navItems()[0] ||
          $("hdr")
        ).focus();
      if (e.shiftKey) {
        if (inUrl)
          S && S.active ? send({ cmd: "focus_tab", id: S.active }) : list();
        else if (inList) $("url").focus();
        else list();
      } else if (inUrl) list();
      else if (inList)
        S && S.active
          ? send({ cmd: "focus_tab", id: S.active })
          : $("url").focus();
      else $("url").focus();
    },
    true,
  );

  /* ----- menus give focus back to where they were opened ----- */
  new MutationObserver((records) => {
    for (const r of records)
      for (const n of r.removedNodes) {
        const from = n.__from;
        if (!from || n.isConnected) continue;
        n.__from = null;
        const active = document.activeElement;
        if (active && active !== document.body && !n.contains(active)) continue;
        const key = from.dataset ? window.__navKeyOf(from) : null;
        if (!(key && window.__focusNavKey(key)) && from.isConnected)
          from.focus({ preventScroll: true });
      }
  }).observe(side, { childList: true });

  /* ----- dialogs: role, first focus, Tab trap ----- */
  const FOCUSABLE =
    'button:not([disabled]),a[href],input:not([disabled]):not([type="hidden"]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex="-1"])';
  const focusables = (root) =>
    [...root.querySelectorAll(FOCUSABLE)].filter(
      (n) => n.getClientRects().length && !n.closest("[hidden]"),
    );
  const stage = $("stage");
  new MutationObserver(() => {
    const m = stage.querySelector(".dlg, .settings-modal");
    if (!m || m.__a11y) return;
    m.__a11y = true;
    m.setAttribute("role", "dialog");
    m.setAttribute("aria-modal", "true");
    if (!m.getAttribute("aria-label"))
      m.setAttribute(
        "aria-label",
        (m.querySelector("h2,h1") || {}).textContent || "Dialog",
      );
    requestAnimationFrame(() => {
      if (m.contains(document.activeElement)) return;
      const first =
        m.querySelector("[autofocus],input.inp,input,textarea") ||
        focusables(m)[0];
      first && first.focus({ preventScroll: true });
    });
  }).observe(stage, { childList: true, subtree: true });
  function trap(root, e) {
    if (e.key !== "Tab" || e.ctrlKey || e.altKey) return;
    const list = focusables(root);
    if (!list.length) return;
    const first = list[0],
      last = list[list.length - 1];
    const a = document.activeElement;
    if (e.shiftKey && (a === first || !root.contains(a))) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && (a === last || !root.contains(a))) {
      e.preventDefault();
      first.focus();
    }
  }
  document.addEventListener(
    "keydown",
    (e) => {
      const m = !stage.hidden && stage.querySelector(".dlg, .settings-modal");
      if (m && m.contains(document.activeElement)) trap(m, e);
      for (const id of PANELS) {
        const p = document.getElementById(id);
        if (p && !p.hidden && p.contains(document.activeElement)) trap(p, e);
      }
    },
    true,
  );

  /* ----- popover panels (downloads, history, passwords): name, focus in, focus back ----- */
  const PANELS = ["downloads-panel", "history-panel", "passwords-panel"];
  const PANEL_NAME = {
    "downloads-panel": "Downloads",
    "history-panel": "History",
    "passwords-panel": "Saved passwords",
  };
  const watched = new WeakSet();
  const returns = new Map();
  function panelState(p) {
    const open = p.isConnected && !p.hidden;
    if (open && !returns.has(p)) {
      const a = document.activeElement;
      returns.set(
        p,
        a && a !== document.body && !p.contains(a)
          ? { el: a, key: window.__navKeyOf(a) }
          : { el: null },
      );
      p.setAttribute("role", "dialog");
      p.setAttribute("aria-label", PANEL_NAME[p.id] || "Panel");
      requestAnimationFrame(() => {
        if (p.contains(document.activeElement)) return;
        const first = p.querySelector("input") || focusables(p)[0];
        first && first.focus({ preventScroll: true });
      });
    } else if (!open && returns.has(p)) {
      const r = returns.get(p);
      returns.delete(p);
      if (!document.hasFocus()) return;
      if (r.key && window.__focusNavKey(r.key)) return;
      if (r.el && r.el.isConnected) r.el.focus({ preventScroll: true });
      else if (S && S.active) send({ cmd: "focus_tab", id: S.active });
    }
  }
  const panelObs = new MutationObserver((rs) =>
    rs.forEach((r) => panelState(r.target)),
  );
  new MutationObserver(() => {
    for (const id of PANELS) {
      const p = document.getElementById(id);
      if (p && !watched.has(p)) {
        watched.add(p);
        panelObs.observe(p, { attributes: true, attributeFilter: ["hidden"] });
        panelState(p);
      }
    }
  }).observe(document.body, { childList: true });

  /* ----- names for icon-only controls (Voice Control says "click Close") ----- */
  const iconOnly = (b) => !b.textContent.trim();
  function nameIt(n) {
    if (n.matches("button,[role=button]") && iconOnly(n)) {
      const t = (n.getAttribute("title") || "").split("\n")[0];
      if (t && n.getAttribute("aria-label") !== t)
        n.setAttribute("aria-label", t);
    }
    if (n.matches("input:not([aria-label])") && n.placeholder)
      n.setAttribute("aria-label", n.placeholder);
  }
  const sweep = (root) => {
    if (root.nodeType !== 1) return;
    nameIt(root);
    root.querySelectorAll("button,[role=button],input").forEach(nameIt);
  };
  let queue = [],
    queued = false;
  new MutationObserver((records) => {
    for (const r of records) {
      if (r.type === "attributes") queue.push(r.target);
      else r.addedNodes.forEach((n) => queue.push(n));
    }
    if (queued) return;
    queued = true;
    requestAnimationFrame(() => {
      queued = false;
      const q = queue;
      queue = [];
      q.forEach((n) => n.isConnected && sweep(n));
    });
  }).observe(document.body, {
    childList: true,
    subtree: true,
    attributes: true,
    attributeFilter: ["title"],
  });
  sweep(document.body);
  decorate();
})();
