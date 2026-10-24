// Round 3G item 3: the pages work under the app's real Content-Security-Policy,
// with Object.prototype frozen the way Tauri's `freezePrototype` freezes it.
// Serves the built pages (dist/) with the CSP from tauri.conf.json as a
// header, opens the island, every Settings page and tab, the Studio and the
// desktop Groovy in a browser, and fails on any CSP violation or script error.
//
//   npx vite build && node scripts/csp-check.mjs [dist dir]
// Uses playwright-core with the installed Edge (CI) or $CHROME (elsewhere).

import { createReadStream, existsSync, readFileSync, statSync } from "node:fs";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import { dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const dist = resolve(process.argv[2] ?? join(root, "dist"));
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || "playwright-core");

const conf = JSON.parse(readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8"));
const csp = conf.app.security.csp;
if (/unsafe-inline/.test(csp.match(/style-src[^;]*/)?.[0] ?? "")) {
  console.log("FAIL: the release CSP still allows inline styles");
  process.exit(1);
}
if (conf.app.security.freezePrototype !== true) {
  console.log("FAIL: freezePrototype is off");
  process.exit(1);
}

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".json": "application/json", ".woff2": "font/woff2", ".svg": "image/svg+xml", ".png": "image/png" };
const server = createServer((req, res) => {
  const path = join(dist, decodeURIComponent(new URL(req.url, "http://x").pathname));
  if (!path.startsWith(dist) || !existsSync(path) || !statSync(path).isFile()) {
    res.writeHead(404).end();
    return;
  }
  res.writeHead(200, { "content-type": TYPES[extname(path)] ?? "application/octet-stream", "content-security-policy": csp });
  createReadStream(path).pipe(res);
});
await new Promise((ok) => server.listen(0, "127.0.0.1", ok));
const base = `http://127.0.0.1:${server.address().port}`;

const { ALL_PAGES, TABS } = await import("../src/hub/nav.ts").catch(() => ({}));
const places = [];
for (const p of ALL_PAGES ?? []) {
  const tabs = TABS[p.id];
  if (tabs) for (const tab of tabs) places.push(`settings.html?page=${p.id}${tab.id === tabs[0].id ? "" : `&tab=${tab.id}`}`);
  else places.push(`settings.html?page=${p.id}`);
}
places.push("index.html", "studio.html", "mochi.html");

const browser = await chromium.launch(process.env.CHROME ? { executablePath: process.env.CHROME } : { channel: "msedge" });
const problems = [];
for (const place of places) {
  const page = await browser.newPage({ viewport: { width: 1000, height: 800 } });
  await page.addInitScript(() => {
    Object.freeze(Object.prototype); // what freezePrototype does
    document.addEventListener("securitypolicyviolation", (e) => {
      (window.__csp ??= []).push(`${e.violatedDirective}: ${e.blockedURI || "inline"} ${e.sample ?? ""}`.trim());
    });
  });
  page.on("pageerror", (e) => problems.push(`${place}: error: ${e.message}`));
  page.on("console", (m) => {
    if (m.type() === "error" && /Content Security Policy|Refused to/i.test(m.text())) problems.push(`${place}: ${m.text().slice(0, 200)}`);
  });
  await page.goto(`${base}/${place}`, { waitUntil: "load" });
  await page.evaluate(() => document.body.classList.add("shown"));
  await page.waitForTimeout(1500);
  const seen = await page.evaluate(() => window.__csp ?? []);
  for (const v of seen) problems.push(`${place}: blocked ${v}`);
  await page.close();
}
await browser.close();
server.close();
console.log(problems.length ? problems.join("\n") : `CSP: ${places.length} pages opened under the release policy with a frozen Object.prototype, nothing blocked`);
if (problems.length) process.exit(1);
