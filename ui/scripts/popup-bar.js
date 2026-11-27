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
  const mode = init.mode || "popup";
  const $id = (n) => document.getElementById(n);
  if (mode !== "popup") {
    $id("winExpand").hidden = false;
    $id("winExpand").onclick = () => send("expand");
  }
  document.body.classList.add("mode-" + mode);
  if (mode === "peek") {
    $id("winMin").hidden = true;
    $id("winMax").hidden = true;
  }
  if (mode === "mini") {
    const go = $id("barGo");
    go.hidden = false;
    title.hidden = true;
    go.focus();
    go.onkeydown = (e) => {
      if (e.key === "Enter" && go.value.trim()) send("go:" + go.value.trim());
    };
  }
  document.getElementById("winMin").onclick = () => send("min");
  document.getElementById("winMax").onclick = () => send("max");
  document.getElementById("winClose").onclick = () => send("close");
  const bar = document.getElementById("titlebar");
  bar.addEventListener("dblclick", (e) => {
    if (!e.target.closest(".window-controls,input") && mode === "popup")
      send("max");
  });
  bar.addEventListener("pointerdown", (e) => {
    if (e.button === 0 && !e.target.closest(".window-controls,input"))
      send("drag");
  });
}
