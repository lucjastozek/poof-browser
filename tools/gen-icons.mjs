// Generates ui/icons.js from MingCute (Apache-2.0, https://www.mingcute.com).
// Usage: node tools/gen-icons.mjs
// Optional: set GITHUB_TOKEN to avoid GitHub API rate limits.
// Optional: set MINGCUTE_DIR=/path/to/mingcute to read from a local clone instead of the network.

import { writeFileSync, readFileSync, readdirSync } from "node:fs";
import { gzipSync } from "node:zlib";
import { join } from "node:path";

const REPO = "Richard9394/MingCute";
const BRANCHES = ["main", "master"];
// MingCute now ships as packages/svg/<style>/<name>.svg with kebab-case names.
// "core-regular" is the outline ("line") style, "core-filled" the filled one.
const STYLE_DIR = "packages/svg/core-regular";

// UI name -> MingCute icon names, tried in order (first one that exists wins).
const UI = {
  back: ["arrow-left", "left"],
  fwd: ["arrow-right", "right"],
  reload: ["refresh-1", "refresh-2", "refresh-3"],
  plus: ["add"],
  puzzle: ["puzzle", "plugin-2", "plugin", "box"],
  pin: ["pin", "pin-2"],
  unpin: ["pin-off", "unpin", "pin-2", "pin"],
  star: ["star"],
  x: ["close"],
  sliders: ["settings-horizontal", "sliders", "settings-6", "filter"],
  search: ["search"],
  globe: ["earth", "globe", "world-2"],
  pencil: ["pencil", "edit", "edit-2"],
  check: ["check"],
  trash: ["delete-2", "delete"],
  sun: ["sun"],
  moon: ["moon", "moon-stars"],
  monitor: ["computer", "monitor", "display", "tv-2"],
  settings: ["settings-3", "settings-1", "settings-2"],
  external: ["external-link", "arrow-right-up"],
  lock: ["lock"],
  layers: ["layers", "layers-2", "stack"],
  download: ["download-2", "download"],
  panelLeftOpen: [
    "sidebar-left-open",
    "layout-leftbar-open",
    "sidebar-left",
    "layout-left",
  ],
  panelLeftClose: [
    "sidebar-left-close",
    "layout-leftbar-close",
    "sidebar-left",
    "layout-left",
  ],
  copy: ["copy-2", "copy"],
  link: ["link", "link-2"],
  code: ["code", "terminal"],
  // Used directly by sidebar.js:
  folder: ["folder-2", "folder"],
  "folder-open": ["folder-open", "folder-open-2"],
  "chevron-right": ["right"],
  "chevron-down": ["down"],
};

