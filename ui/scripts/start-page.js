/* ---------- start page ---------- */
      const looksUrl = (q) =>
        /^https?:\/\//i.test(q) ||
        /^localhost/.test(q) ||
        /^[\w-]+(\.[\w-]+)+(\/\S*)?$/.test(q);
      window.__openNew = () => openView("newtab");
      function siteTile(s) {
        const d = el("div", "site");
        d.append(
          icon(s, true),
          el(
            "span",
            "n",
            s.label.replace(/\.(com|org|net|io|dev|app|co)$/, ""),
          ),
        );
        d.title = s.url;
        d.onclick = () => go({ cmd: "new_tab", input: s.url });
        return d;
      }
      function buildNewTab(v) {
        const wrap = el("div", "center"),
          box = el("div", "w nt");
        const mark = el("div", "mark", "poof");
        mark.append(el("i"));
        box.append(mark);
        const f = el("div", "field");
        f.append(ic("search", 18));
        const q = el("input");
        q.placeholder = "Search or enter an address";
        q.spellcheck = false;
        q.setAttribute("aria-label", "Search or enter an address");
        f.append(q);
        box.append(f);
        const body = el("div");
        box.append(body);
        wrap.append(box);
        v.replaceChildren(wrap);
        let items = [],
          sel = 0;
        const run = (it) => it && it.run();
        function showHome() {
          body.replaceChildren();
          items = [];
          const list = [
            ...S.suggestions.filter((x) => !x.popular),
            ...S.suggestions.filter((x) => x.popular),
          ].slice(0, 8);
          const g = el("div", "sites");
          list.forEach((x) => g.append(siteTile(x)));
          body.append(g);
        }
        function showResults(txt) {
          const ql = txt.toLowerCase();
          items = [];
          items.push({
            ic: looksUrl(txt) ? "external" : "search",
            t: looksUrl(txt) ? "Go to " + txt : "Search for “" + txt + "”",
            sub: "",
            run: () => go({ cmd: "new_tab", input: txt }),
          });
          [...S.favorites, ...S.pinned, ...S.tabs]
            .filter((t) =>
              ((t.title || "") + " " + t.url).toLowerCase().includes(ql),
            )
            .slice(0, 4)
            .forEach((t) =>
              items.push({
                fav: t,
                t: t.title || pretty(t.url),
                sub: "Switch to tab",
                run: () => go({ cmd: "select_tab", id: t.id }),
              }),
            );
          S.suggestions
            .filter((s) => (s.label + " " + s.url).toLowerCase().includes(ql))
            .slice(0, 5)
            .forEach((s) =>
              items.push({
                fav: s,
                t: s.label,
                sub: s.url.replace(/^https?:\/\//, "").replace(/\/$/, ""),
                run: () => go({ cmd: "new_tab", input: s.url }),
              }),
            );
          sel = 0;
          paint();
        }
        function paint() {
          const list = el("div", "res");
          items.forEach((it, i) => {
            const r = el("div", "row" + (i === sel ? " sel" : ""));
            if (it.ic) {
              const w = el("span", "ico");
              w.append(ic(it.ic, 14));
              r.append(w);
            } else r.append(icon(it.fav, true));
            r.append(el("span", "ttl", it.t), el("span", "sub", it.sub));
            if (i === sel) r.append(el("kbd", null, "Enter"));
            r.onclick = () => run(it);
            r.onmouseenter = () => {
              if (sel !== i) {
                sel = i;
                [...list.children].forEach((c, j) =>
                  c.classList.toggle("sel", j === i),
                );
              }
            };
            list.append(r);
          });
          body.replaceChildren(list);
        }
        q.addEventListener("input", () => {
          q.value.trim() ? showResults(q.value.trim()) : showHome();
        });
        q.addEventListener("keydown", (e) => {
          if (e.key === "ArrowDown" && items.length) {
            e.preventDefault();
            sel = (sel + 1) % items.length;
            paint();
          } else if (e.key === "ArrowUp" && items.length) {
            e.preventDefault();
            sel = (sel - 1 + items.length) % items.length;
            paint();
          } else if (e.key === "Enter" && q.value.trim()) run(items[sel]);
        });
        showHome();
        setTimeout(() => q.focus(), 30);
      }

      