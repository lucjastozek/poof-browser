const settingsButton = ib("settings-3", "Settings", () => openView("settings"));
const spaces = el("div");
spaces.id = "dots";
const newSpaceButton = ib("add", "New space", () => openView("space:add"));
$("foot").append(settingsButton, spaces, newSpaceButton);
const clearTabsBtn = el("button", "clear-tabs", "Clear");
clearTabsBtn.type = "button";
clearTabsBtn.title = "Close all unpinned tabs";
clearTabsBtn.onpointerdown = (e) => e.stopPropagation();
clearTabsBtn.onclick = (e) => {
  e.stopPropagation();
  window.__clearTabs();
};
{
  const line = $("line");
  if (getComputedStyle(line).position === "static")
    line.style.position = "relative";
  line.append(clearTabsBtn);
  const st = document.createElement("style");
  st.textContent = `
  #line .clear-tabs{position:absolute;right:4px;top:50%;transform:translateY(-50%);
    padding:2px 8px;border:0;border-radius:var(--r-sm);background:var(--chrome);
    color:var(--ink-2);font:600 11px var(--f-display);cursor:pointer;
    opacity:0;pointer-events:none;transition:opacity .12s,background .12s,color .12s}
  #line:hover .clear-tabs{opacity:1;pointer-events:auto}
  #line .clear-tabs:hover{background:var(--hover);color:var(--ink)}
  #line .clear-tabs:active{background:var(--pressed)}
  #line .clear-tabs[hidden],body.dragging #line .clear-tabs{display:none}`;
  document.head.append(st);
}
$("hdr").onclick = () => openView("space:edit");
