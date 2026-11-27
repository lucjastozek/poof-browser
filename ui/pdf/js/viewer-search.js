/* Find in document: matches across all pages, highlighted on the rendered pages. */
"use strict";

const Find = {
  q: "",
  matchCase: false,
  flat: [],
  idx: -1,
  token: 0,
  scrollTo: 0,
  timer: 0,
  input: null,
  label: null,
};

function buildFindBar() {
  const bar = $("findbar");
  const input = el("input", "field");
  input.placeholder = "Find in document";
  input.spellcheck = false;
  input.setAttribute("aria-label", "Find in document");
  const label = el("span", "find-count muted");
  const aa = el("button", "ib txt", "Aa");
  aa.type = "button";
  aa.title = "Match case";
  aa.setAttribute("aria-pressed", "false");
  aa.onclick = () => {
    Find.matchCase = !Find.matchCase;
    aa.classList.toggle("on", Find.matchCase);
    aa.setAttribute("aria-pressed", String(Find.matchCase));
    runFind();
  };
  input.oninput = () => {
    clearTimeout(Find.timer);
    Find.timer = setTimeout(runFind, 160);
  };
  input.onkeydown = (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      stepFind(e.shiftKey ? -1 : 1);
    } else if (e.key === "Escape") closeFind();
  };
  bar.append(
    input,
    aa,
    label,
    iconBtn("up", "Previous match", () => stepFind(-1)),
    iconBtn("down", "Next match", () => stepFind(1)),
    iconBtn("close", "Close search", closeFind),
  );
  Find.input = input;
  Find.label = label;
}

function openFind() {
  if (!V.doc) return;
  if (!Find.input) buildFindBar();
  $("findbar").hidden = false;
  Find.input.focus();
  Find.input.select();
  if (Find.input.value) runFind();
}
function closeFind() {
  $("findbar").hidden = true;
  Find.token++;
  Find.flat = [];
  clearHighlights();
  scrollBox().focus();
}

function findRegex() {
  const esc = Find.q.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return new RegExp(esc, Find.matchCase ? "g" : "gi");
}
async function pageStrings(num) {
  if (V.textCache.has(num)) return V.textCache.get(num);
  const page = await V.doc.pdf.getPage(num);
  const content = await page.getTextContent();
  const strs = content.items
    .filter((i) => typeof i.str === "string")
    .map((i) => i.str);
  V.textCache.set(num, strs);
  return strs;
}

async function runFind() {
  const q = Find.input.value;
  const token = ++Find.token;
  Find.q = q;
  Find.flat = [];
  Find.idx = -1;
  clearHighlights();
  if (!q) return (Find.label.textContent = "");
  const re = findRegex();
  const doc = V.doc;
  for (let n = 1; n <= doc.n; n++) {
    const text = (await pageStrings(n)).join("");
    let k = 0;
    for (const _ of text.matchAll(re)) Find.flat.push({ page: n, k: k++ });
    if (token !== Find.token || V.doc !== doc) return;
    if (n % 20 === 0) {
      Find.label.textContent = Find.flat.length + " found…";
      await frame();
    }
  }
  if (!Find.flat.length) return (Find.label.textContent = "No results");
  const first = Find.flat.findIndex((m) => m.page >= V.current);
  showMatch(first < 0 ? 0 : first);
}
function stepFind(dir) {
  if (!Find.flat.length) return;
  showMatch((Find.idx + dir + Find.flat.length) % Find.flat.length);
}
function showMatch(i) {
  Find.idx = i;
  Find.label.textContent = `${i + 1} of ${Find.flat.length}`;
  const m = Find.flat[i];
  const p = V.pages[m.page - 1];
  Find.scrollTo = m.page;
  if (p.rendered) paintHighlights(p);
  else goToPage(m.page);
  // Rendered, but maybe far away: the highlight scroll below brings it into view.
}

function clearHighlights() {
  for (const p of V.pages) resetMarks(p);
}
function resetMarks(p) {
  if (!p.marked) return;
  p.marked.forEach((d) => (d.textContent = d.__orig));
  p.marked = null;
}

/* Marks every match on a rendered page; the current one is emphasised and scrolled to. */
function paintHighlights(p) {
  resetMarks(p);
  if (!Find.q || !p.textStr || !p.textDivs || Find.token === 0) return;
  const strs = p.textStr;
  const offsets = [];
  let total = 0;
  for (const s of strs) {
    offsets.push(total);
    total += s.length;
  }
  const text = strs.join("");
  const current = Find.flat[Find.idx];
  const segs = new Map(); // div index -> [[start, end, isCurrent]]
  let k = 0;
  for (const m of text.matchAll(findRegex())) {
    const isCur = !!current && current.page === p.num && current.k === k++;
    const start = m.index;
    const end = start + m[0].length;
    let i = upperBound(offsets, start);
    for (; i < strs.length && offsets[i] < end; i++) {
      const s = Math.max(start, offsets[i]) - offsets[i];
      const e = Math.min(end, offsets[i] + strs[i].length) - offsets[i];
      if (e > s) (segs.get(i) || segs.set(i, []).get(i)).push([s, e, isCur]);
    }
  }
  p.marked = [];
  let currentMark = null;
  for (const [i, list] of segs) {
    const div = p.textDivs[i];
    if (!div) continue;
    const str = strs[i];
    div.__orig = str;
    p.marked.push(div);
    div.textContent = "";
    let at = 0;
    for (const [s, e, isCur] of list) {
      if (s > at) div.append(str.slice(at, s));
      const mark = el("mark", isCur ? "cur" : "", str.slice(s, e));
      if (isCur) currentMark = mark;
      div.append(mark);
      at = e;
    }
    if (at < str.length) div.append(str.slice(at));
  }
  if (currentMark && Find.scrollTo === p.num) {
    Find.scrollTo = 0;
    currentMark.scrollIntoView({ block: "center", inline: "nearest" });
  }
}
/* Index of the last offset <= x. */
function upperBound(arr, x) {
  let lo = 0;
  let hi = arr.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (arr[mid] <= x) lo = mid;
    else hi = mid - 1;
  }
  return lo;
}

V.on("rendered", (p) => {
  if (!$("findbar").hidden && Find.q) paintHighlights(p);
});
V.on("doc", () => {
  Find.token++;
  Find.flat = [];
  Find.q = "";
  if (Find.input) {
    Find.input.value = "";
    Find.label.textContent = "";
  }
  $("findbar").hidden = true;
});
