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
  const freshFn = uiSource.match(/function freshFaviconUrl\(src\) \{[\s\S]*?\n\}/)?.[0];
  assert.ok(freshFn, "Could not extract the production favicon URL normalizer");
  assert.ok(uiSource.includes("i.src = freshFaviconUrl(candidates[index]);"),
    "Sidebar icon loader must use the production favicon URL normalizer");
  const uiPage = await browser.newPage();
  await uiPage.setContent("<html><body></body></html>");
  await uiPage.addScriptTag({ content: freshFn + "\nwindow.__freshFaviconUrl = freshFaviconUrl;" });
  const png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADUlEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC";
  await uiPage.evaluate(src => {
    const img = new Image();
    img.id = "native-favicon";
    img.src = window.__freshFaviconUrl(src);
    document.body.append(img);
  }, png);
  await uiPage.waitForFunction(() => {
    const img = document.querySelector("#native-favicon");
    return img && img.complete;
  });
  const loaded = await uiPage.evaluate(() => {
    const img = document.querySelector("#native-favicon");
    return { src: img.src, width: img.naturalWidth, height: img.naturalHeight };
  });
  assert.equal(loaded.src, png, "Sidebar must preserve native favicon data without URL rewriting");
  assert.ok(loaded.width > 0 && loaded.height > 0, "Sidebar must decode the native PNG data URI");
  await uiPage.screenshot({ path: "artifacts/native-favicon-sidebar.png" });
  await uiPage.close();

  const rust = await readFile("src/browser.rs", "utf8");
  const metaBranch = rust.match(/PageMsg::Meta \{ u \} => \{([\s\S]*?)\n            \}\n            PageMsg::Favicon/);
  assert.ok(metaBranch, "Rust page metadata handler must remain discoverable");
  assert.match(metaBranch[1], /self\.push_state\(\)/,
    "Page metadata must be pushed to the separate sidebar WebView");
  assert.ok(rust.includes("FaviconChangedEventHandler::create"),
    "WebView2 FaviconChanged must drive favicon updates");
  assert.ok(rust.includes("GetFaviconCompletedHandler::create"),
    "WebView2 GetFavicon must retrieve the engine-decoded image");
  assert.ok(rust.includes("COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG"),
    "WebView2 must normalize ICO and SVG favicons to PNG");
  assert.equal(rust.split('json!({"t": "favicon", "f": data_uri}).to_string()').length - 1, 2,
    "Both initial and changed favicons must use the native IPC payload");
  assert.ok(rust.includes("body.len() > 700_000"),
    "IPC size limit must allow bounded embedded favicon data");
  console.log("PASS: native WebView2 favicon handoff and sidebar PNG handling");
} finally {
  await browser.close();
}
