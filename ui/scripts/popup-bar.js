/* Popup window title bar: theme and space color come from the host, commands go back over IPC. */
{
  const init = window.__popupInit || {};
  const send = (cmd) => {
    try {
      window.ipc.postMessage(JSON.stringify({ cmd }));
    } catch (_) {}
  };
  applyMode(init.theme || "system");
  if (init.space) applyColor(init.space);
  const title = document.getElementById("popupTitle");
  window.__popupTitle = (text) => {
    title.textContent = text || "";
  };
  window.__popupTitle(init.title);
  document.getElementById("winMin").onclick = () => send("min");
  document.getElementById("winMax").onclick = () => send("max");
  document.getElementById("winClose").onclick = () => send("close");
  const bar = document.getElementById("titlebar");
  bar.addEventListener("dblclick", (e) => {
    if (!e.target.closest(".window-controls")) send("max");
  });
  bar.addEventListener("pointerdown", (e) => {
    if (e.button === 0 && !e.target.closest(".window-controls")) send("drag");
  });
}
