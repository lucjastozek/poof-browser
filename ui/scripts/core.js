
      const $ = (id) => document.getElementById(id);
      const send = (o) => window.ipc.postMessage(JSON.stringify(o));
      const el = (t, c, x) => {
        const e = document.createElement(t);
        if (c) e.className = c;
        if (x != null) e.textContent = x;
        return e;
      };
      const NS = "http://www.w3.org/2000/svg";
      const svg = (set, n, s = 16) => {
        const e = document.createElementNS(NS, "svg");
        [
          ["viewBox", "0 0 24 24"],
          ["width", s],
          ["height", s],
          ["fill", "none"],
          ["stroke", "currentColor"],
          ["stroke-width", "1.75"],
          ["stroke-linecap", "round"],
          ["stroke-linejoin", "round"],
          ["aria-hidden", "true"],
        ].forEach(([k, v]) => e.setAttribute(k, v));
        e.innerHTML = set[n] || "";
        return e;
      };
      const ic = (n, s) => svg(ICONS, n, s);
      const sic = (n, s) => {
        if (typeof n === "string" && n.startsWith("svg:")) {
          const i = el("img", "custom-space-icon");
          i.src = "data:image/svg+xml;base64," + n.slice(4);
          i.width = s;
          i.height = s;
          i.alt = "";
          return i;
        }
        return svg(SPACE_ICONS, SPACE_ICONS[n] ? n : "house", s);
      };
      const ib = (n, tip, fn) => {
        const b = el("button", "ib");
        b.title = tip;
        b.setAttribute("aria-label", tip);
        b.append(ic(n));
        b.onclick = (e) => {
          e.stopPropagation();
          fn();
        };
        return b;
      };
      const btn = (cls, txt, fn) => {
        const b = el("button", "btn " + cls, txt);
        if (fn) b.onclick = fn;
        return b;
      };

      /* ---------- colors ---------- */
      /* The twelve logo colors from the palette. They read well on both beige and near-black. */
      const PRESETS = [
        "#d64933",
        "#fa8334",
        "#ea9010",
        "#56c876",
        "#4fb561",
        "#6dc5e2",
        "#23b5d3",
        "#a288b1",
        "#8c4dc0",
        "#da4167",
        "#e87461",
        "#831f32",
      ];
      const hex2rgb = (h) =>
        [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));
      const rgb2hex = (c) =>
        "#" +
        c.map((v) => Math.round(v).toString(16).padStart(2, "0")).join("");
      function hsv2rgb(h, s, v) {
        const f = (n) => {
          const k = (n + h / 60) % 6;
          return (v - v * s * Math.max(0, Math.min(k, 4 - k, 1))) * 255;
        };
        return [f(5), f(3), f(1)];
      }
      function rgb2hsv([r, g, b]) {
        r /= 255;
        g /= 255;
        b /= 255;
        const mx = Math.max(r, g, b),
          mn = Math.min(r, g, b),
          d = mx - mn;
        let h = 0;
        if (d) {
          h =
            mx === r
              ? ((g - b) / d) % 6
              : mx === g
                ? (b - r) / d + 2
                : (r - g) / d + 4;
          h *= 60;
          if (h < 0) h += 360;
        }
        return [h, mx ? d / mx : 0, mx];
      }
      const lum = ([r, g, b]) => {
        const f = (c) => {
          c /= 255;
          return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
        };
        return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
      };
      const onColor = (h) => (lum(hex2rgb(h)) > 0.4 ? "#050505" : "#fffbe6");
      function applyColor(c) {
        const r = document.documentElement.style;
        r.setProperty("--space", c);
        r.setProperty("--on-space", onColor(c));
      }

      /* ---------- light / dark ---------- */
      const mq = matchMedia("(prefers-color-scheme: dark)");
      let pref = "system";
      function applyMode(p) {
        pref = p;
        document.documentElement.dataset.theme =
          p === "system" ? (mq.matches ? "dark" : "light") : p;
      }
      mq.addEventListener("change", () => applyMode(pref));
      applyMode("system");

      /* ---------- state ---------- */
      let S = null,
        prevIdx = -1,
        ov = null,
        EXTS = [],
        lastKey = "",
        pending = null,
        drag = null,
        noClick = false;
      const curSpace = () => S.spaces.find((x) => x.id === S.current);
      const letter = (t) =>
        ((t.title || t.label || t.url || "?").trim()[0] || "?").toUpperCase();
      function icon(t, guess) {
        const w = el("span", "ico");
        const fb = () => {
          w.textContent = letter(t);
        };
        let src = t.favicon;
        if (!src && guess) {
          try {
            src = new URL("/favicon.ico", t.url).href;
          } catch (e) {}
        }
        if (src) {
          const i = new Image();
          i.referrerPolicy = "no-referrer";
          i.draggable = false;
          i.onerror = () => {
            i.remove();
            fb();
          };
          i.src = src;
          w.appendChild(i);
        } else fb();
        return w;
      }
      const pretty = (u) => {
        try {
          const x = new URL(u);
          return x.host.replace(/^www\./, "");
        } catch (e) {
          return u || "";
        }
      };

      /* A command that changes what is shown: close any full-area view first. */
      function go(cmd) {
        if (ov) {
          ov = null;
          $("stage").hidden = true;
          send({ cmd: "overlay", open: false });
        }
        send(cmd);
      }
      function openView(name) {
        ov = name;
        $("stage").hidden = false;
        send({ cmd: "overlay", open: true });
        build(name);
      }
      function closeView() {
        applyColor(curSpace().color);
        if (!S || !S.active) {
          openView("newtab");
          return;
        }
        ov = null;
        $("stage").hidden = true;
        applyColor(curSpace().color);
        send({ cmd: "overlay", open: false });
      }

      