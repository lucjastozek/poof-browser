import assert from "node:assert/strict";
import { mkdir, readFile } from "node:fs/promises";
import { chromium } from "playwright";

const script = await readFile("src/browser/init_script.js", "utf8");
const browser = await chromium.launch({ headless: true });
await mkdir("artifacts", { recursive: true });

async function faviconFor(url, requestedUrl, html) {
  const context = await browser.newContext();
  await context.addInitScript((requested) => {
    window.__poofRequestedUrl = requested;
    window.__poofMessages = [];
    window.ipc = {
      postMessage(message) {
        try { window.__poofMessages.push(JSON.parse(message)); } catch {}
      },
    };
  }, requestedUrl);

  const page = await context.newPage();
  await page.route(url, (route) => route.fulfill({
    status: 200,
    contentType: "text/html",
    body: html,
  }));
  await page.goto(url);
  await page.addScriptTag({ content: script });
  await page.evaluate(() => window.dispatchEvent(new Event("load")));
  await page.waitForFunction(() => window.__poofMessages.some((m) => m.t === "meta"));
  await page.screenshot({ path: `artifacts/${new URL(url).hostname}.png`, fullPage: true });
  const message = await page.evaluate(() => window.__poofMessages.find((m) => m.t === "meta"));
  await context.close();
  return message.f;
}

try {
  const genericGoogleIcon = '<link rel="icon" href="https://www.gstatic.com/images/branding/product/1x/gsa_64dp.png">';
  const gmail = await faviconFor(
    "https://mail.google.com/",
    "https://mail.google.com/",
    '<title>Gmail</title>' + genericGoogleIcon,
  );
  assert.equal(gmail, "https://ssl.gstatic.com/ui/v1/icons/mail/images/favicon_gmail_2026_v2.ico",
    "Gmail must use its product favicon, not the generic Google icon");

  const redirectedLogin = await faviconFor(
    "https://accounts.google.com/v3/signin/identifier",
    "https://mail.google.com/",
    '<title>Sign in</title><form action="/signin"><input autocomplete="username"></form>' + genericGoogleIcon,
  );
  assert.equal(redirectedLogin, "https://ssl.gstatic.com/ui/v1/icons/mail/images/favicon_gmail_2026_v2.ico",
    "Gmail login redirects must retain the requested app favicon");

  for (const [host, expected] of [
    ["calendar.google.com", "https://calendar.google.com/favicon.ico"],
    ["drive.google.com", "https://drive.google.com/favicon.ico"],
    ["docs.google.com", "https://docs.google.com/favicon.ico"],
  ]) {
    const actual = await faviconFor(
      `https://${host}/`,
      `https://${host}/`,
      `<title>${host}</title>` + genericGoogleIcon,
    );
    assert.equal(actual, expected,
      `${host} must use its product-host favicon endpoint, not Google's generic icon`);
  }

  // Exercise the actual sidebar icon loader against the official Gmail asset.
  const uiSource = await readFile("ui/scripts/core.js", "utf8");
  const freshFn = uiSource.match(/function freshFaviconUrl\\(src\\) \\{[\\s\\S]*?\\n\\}/)?.[0];
  const iconFn = uiSource.match(/function icon\\(t, guess = true\\) \\{[\\s\\S]*?\\n\\}\\nconst pretty/)?.[0]?.replace(/\\nconst pretty$/, "");
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
  `});
  const gmailAsset = "https://ssl.gstatic.com/ui/v1/icons/mail/images/favicon_gmail_2026_v2.ico";
  await uiPage.evaluate((src) => {
    const node = window.__poofIcon({ favicon: src, url: "https://accounts.google.com/" });
    node.id = "gmail-icon-test";
    document.body.append(node);
  }, gmailAsset);
  await uiPage.waitForFunction(() => {
    const img = document.querySelector("#gmail-icon-test img");
    return img && img.complete;
  }, { timeout: 30000 });
  const loadedIcon = await uiPage.evaluate(() => {
    const img = document.querySelector("#gmail-icon-test img");
    return { src: img.src, width: img.naturalWidth, height: img.naturalHeight };
  });
  assert.equal(loadedIcon.src, gmailAsset,
    "Sidebar must request the direct Gmail asset without cache-busting parameters");
  assert.ok(loadedIcon.width > 0 && loadedIcon.height > 0,
    "Official Gmail favicon must actually load as an image");
  await uiPage.screenshot({ path: "artifacts/gmail-sidebar-icon.png" });
  await uiPage.close();

  // Regression guard for the native-to-sidebar handoff: updating Tab.favicon
  // must be followed by pushing the new state to the separate sidebar WebView.
  const rust = await readFile("src/browser.rs", "utf8");
  const metaBranch = rust.match(/PageMsg::Meta \{ u, f \} => \{([\s\S]*?)\n            \}\n            PageMsg::Fullscreen/);
  assert.ok(metaBranch, "Rust PageMsg::Meta handler must remain discoverable");
  assert.match(metaBranch[1], /self\.push_state\(\)/,
    "Rust must push updated favicon metadata to the sidebar");

  console.log("PASS: Gmail, Calendar, Drive, Docs and native sidebar state handoff");
} finally {
  await browser.close();
}