// Saved spaces may still hold Lucide icon names. MingCute's names match Lucide's for many icons
// (house, star, heart, folder ...) and those work as-is. For Lucide names MingCute spells
// differently, this maps them to the closest MingCute icon. Native names always win, and
// Lucide names that are in neither place fall back to "house" at runtime (see sic() in core.js).
const LUCIDE_ALIASES = {
  x: "close",
  "x-circle": "close-circle",
  "circle-x": "close-circle",
  "x-square": "close-square",
  "square-x": "close-square",
  "check-circle": "check-circle",
  "circle-check": "check-circle",
  "check-circle-2": "check-circle",
  "check-square": "checkbox",
  "square-check": "checkbox",
  "check-check": "checks",
  "plus-circle": "add-circle",
  "circle-plus": "add-circle",
  "plus-square": "add-square",
  "square-plus": "add-square",
  minus: "minus-circle",
  "circle-minus": "minus-circle",
  "square-minus": "minus-square",
  "alert-circle": "alert",
  "circle-alert": "alert",
  "alert-triangle": "alert",
  "triangle-alert": "alert",
  "alert-octagon": "alert-octagon",
  "octagon-alert": "alert-octagon",
  "help-circle": "question",
  "circle-help": "question",
  "circle-question-mark": "question",
  "question-mark": "question",
  info: "information",
  "chevron-down": "down",
  "chevron-up": "up",
  "chevron-left": "left",
  "chevron-right": "right",
  "chevrons-down": "down",
  "chevrons-up": "up",
  "chevrons-left": "left",
  "chevrons-right": "right",
  "arrow-up-right": "arrow-right-up",
  "arrow-up-left": "arrow-left-up",
  "arrow-down-right": "arrow-right-down",
  "arrow-down-left": "arrow-left-down",
  "arrow-up-right-from-square": "external-link",
  "external-link": "external-link",
  home: "home-1",
  settings: "settings-3",
  cog: "settings-3",
  sliders: "settings-horizontal",
  "sliders-horizontal": "settings-horizontal",
  "sliders-vertical": "settings-vertical",
  trash: "delete-2",
  "trash-2": "delete-2",
  pen: "pen",
  "pencil-line": "pencil",
  "pen-line": "pen",
  "pen-square": "edit",
  "square-pen": "edit",
  "file-edit": "edit",
  bell: "bell-ringing",
  "bell-ring": "bell-ringing",
  user: "user-1",
  "user-round": "user-1",
  "user-circle": "user-1",
  "circle-user-round": "user-1",
  users: "group",
  "users-round": "group",
  "user-plus": "user-add",
  "user-check": "user-follow",
  "more-horizontal": "more-1",
  "more-vertical": "more-2",
  ellipsis: "more-1",
  "ellipsis-vertical": "more-2",
  "menu-square": "menu",
  "message-circle": "chat-1",
  "message-square": "message-1",
  "messages-circle": "chat-2",
  "send-horizontal": "send",
  "send-horizonal": "send",
  "refresh-cw": "refresh-2",
  "refresh-ccw": "refresh-1",
  "rotate-cw": "refresh-2",
  "rotate-ccw": "refresh-1",
  loader: "loading",
  "loader-2": "loading",
  "loader-circle": "loading",
  zap: "flash",
  bolt: "flash",
  cpu: "chip",
  microchip: "chip",
  "calendar-days": "calendar-1",
  "calendar-check": "calendar-check-2",
  "calendar-plus": "calendar-add",
  "briefcase-business": "briefcase-2",
  "shopping-cart": "shopping-cart-1",
  "shopping-bag": "shopping-bag-1",
  "credit-card": "card-pay",
  banknote: "bank",
  "wallet-minimal": "wallet-2",
  "dollar-sign": "currency-dollar",
  image: "photo-album",
  images: "photo-album-2",
  "book-open": "book-2",
  "file-text": "file",
  files: "file",
  "folder-plus": "folder-more",
  "folder-closed": "folder",
  "folder-x": "folder-delete",
  sparkle: "sparkles",
  stars: "sparkles-2",
  "wand-sparkles": "magic-2",
  wand: "magic-1",
  "wand-2": "magic-2",
  palette: "palette",
  paintbrush: "brush",
  "paintbrush-vertical": "brush-2",
  "paint-bucket": "paint",
  lightbulb: "lightbulb-off",
  "graduation-cap": "school",
  "monitor-pc": "computer",
  "laptop-minimal": "laptop-2",
  smartphone: "phone",
  "tv-minimal": "tv-1",
  printer: "print",
  mail: "mail-open",
  "at-sign": "at",
  "map-pinned": "map-pin",
  "navigation-2": "navigation",
  "panel-left": "sidebar-left",
  "panel-left-open": "sidebar-left-open",
  "panel-left-close": "sidebar-left-close",
  sidebar: "sidebar-left",
  "sidebar-open": "sidebar-left-open",
  "sidebar-close": "sidebar-left-close",
  "layout-dashboard": "dashboard",
  "layout-grid": "grid",
  "layout-list": "list-check",
  "list-checks": "list-check",
  "list-filter": "filter",
  funnel: "filter",
  "code-xml": "code",
  "code-2": "code",
  "square-terminal": "terminal-box",
  "terminal-square": "terminal-box",
  flame: "flame",
  droplet: "droplets",
  "cloud-rain": "cloud-sun-rain",
  "cloud-sun": "sun-cloudy",
  "cloud-moon": "moon-cloudy",
  "sun-moon": "sun-2",
  "moon-star": "moon-stars",
  "star-off": "star",
  "thumbs-up": "thumb-up",
  "thumbs-down": "thumb-down",
  "lock-keyhole": "lock-2",
  "unlock-keyhole": "lock-open",
  "key-round": "key-2",
  "shield-check": "shield-shape",
  "log-in": "enter-door",
  "log-out": "exit-door",
  share: "share-2",
  "upload-cloud": "cloud-backup",
  "download-cloud": "cloud-backup",
  "save-all": "save-2",
  "archive-arrow-down": "archive",
  puzzle: "puzzled",
  plug: "plugin",
  "plug-2": "plugin-2",
  toolbox: "tool",
  wrench: "tool",
  hammer: "hammer",
  rocket: "rocket",
  timer: "stopwatch",
  hourglass: "sandglass",
  "alarm-clock": "alarm-1",
  dumbbell: "barbell",
  "biceps-flexed": "barbell",
  flag: "flag-1",
  "bookmark-plus": "bookmark-add",
  tags: "tag-2",
  "eye-closed": "eye-close",
  "mic-vocal": "mic",
  "volume-2": "volume",
  "volume-1": "volume-low",
  "volume-x": "volume-mute",
  "play-square": "play-circle",
  "circle-play": "play-circle",
  "circle-pause": "pause-circle",
  "circle-stop": "stop-circle",
  "zodiac-aries": "star",
  trees: "tree-2",
  "tree-pine": "tree-3",
  "tree-deciduous": "tree",
  sprout: "leaf-2",
  shrub: "leaf-3",
  "mountain-snow": "mountain-2",
  waves: "wave",
  earbuds: "headphone",
  headset: "headphone-2",
  "gamepad-2": "game-1",
  gamepad: "game-2",
};

