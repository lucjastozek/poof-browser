(cfg => {
let ui = window.__poofSplitUi;
if (!cfg) { if (ui) ui.handle.style.display = "none"; return; }
if (!ui) {
    const post = m => window.ipc.postMessage(JSON.stringify(m));
    const handle = document.createElement("div");
    handle.style.cssText = "position:fixed;z-index:10;touch-action:none;user-select:none;display:none;background:var(--chrome)";    
    const line = document.createElement("i");
    line.style.cssText = "position:absolute;border-radius:2px;background:transparent;transition:background .12s";
    handle.appendChild(line);
    let drag = null, raf = 0, last = 0;
    const ratioAt = e => {
        const [x, y, w, h] = ui.cfg.box, g = ui.cfg.gap;
        return ui.cfg.stacked ? (e.clientY - y - g / 2) / (h - g) : (e.clientX - x - g / 2) / (w - g);
    };
    const send = (ratio, persist) => post({ cmd: "split_ratio", ratio, persist });
    handle.addEventListener("pointerenter", () => { if (drag === null) line.style.background = "var(--space)"; });
    handle.addEventListener("pointerleave", () => { if (drag === null) line.style.background = "transparent"; });
    handle.addEventListener("pointerdown", e => {
        if (e.button !== 0) return;
        drag = e.pointerId; handle.setPointerCapture(drag);
        line.style.background = "var(--space)"; e.preventDefault();
    });
    handle.addEventListener("pointermove", e => {
        if (drag !== e.pointerId) return;
        last = ratioAt(e);
        if (!raf) raf = requestAnimationFrame(() => { raf = 0; send(last, false); });
    });
    const end = e => {
        if (drag !== e.pointerId) return;
        drag = null;
        if (raf) { cancelAnimationFrame(raf); raf = 0; }
        send(ratioAt(e), true);
        line.style.background = "transparent";
    };
    handle.addEventListener("pointerup", end);
    handle.addEventListener("pointercancel", end);
    document.body.append(handle);
    ui = window.__poofSplitUi = { cfg, handle, line };
}
ui.cfg = cfg;
const [x, y, w, h] = cfg.handle;
const pad = 8; // = the pane corner radius
const [hx, hy, hw, hh] = cfg.stacked
    ? [x, y - pad, w, h + 2 * pad]
    : [x - pad, y, w + 2 * pad, h];
Object.assign(ui.handle.style, { display: "block", left: hx + "px", top: hy + "px",
    width: hw + "px", height: hh + "px", cursor: cfg.stacked ? "row-resize" : "col-resize" });
Object.assign(ui.line.style, cfg.stacked
    ? { left: "0", right: "0", top: "50%", bottom: "auto", width: "auto", height: "2px", transform: "translateY(-50%)" }
    : { top: "0", bottom: "0", left: "50%", right: "auto", height: "auto", width: "2px", transform: "translateX(-50%)" });
})
