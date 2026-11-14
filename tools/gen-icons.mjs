// Generates ui/icons.js from Lucide.
// Usage: node tools/gen-icons.mjs

import { writeFileSync } from "node:fs";

const BASE = "https://raw.githubusercontent.com/lucide-icons/lucide/main/icons";

// UI name -> Lucide file name
const UI = {
  back: "arrow-left",
  fwd: "arrow-right",
  reload: "rotate-cw",
  plus: "plus",
  puzzle: "puzzle",
  pin: "pin",
  unpin: "pin-off",
  star: "star",
  x: "x",
  sliders: "sliders-horizontal",
  search: "search",
  globe: "globe",
  pencil: "pencil",
  check: "check",
  trash: "trash",
  sun: "sun",
  moon: "moon",
  monitor: "monitor",
  settings: "settings",
  external: "arrow-up-right",
  lock: "lock",
  layers: "layers",
  download: "download",
  panelLeftOpen: "panel-left-open",
  panelLeftClose: "panel-left-close",
  copy: "copy",
  link: "link",
};

// Download one SVG and extract its inner markup.
async function getIcon(name) {
  const response = await fetch(`${BASE}/${name}.svg`);

  if (!response.ok) {
    throw new Error(`Failed to download ${name}.svg: ${response.status}`);
  }

  return (await response.text())
    .replace(/^[\s\S]*?<svg[^>]*>/, "")
    .replace(/<\/svg>\s*$/, "")
    .replace(/\s*\n\s*/g, "")
    .trim();
}

const out = {
  ui: {},
  space: {},
};

console.log("Downloading Lucide UI icons...");

for (const [key, name] of Object.entries(UI)) {
  out.ui[key] = await getIcon(name);
}

console.log("Downloading complete Lucide icon library...");

const response = await fetch(
  "https://api.github.com/repos/lucide-icons/lucide/git/trees/main?recursive=1",
);

if (!response.ok) {
  throw new Error(`Failed to retrieve icon list: ${response.status}`);
}

const tree = await response.json();

if (tree.truncated) {
  throw new Error("Lucide icon listing was truncated by GitHub");
}

const names = tree.tree
  .filter((file) => file.path.startsWith("icons/") && file.path.endsWith(".svg"))
  .map((file) => file.path.slice("icons/".length, -4))
  .sort();

for (const name of names) {
  out.space[name] = await getIcon(name);
}

const output = `/* Generated from Lucide (ISC license, https://lucide.dev). */
const ICONS = ${JSON.stringify(out.ui)};
const SPACE_ICONS = ${JSON.stringify(out.space)};
`;

writeFileSync(new URL("../ui/icons.js", import.meta.url), output);

console.log(
  `Done! Generated ${Object.keys(out.ui).length} UI icons and ${Object.keys(out.space).length} space icons.`,
);