const headers = { "User-Agent": "poof-icon-generator" };
if (process.env.GITHUB_TOKEN)
  headers.Authorization = `Bearer ${process.env.GITHUB_TOKEN}`;

async function pool(items, limit, fn) {
  const results = new Array(items.length);
  let next = 0;
  const worker = async () => {
    for (;;) {
      const i = next++;
      if (i >= items.length) return;
      results[i] = await fn(items[i], i);
    }
  };
  await Promise.all(
    Array.from({ length: Math.min(limit, items.length) }, worker),
  );
  return results;
}

async function fetchText(url, tries = 3) {
  let last;
  for (let i = 0; i < tries; i++) {
    try {
      const response = await fetch(url, { headers });
      if (!response.ok) throw new Error(`${response.status} ${url}`);
      return await response.text();
    } catch (error) {
      last = error;
      await new Promise((r) => setTimeout(r, 400 * (i + 1)));
    }
  }
  throw last;
}

// MingCute SVGs are a single filled <path fill="currentColor">. Keep only the inner markup;
// svg() in core.js supplies the <svg> wrapper (viewBox, size, fill).
function inner(svg) {
  return svg
    .replace(/^[\s\S]*?<svg[^>]*>/, "")
    .replace(/<\/svg>\s*$/, "")
    .replace(/\s*\n\s*/g, "")
    .trim();
}

// name (e.g. "arrow-left") -> how to load it (repo path, or local file)
const index = new Map();
let load; // (name) => Promise<string>
const localDir = process.env.MINGCUTE_DIR;

if (localDir) {
  console.log(`Reading MingCute icons from ${localDir} ...`);
  const dir = join(localDir, STYLE_DIR);
  for (const f of readdirSync(dir))
    if (f.endsWith(".svg")) index.set(f.slice(0, -4), join(dir, f));
  load = async (name) => readFileSync(index.get(name), "utf8");
} else {
  console.log("Listing MingCute icons...");
  let tree = null;
  let branch = null;
  const errors = [];
  for (const b of BRANCHES) {
    try {
      tree = JSON.parse(
        await fetchText(
          `https://api.github.com/repos/${REPO}/git/trees/${b}?recursive=1`,
        ),
      );
      branch = b;
      break;
    } catch (error) {
      errors.push(`${b}: ${error.message}`);
    }
  }
  if (!tree)
    throw new Error(
      "Could not read the MingCute repository tree:\n  " + errors.join("\n  "),
    );
  if (tree.truncated) throw new Error("MingCute icon listing was truncated");

  const prefix = STYLE_DIR + "/";
  for (const file of tree.tree) {
    if (!file.path.startsWith(prefix) || !file.path.endsWith(".svg")) continue;
    const name = file.path.slice(prefix.length, -4);
    if (!name.includes("/")) index.set(name, file.path);
  }
  console.log(`Branch "${branch}".`);
  load = (name) =>
    fetchText(
      `https://raw.githubusercontent.com/${REPO}/${branch}/${index.get(name)}`,
    );
}

if (index.size === 0)
  throw new Error(
    `No icons found under ${STYLE_DIR}. The MingCute repo layout may have changed again.`,
  );
console.log(`Found ${index.size} icons.`);

const missing = [];
function resolve(label, candidates) {
  for (const c of candidates) if (index.has(c)) return c;
  const token = candidates[0].split("-")[0];
  const hints = [...index.keys()].filter((n) => n.includes(token)).slice(0, 10);
  missing.push(
    `${label}: none of [${candidates.join(", ")}]. Try: ${hints.join(", ") || "(no hints)"}`,
  );
  return null;
}

const uiPick = {};
for (const [key, candidates] of Object.entries(UI)) {
  const name = resolve(key, candidates);
  if (name) {
    uiPick[key] = name;
    if (name !== candidates[0])
      console.log(
        `  ${key}: using ${name} (first choice ${candidates[0]} not found)`,
      );
  }
}
const aliasPick = {};
const badAliases = [];
for (const [lucide, target] of Object.entries(LUCIDE_ALIASES)) {
  if (index.has(lucide)) continue; // MingCute already has this name natively
  if (index.has(target)) aliasPick[lucide] = target;
  else badAliases.push(`${lucide} -> ${target}`);
}
if (badAliases.length)
  console.warn(
    `Skipped ${badAliases.length} Lucide aliases with missing targets: ${badAliases.join(", ")}`,
  );
