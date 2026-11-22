/* ---------- address bar: shows the site name, the full address while editing ---------- */
const url = $("url");
let urlDraft = null;
let lastSyncedKey = null;
function syncUrl() {
  const currentKey = S ? (S.active || "") + "\n" + (S.url || "") : "";
  if (currentKey !== lastSyncedKey) {
    lastSyncedKey = currentKey;
    urlDraft = null;
  }
  if (document.activeElement !== url && urlDraft === null) {
    url.value = S ? pretty(S.url) : "";
  }
}
window.__focusUrl = () => {
  url.focus();
};
url.addEventListener("focus", () => {
  if (urlDraft === null) {
    url.value = S ? S.url || "" : "";
    url.select();
  }
});
// Inline autofill: while typing, complete the address with the best matching visited
// (or popular) site and leave the completion selected, so Backspace removes it and
// Enter accepts it.
function autofillFor(typed) {
  if (!S || !typed || /\s/.test(typed)) return null;
  const bare = typed
    .toLowerCase()
    .replace(/^https?:\/\//, "")
    .replace(/^www\./, "");
  if (!bare || bare.includes("/")) return null;
  for (const s of S.suggestions || []) {
    let host;
    try {
      host = new URL(s.url).host.replace(/^www\./, "").toLowerCase();
    } catch (_) {
      continue;
    }
    if (host.length > bare.length && host.startsWith(bare))
      return typed + host.slice(bare.length);
  }
  return null;
}
url.addEventListener("input", (e) => {
  urlDraft = url.value;
  if (
    e.isComposing ||
    e.inputType !== "insertText" ||
    url.selectionStart !== url.value.length ||
    url.selectionEnd !== url.value.length
  )
    return;
  const typed = url.value;
  const full = autofillFor(typed);
  if (full) {
    url.value = full;
    url.setSelectionRange(typed.length, full.length);
    urlDraft = full;
  }
});
url.addEventListener("blur", syncUrl);
url.addEventListener("keydown", (e) => {
  if (e.key === "Enter" && url.value.trim()) {
    urlDraft = url.value;
    go({ cmd: "navigate", input: url.value });
    url.blur();
  }
  if (e.key === "Escape") {
    url.blur();
  }
});
