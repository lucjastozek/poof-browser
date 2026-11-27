// Right-clicking open sidebar space offers folder creation without doing it immediately.
const folderContextMenu = el("div", "folder-context-menu");
const createFolderItem = el("button", "folder-context-item", "New folder");
createFolderItem.type = "button";
createFolderItem.onclick = () => {
  folderContextMenu.remove();
  send({ cmd: "create_folder", name: "New folder" });
};
const createLiveFolderItem = el(
  "button",
  "folder-context-item",
  "New live folder",
);
createLiveFolderItem.type = "button";
createLiveFolderItem.onclick = () => {
  folderContextMenu.remove();
  openView("livefolder:add");
};
folderContextMenu.append(createFolderItem, createLiveFolderItem);

// Right-clicking a tab row: split view actions.
const rowMenu = el("div", "folder-context-menu");
function showRowMenu(e, t) {
  e.preventDefault();
  e.stopPropagation();
  folderContextMenu.remove();
  rowMenu.remove();
  const item = (label, cmd) => {
    const b = el("button", "folder-context-item", label);
    b.type = "button";
    b.onclick = () => {
      rowMenu.remove();
      send(cmd);
    };
    return b;
  };
  const pair = (S.splits || []).find((p) => p.a === t.id || p.b === t.id);
  const live = S.split && (S.split.a === t.id || S.split.b === t.id);
  const items = [];
  if (pair) {
    items.push(item("Close split view", { cmd: "split_close", id: t.id }));
    if (live)
      items.push(
        item(S.split.stacked ? "Side by side" : "Top and bottom", {
          cmd: "split_orientation",
        }),
      );
  } else if (S.active && S.active !== t.id) {
    items.push(
      item("Split side by side", {
        cmd: "split_tab",
        id: t.id,
        stacked: false,
      }),
      item("Split top and bottom", {
        cmd: "split_tab",
        id: t.id,
        stacked: true,
      }),
    );
  }
  const tail = [];
  if (t.url && /^https?:/i.test(t.url))
    tail.push(
      Object.assign(item("Copy link", { cmd: "noop" }), {
        onclick: () => {
          rowMenu.remove();
          window.__copyLink(t.url, "Link copied");
        },
      }),
    );
  if (t.kind !== "favorite" && !S.favorites.some((f) => f.id === t.id))
    tail.push(item("Add to favorites", { cmd: "fav_tab", id: t.id }));
  if (t.loaded)
    tail.push(item("Put to sleep", { cmd: "unload_tab", id: t.id }));
  const close = item("Close tab", { cmd: "close_tab", id: t.id });
  close.classList.add("danger");
  tail.push(close);
  if (items.length) items.push(el("div", "folder-context-sep"));
  items.push(...tail);
  placeMenu(rowMenu, items, e);
}
// Measures the menu after it is in the DOM so it never spills past the sidebar or window.
function placeMenu(menu, items, e) {
  menu.setAttribute("role", "menu");
  items.forEach(
    (i) => i.matches?.("button") && i.setAttribute("role", "menuitem"),
  );
  menu.replaceChildren(...items);
  menu.style.visibility = "hidden";
  $("side").append(menu);
  const b = $("side").getBoundingClientRect();
  const w = menu.offsetWidth,
    h = menu.offsetHeight;
  const x = Math.max(6, Math.min(e.clientX - b.left, b.width - w - 6));
  let y = e.clientY - b.top;
  if (y + h > b.height - 6) y = Math.max(6, y - h);
  menu.style.left = x + "px";
  menu.style.top = y + "px";
  menu.style.visibility = "";
  const first = menu.querySelector("button");
  if (first) first.focus({ preventScroll: true });
}
function menuKeys(menu, e) {
  if (!menu.isConnected) return;
  const btns = [...menu.querySelectorAll("button")];
  const i = btns.indexOf(document.activeElement);
  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    const d = e.key === "ArrowDown" ? 1 : -1;
    btns[(i + d + btns.length) % btns.length]?.focus();
  }
}

$("side").addEventListener("contextmenu", (e) => {
  if (
    e.target.closest(
      ".row,.tile,.tab-folder,.folder-actions,.folder-rename,input,button,a",
    )
  )
    return;
  e.preventDefault();
  rowMenu.remove();
  folderContextMenu.remove();
  placeMenu(folderContextMenu, [createFolderItem, createLiveFolderItem], e);
});
document.addEventListener("keydown", (e) => {
  menuKeys(rowMenu, e);
  menuKeys(folderContextMenu, e);
});
window.addEventListener("blur", () => {
  rowMenu.remove();
  folderContextMenu.remove();
});
document.addEventListener("pointerdown", (e) => {
  if (!folderContextMenu.contains(e.target)) folderContextMenu.remove();
  if (!rowMenu.contains(e.target)) rowMenu.remove();
});
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    folderContextMenu.remove();
    rowMenu.remove();
  }
});
