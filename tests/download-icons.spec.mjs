import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import vm from "node:vm";

const source = await readFile("ui/scripts/views/downloads.js", "utf8");
const match = source.match(/const FILE_TYPES = (\{[\s\S]*?\n  \});/);
assert.ok(match, "Downloads must declare an extension classifier");
const types = vm.runInNewContext(`(${match[1]})`);
const cases = {
  document: ["doc", "docx", "odt", "rtf"],
  pdf: ["pdf"],
  spreadsheet: ["csv", "ods", "xls", "xlsx"],
  presentation: ["key", "odp", "ppt", "pptx"],
  image: ["avif", "gif", "heic", "jpeg", "png", "svg", "webp"],
  video: ["avi", "mkv", "mov", "mp4", "webm"],
  audio: ["aac", "flac", "m4a", "mp3", "ogg", "wav"],
  archive: ["7z", "bz2", "gz", "rar", "tar", "zip"],
  code: ["c", "css", "go", "html", "js", "json", "py", "rs", "ts", "yaml"],
  package: ["apk", "deb", "dmg", "exe", "ipa", "msi", "rpm"],
  font: ["otf", "ttf", "woff", "woff2"],
  database: ["db", "mdb", "sqlite", "sqlite3"],
  design: ["blend", "dwg", "fbx", "obj", "stl"],
  ebook: ["azw", "cbr", "cbz", "epub", "mobi"],
  email: ["eml", "msg", "pst", "vcf"],
  subtitle: ["ass", "srt", "vtt"],
  config: ["cfg", "conf", "env", "ini", "toml"],
  certificate: ["cer", "crt", "pem", "pfx"],
  torrent: ["torrent"],
};
for (const [type, extensions] of Object.entries(cases)) {
  assert.ok(types[type], `Missing ${type} category`);
  for (const ext of extensions) assert.ok(types[type].has(ext), `Missing .${ext}`);
}
const iconMatch = source.match(/const FILE_ICONS = (\{[\s\S]*?\n  \});/);
assert.ok(iconMatch, "Every file category must have an icon mapping");
const iconsByType = vm.runInNewContext(`(${iconMatch[1]})`);
const generated = await readFile("ui/icons.js", "utf8");
const generatedMatch = generated.match(/const ICONS = (\{[\s\S]*?\});\nconst SPACE_ICONS =/);
assert.ok(generatedMatch, "Generated MingCute UI icons must be readable");
const generatedIcons = JSON.parse(generatedMatch[1]);
for (const [type, icon] of Object.entries(iconsByType)) {
  assert.ok(types[type], `Icon mapping has no extension category: ${type}`);
  assert.ok(generatedIcons[icon], `Missing generated MingCute icon: ${icon}`);
}
const all = Object.entries(types).flatMap(([type, extensions]) =>
  [...extensions].map(ext => [ext, type])
);
assert.equal(new Set(all.map(([ext]) => ext)).size, all.length,
  "An extension must not resolve to conflicting file types");
assert.match(source, /ic\(type \? FILE_ICONS\[type\] : "file-default", 16\)/,
  "Downloads must show a small file-type icon instead of extension text");
console.log("PASS: common download extensions map to distinct file categories");
