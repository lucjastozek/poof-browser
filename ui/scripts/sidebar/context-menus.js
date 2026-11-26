// Right-clicking open sidebar space offers folder creation without doing it immediately.
const folderContextMenu = el("div", "folder-context-menu");
const createFolderItem = el("button", "folder-context-item", "New folder");
createFolderItem.type = "button";
createFolderItem.onclick = () => {
  folderContextMenu.remove();
  send({ cmd: "create_folder", name: "New folder" });
};
const createLiveFolderItem = el("button", "folder-context-item", "New live folder");
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
      item("Open in split view (side by side)", {
        cmd: "split_tab",
        id: t.id,
        stacked: false,
      }),
      item("Open in split view (top and bottom)", {
        cmd: "split_tab",
        id: t.id,
        stacked: true,
      }),
    );
  }
  if (!items.length) return;
  rowMenu.replaceChildren(...items);
  $("side").append(rowMenu);
  const bounds = $("side").getBoundingClientRect();
  rowMenu.style.left =
    Math.max(8, Math.min(e.clientX - bounds.left, bounds.width - 220)) + "px";
  rowMenu.style.top = Math.max(8, e.clientY - bounds.top) + "px";
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
  $("side").append(folderContextMenu);
  const bounds = $("side").getBoundingClientRect();
  folderContextMenu.style.left =
    Math.max(8, Math.min(e.clientX - bounds.left, bounds.width - 168)) + "px";
  folderContextMenu.style.top = Math.max(8, e.clientY - bounds.top) + "px";
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
