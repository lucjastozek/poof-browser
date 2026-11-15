import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { chromium } from "playwright";

const browser = await chromium.launch({ headless: true });
try {
  // The injected page script now reports navigation metadata only. Favicons
  // are supplied by WebView2's native FaviconChanged/GetFavicon API.
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

  // Exercise the real sidebar loader with the same PNG data URI emitted by
  // the native WebView2 GetFavicon callback.
  const uiSource = await readFile("ui/scripts/core.js", "utf8");
  const freshFn = uiSource.match(/function freshFaviconUrl\(src\) \{[\s\S]*?\n\}/)?.[0];
  const iconFn = uiSource.match(/function icon\(t, guess = true\) \{[\s\S]*?\n\}\nconst pretty/)?.[0]?.replace(/\nconst pretty$/, "");
  assert.ok(freshFn && iconFn, "Could not extract the production sidebar icon loader");
  const uiPage = await browser.newPage();
  await uiPage.setContent("<html><body></body></html>");
  await uiPage.addScriptTag({ content: `
    const FAVICON_CACHE_TAG = "ci";
    const el = (tag, cls) => { const e = document.createElement(tag); if (cls) e.className = cls; return e; };
    const ic = () => document.createElement("span");
    ${freshFn}
    ${iconFn}
    window.__poofIcon = icon;
  `);
  const png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADUlEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC";
  await uiPage.evaluate(src => {
    const node = window.__poofIcon({ favicon: src, url: "https://example.test/" });
    node.id = "native-favicon";
    document.body.append(node);
  }, png);
  await uiPage.waitForFunction(() => {
    const img = document.querySelector("#native-favicon img");
    return img && img.complete;
  });
  const loaded = await uiPage.evaluate(() => {
    const img = document.querySelector("#native-favicon img");
    return { src: img.src, width: img.naturalWidth, height: img.naturalHeight };
  });
  assert.equal(loaded.src, png, "Sidebar must preserve native favicon data without URL rewriting");
  assert.ok(loaded.width > 0 && loaded.height > 0, "Sidebar must decode the native PNG data URI");
  await uiPage.screenshot({ path: "artifacts/native-favicon-sidebar.png" });
  await uiPage.close();

  const rust = await readFile("src/browser.rs", "utf8");
  const metaBranch = rust.match(/PageMsg::Meta \{ u, f \} => \{([\s\S]*?)\n            \}\n            PageMsg::Favicon/);
  assert.ok(metaBranch, "Rust page metadata handler must remain discoverable");
  assert.match(metaBranch[1], /self\.push_state\(\)/,
    "Page metadata must be pushed to the separate sidebar WebView");
  assert.ok(rust.includes("FaviconChangedEventHandler::create"),
    "WebView2 FaviconChanged must drive favicon updates");
  assert.ok(rust.includes("GetFaviconCompletedHandler::create"),
    "WebView2 GetFavicon must retrieve the engine-decoded image");
  assert.ok(rust.includes("COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG"),
    "WebView2 must normalize ICO and SVG favicons to PNG");
  assert.ok(rust.includes("data:image/png;base64,"),
    "Native PNG bytes must be passed to the sidebar as an embedded image");
  assert.ok(rust.includes("body.len() > 700_000"),
    "IPC size limit must allow bounded embedded favicon data");
  console.log("PASS: native WebView2 favicon handoff and production sidebar image loader");
} finally {
  await browser.close();
}
