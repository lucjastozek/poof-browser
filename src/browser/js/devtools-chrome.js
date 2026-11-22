(cfg => {
let ui = window.__poofDevtoolsUi;
if (!cfg) {
    if (ui) ui.handle.style.display = ui.header.style.display = "none";
    return;
}
if (!ui) {
    const post = msg => window.ipc.postMessage(JSON.stringify(msg));
    ui = window.__poofDevtoolsUi = { cfg };

    // Rasterize the browser's own icon set (ic) into mask-ready data URIs, once, and hand
    // them to the native side, which passes them on to the DevTools frontend.
    const sendIcons = () => {
        if (typeof ic !== "function") return;
        // DevTools icon name -> candidate names in icons.js (first one that exists wins).
        // Add or rename entries to match your icon set.
        const MAP = {
            cross: ["x"], "cross-circle": ["x-circle", "x"], "cross-circle-filled": ["x-circle", "x"],
            close: ["x"], plus: ["plus"], minus: ["minus"], bin: ["trash"], clear: ["trash"],
            gear: ["settings"], "gear-filled": ["settings"], search: ["search"],
            refresh: ["reload"], copy: ["copy"], download: ["download"], upload: ["upload"],
            filter: ["filter"], info: ["info"], warning: ["alert", "warning"],
            "warning-filled": ["alert", "warning"], check: ["check"], checkmark: ["check"],
            "arrow-up": ["arrow-up"], "arrow-down": ["arrow-down"],
            "arrow-left": ["back", "arrow-left"], "arrow-right": ["fwd", "arrow-right"],
            "chevron-up": ["chevron-up"], "chevron-down": ["chevron-down"],
            "chevron-left": ["chevron-left"], "chevron-right": ["chevron-right"],
            "triangle-right": ["chevron-right"], "triangle-down": ["chevron-down"],
            "triangle-left": ["chevron-left"], "triangle-up": ["chevron-up"],
            "dots-horizontal": ["more", "dots"], "dots-vertical": ["more-vertical", "more"],
            play: ["play"], resume: ["play"], pause: ["pause"], stop: ["stop"],
            eye: ["eye"], "eye-crossed": ["eye-off"], bookmark: ["bookmark"], star: ["star"],
            code: ["code"], devices: ["devices", "smartphone"], inspect: ["cursor"],
            undo: ["undo"], redo: ["redo"], document: ["file"], folder: ["folder"],
            "folder-open": ["folder-open"], link: ["link"], edit: ["pencil"], pen: ["pencil"],
            lock: ["lock"], bug: ["bug"], clock: ["clock"], history: ["history"],
            "left-panel-open": ["panelLeftOpen"], "left-panel-close": ["panelLeftClose"],
            puzzle: ["puzzle"],
            // main toolbar and panel tabs
            "select-element": ["cursor", "pointer", "inspect"],
            devices: ["devices", "smartphone", "phone", "device"],
            terminal: ["terminal", "terminal-box", "command", "code"],
            console: ["terminal", "terminal-box"],
            elements: ["code"], sources: ["bug", "file-code", "code"],
            policy: ["shield", "safe-shield", "security", "lock"],
            security: ["shield", "safe-shield", "lock"],
            cpu: ["cpu", "chip"], memory: ["cpu", "chip"], performance: ["dashboard", "speed", "gauge"],
            speed: ["dashboard", "speed", "gauge"], gauge: ["dashboard", "speed", "gauge"],
            network: ["wifi", "network"], wifi: ["wifi", "network"],
            lighthouse: ["lighthouse", "flashlight", "lamp", "light"],
            application: ["layout", "app", "window"], storage: ["database", "storage"],
            database: ["database"], cookie: ["cookie"], frame: ["layout", "window"],
            "more-tabs": ["plus"], "plus-small": ["plus"], "layers": ["layers"],
            "paint-bucket": ["paint", "palette"], "color-picker": ["color-picker", "eyedropper"],
            "bracket-curly": ["code", "braces"], "braces": ["code"], "record": ["record", "circle"],
            "lightbulb": ["lightbulb", "bulb", "lamp"], "tune": ["settings", "sliders"],
            "help": ["help", "question"], "feedback": ["chat", "message"],
        };
        const out = {};
        for (const [name, candidates] of Object.entries(MAP)) {
            for (const c of candidates.flatMap(x => [x, x + "-line"])) {
                let node = null;
                try { node = ic(c, 24); } catch (_) {}
                const svg = node && (node.localName === "svg" ? node : node.querySelector && node.querySelector("svg"));
                if (!svg || !svg.childElementCount) continue;
                const clone = svg.cloneNode(true);
                clone.setAttribute("xmlns", "http://www.w3.org/2000/svg");
                clone.setAttribute("width", "24");
                clone.setAttribute("height", "24");
                if (!clone.getAttribute("viewBox")) clone.setAttribute("viewBox", "0 0 24 24");
                const markup = new XMLSerializer().serializeToString(clone).replace(/currentColor/g, "#000");
                const uri = "data:image/svg+xml," + encodeURIComponent(markup);
                if (uri.length <= 12000) { out[name] = uri; break; }
            }
        }
        if (Object.keys(out).length) post({ cmd: "dev_tools_icons", icons: out });
    };
    try { sendIcons(); } catch (_) {}

    // Styled with the sidebar's own design tokens so it always matches the rest of the browser.
    const style = document.createElement("style");
    style.textContent = `
    .poof-dt-header{position:fixed;z-index:2147483647;display:none;align-items:center;gap:8px;
        box-sizing:border-box;padding:0 4px 0 10px;
        background:var(--chrome);color:var(--ink-2);
        border-bottom:1px solid var(--line);
        font:600 12px var(--f-display);letter-spacing:-.005em;user-select:none;border:none;}
    .poof-dt-header svg{width:14px;height:14px;flex:none}
    .poof-dt-title{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;background:var(--chrome);}
    .poof-dt-close{width:24px;height:24px;padding:0;border:0;border-radius:var(--r-sm);
        display:inline-flex;align-items:center;justify-content:center;background:transparent;
        color:var(--ink-2);cursor:pointer;transition:background .12s,color .12s}
    .poof-dt-close:hover{background:var(--hover);color:var(--ink)}
    .poof-dt-close:active{background:var(--pressed)}
    .poof-dt-handle{position:fixed;z-index:2147483647;display:none;touch-action:none;user-select:none;background:var(--chrome);}
    .poof-dt-handle i{position:absolute;border-radius:2px;background:transparent;transition:background .12s}
    .poof-dt-handle:hover i,.poof-dt-handle.on i{background:var(--space)}`;
    document.head.appendChild(style);

    const icon = (name, size) => {
    try { return typeof ic === "function" ? ic(name, size) : null; } catch (_) { return null; }
    };

    const handle = document.createElement("div");
    handle.className = "poof-dt-handle";
    const line = document.createElement("i");
    handle.appendChild(line);

    let drag = null, raf = 0, last = 0;
    const send = (size, persist) => post({ cmd: "dev_tools_size", size, persist });
    const sizeAt = e => {
    const [cx, cy, cw, ch] = ui.cfg.box, gap = ui.cfg.gap;
    switch (ui.cfg.position) {
        case "left": return e.clientX - cx - gap / 2;
        case "top": return e.clientY - cy - gap / 2;
        case "bottom": return cy + ch - e.clientY - gap / 2;
        default: return cx + cw - e.clientX - gap / 2;
    }
    };
    handle.addEventListener("pointerdown", e => {
    if (e.button !== 0) return;
    drag = e.pointerId;
    handle.setPointerCapture(e.pointerId);
    handle.classList.add("on");
    e.preventDefault();
    });
    handle.addEventListener("pointermove", e => {
    if (drag !== e.pointerId) return;
    last = sizeAt(e);
    if (!raf) raf = requestAnimationFrame(() => { raf = 0; send(last, false); });
    });
    const end = e => {
    if (drag !== e.pointerId) return;
    drag = null;
    if (raf) { cancelAnimationFrame(raf); raf = 0; }
    send(sizeAt(e), true);
    handle.classList.remove("on");
    };
    handle.addEventListener("pointerup", end);
    handle.addEventListener("pointercancel", end);

    const header = document.createElement("div");
    header.className = "poof-dt-header";
    const title = document.createElement("span");
    title.className = "poof-dt-title";
    title.textContent = "Developer Tools";
    const close = document.createElement("button");
    close.type = "button";
    close.className = "poof-dt-close";
    close.title = "Close Developer Tools (F12)";
    close.setAttribute("aria-label", "Close Developer Tools");
    const x = icon("x", 14);
    if (x) close.appendChild(x); else close.textContent = "×";
    close.addEventListener("click", () => post({ cmd: "dev_tools_toggle" }));
    const lead = icon("code", 14);
    if (lead) header.append(lead);
    header.append(title, close);

    document.body.append(handle, header);
    Object.assign(ui, { handle, line, header, title });
}
ui.cfg = cfg;

const place = (el, b) => Object.assign(el.style, {
    display: "block", left: b[0] + "px", top: b[1] + "px", width: b[2] + "px", height: b[3] + "px",
});
place(ui.header, cfg.header);
ui.header.style.display = "flex";

if (!cfg.handle) { ui.handle.style.display = "none"; return; }
const vertical = cfg.position === "left" || cfg.position === "right";
place(ui.handle, cfg.handle);
ui.handle.style.cursor = vertical ? "col-resize" : "row-resize";
Object.assign(ui.line.style, vertical
    ? { left: "50%", top: "0", width: "2px", height: "100%", transform: "translateX(-50%)" }
    : { top: "50%", left: "0", height: "2px", width: "100%", transform: "translateY(-50%)" });
})
