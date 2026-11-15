import assert from "node:assert/strict";
import { mkdir, readFile } from "node:fs/promises";
import { chromium } from "playwright";

const browser = await chromium.launch({ headless: true });
await mkdir("artifacts", { recursive: true });
try {
  const init = await readFile("src/browser/init_script.js", "utf8");
  const context = await browser.newContext();
  await context.addInitScript(() => {
    window.__poofMessages = [];
    window.ipc = { postMessage(message) {
      try { window.__poofMessages.push(JSON.parse(message)); } catch {}
    }};
  });
  const page = await context.newPage();
  await page.route("https://example.test/**", route => route.fulfill({
    status: 200, contentType: "text/html",
    body: "<title>Example</title><link rel='icon' href='/favicon.ico'>",
  }));
  await page.goto("https://example.test/");
  await page.addScriptTag({ content: init });
  await page.evaluate(() => window.dispatchEvent(new Event("load")));
  await page.waitForFunction(() => window.__poofMessages.some(m => m.t === "meta"));
  const meta = await page.evaluate(() => window.__poofMessages.find(m => m.t === "meta"));
  assert.equal(meta.u, "https://example.test/");
  assert.equal(Object.hasOwn(meta, "f"), false,
    "Page JavaScript must not overwrite the browser-native favicon");
  assert.equal(init.includes("function icon()"), false,
    "Old URL-scraping favicon implementation must be removed");
  await context.close();

  // Test the production URL normalizer used by the sidebar's actual icon loader.
  const uiSource = await readFile("ui/scripts/core.js", "utf8");
  const sidebarCss = await readFile("ui/styles/sidebar.css", "utf8");
  assert.match(sidebarCss, /#favs\s*\{[^}]*display:\s*flex;[^}]*flex-wrap:\s*wrap/s,
    "Favorite tiles must retain their original wrapping layout");
  assert.doesNotMatch(sidebarCss.match(/#favs\s*\{[^}]*\}/s)?.[0] || "", /display:\s*grid/,
    "Favorite tiles must not retain the obsolete grid layout declaration");
  assert.match(sidebarCss, /\.tile\s*\{[^}]*flex:\s*1 1 calc\(\(100% - 18px\) \/ 4\);[^}]*height:\s*46px/s,
    "Favorite tiles must retain their original dimensions");
  assert.match(sidebarCss, /\.tile \.ico img\s*\{[^}]*width:\s*20px;[^}]*height:\s*20px/s,
    "Favorite favicon images must retain the original 20px display size");
  assert.ok(uiSource.indexOf("add(t.favicon);") <
    uiSource.indexOf('add(new URL("/favicon.ico", site).href)'),
    "The browser-provided site favicon must remain the first choice");
  assert.ok(uiSource.indexOf('add(new URL("/favicon.ico", site).href)') <
    uiSource.indexOf('add(new URL("/apple-touch-icon.png", site).href)'),
    "Site-specific favicon files must be tried before generic fallbacks");
  assert.ok(uiSource.indexOf('add(new URL("/apple-touch-icon.png", site).href)') <
    uiSource.indexOf("https://www.google.com/s2/favicons"),
    "Google's generic favicon service must remain the final fallback");
  assert.ok(uiSource.includes("i.src = candidates[index];"),
    "Sidebar icon loader must use the original favicon source URL");
  assert.doesNotMatch(uiSource, /_poof_icon_refresh|freshFaviconUrl/,
    "Favicon source URLs must not be rewritten with cache-busting query parameters");
  const rust = await readFile("src/browser.rs", "utf8");
  const metaBranch = rust.match(/PageMsg::Meta \{ u \} => \{([\s\S]*?)\n            \}\n            PageMsg::Favicon/);
  assert.ok(metaBranch, "Rust page metadata handler must remain discoverable");
  assert.match(metaBranch[1], /self\.push_state\(\)/,
    "Page metadata must be pushed to the separate sidebar WebView");
  assert.ok(rust.includes("FaviconChangedEventHandler::create"),
    "WebView2 FaviconChanged must drive favicon updates");
  assert.ok(rust.includes("core15.FaviconUri(&mut uri)"),
    "Sidebar and tab rows must use the original favicon URI, not a small raster snapshot");
  assert.ok(rust.includes("current_favicon_uri(&core15)"),
    "Initial and changed favicon events must publish the original source URI");
  assert.doesNotMatch(rust, /GetFaviconCompletedHandler|COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG/,
    "The favicon pipeline must not rasterize source icons into low-resolution PNGs");
  assert.equal(rust.split('json!({"t": "favicon", "f": favicon}).to_string()').length - 1, 2,
    "Both initial and changed favicons must use the original URI IPC payload");
  console.log("PASS: native WebView2 favicon URI handoff and original-source rendering");
} finally {
  await browser.close();
}