if (missing.length) {
  throw new Error(
    "Some icons could not be found. Edit the candidate lists at the top of this file:\n  " +
      missing.join("\n  "),
  );
}

// Space picker. The whole UI is loaded with WebViewBuilder::with_html, which on Windows goes
// through WebView2's NavigateToString and fails with E_INVALIDARG (0x80070057) when the page is
// over 2 MB. All ~2300 MingCute icons are ~2.5 MB as text, so they are split in two:
//   * SPACE_ICONS: a small curated set embedded as plain text, available immediately.
//   * SPACE_ICONS_PACKED: every icon, gzipped + base64 (~0.9 MB). core.js unpacks it in the
//     background at startup (DecompressionStream) and merges it into SPACE_ICONS.
// Lucide names from older saved spaces resolve directly or via SPACE_ALIASES, and anything
// unknown falls back to "house" (see spaceIconName() in core.js).
const SPACE_FIRST = [
  "house",
  "home-1",
  "home-2",
  "home-3",
  "home-4",
  "home-5",
  "home-6",
  "home-7",
  "building-1",
  "building-2",
  "store",
  "briefcase",
  "book-2",
  "calendar-1",
  "camera",
  "video",
  "film",
  "music",
  "headphone",
  "game-1",
  "palette",
  "brush",
  "pen",
  "pencil",
  "code",
  "terminal",
  "server",
  "laptop",
  "computer",
  "phone",
  "cloud",
  "sun",
  "moon",
  "star",
  "heart",
  "gift",
  "trophy",
  "rocket",
  "flag-1",
  "compass",
  "map",
  "earth",
  "globe",
  "flask",
  "bug",
  "leaf",
  "tree",
  "sparkles",
  "magic-1",
  "fire",
  "mail",
  "chat-1",
  "bell",
  "user-1",
  "group",
  "shopping-cart-1",
  "wallet",
  "bike",
  "car",
  "walk",
  "shield",
  "lock",
  "tool",
  "settings-3",
  "layers",
  "box",
  "folder",
  "file",
  "link",
  "clock-1",
  "eye",
  "bird",
  "cat",
  "dog",
  "fish",
  "rabbit",
];

const allNames = [...index.keys()].sort();
const curated = new Set(SPACE_FIRST.filter((n) => index.has(n)));
for (const target of Object.values(aliasPick)) curated.add(target);

const needed = new Set([...Object.values(uiPick), ...allNames]);
console.log(`Loading ${needed.size} icons...`);
const cache = new Map();
let done = 0;
await pool([...needed], 16, async (name) => {
  cache.set(name, inner(await load(name)));
  if (++done % 500 === 0) console.log(`  ${done}/${needed.size}`);
});

const ui = {};
for (const [key, name] of Object.entries(uiPick)) ui[key] = cache.get(name);

const embedded = {};
const packedSet = {};
for (const name of allNames) {
  if (curated.has(name)) embedded[name] = cache.get(name);
  else packedSet[name] = cache.get(name);
}
const aliases = {};
for (const [lucide, target] of Object.entries(aliasPick))
  if (!index.has(lucide)) aliases[lucide] = target;

const packed = gzipSync(Buffer.from(JSON.stringify(packedSet)), {
  level: 9,
}).toString("base64");

const output = `/* Generated from MingCute (Apache-2.0, https://www.mingcute.com). */
const ICONS = ${JSON.stringify(ui)};
const SPACE_ICONS = ${JSON.stringify(embedded)};
const SPACE_ALIASES = ${JSON.stringify(aliases)};
const SPACE_ICONS_PACKED = "${packed}";
`;

const bytes = Buffer.byteLength(output);
if (bytes > 1_300_000)
  throw new Error(
    `icons.js is ${bytes} bytes. The whole UI page must stay under 2 MB (NavigateToString limit), ` +
      `so keep this under ~1.3 MB.`,
  );

writeFileSync(new URL("../ui/icons.js", import.meta.url), output);

console.log(
  `Done! ${Object.keys(ui).length} UI icons, ${Object.keys(embedded).length} embedded + ` +
    `${Object.keys(packedSet).length} packed space icons, ${Object.keys(aliases).length} Lucide aliases ` +
    `(${(bytes / 1024).toFixed(0)} KB).`,
);
