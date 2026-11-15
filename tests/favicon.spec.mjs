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
  assert.equal(gmail, "https://gmail.google.com/favicon.ico",
    "Gmail must use its product favicon, not the generic Google icon");

  const redirectedLogin = await faviconFor(
    "https://accounts.google.com/v3/signin/identifier",
    "https://mail.google.com/",
    '<title>Sign in</title><form action="/signin"><input autocomplete="username"></form>' + genericGoogleIcon,
  );
  assert.equal(redirectedLogin, "https://gmail.google.com/favicon.ico",
    "Gmail login redirects must retain the requested app favicon");

  console.log("PASS: Gmail favicon selection on app and Google sign-in pages");
} finally {
  await browser.close();
}
