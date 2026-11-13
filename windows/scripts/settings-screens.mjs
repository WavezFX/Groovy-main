// Round 3D item 3: one screenshot of every Settings page (and tab), dark and
// light, for the owner to review (CI uploads them as `settings-screens`).
//   node scripts/settings-screens.mjs <base url> <out dir>
// Uses playwright-core with the installed Edge (CI) or $CHROME (elsewhere).

import { mkdirSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";

const [base = "http://127.0.0.1:1420", out = "settings-screens"] = process.argv.slice(2);
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || "playwright-core");

const { ALL_PAGES, TABS } = await import("../src/hub/nav.ts").catch(() => ({}));
const places = [];
for (const p of ALL_PAGES ?? []) {
  const tabs = TABS[p.id];
  if (tabs) for (const tab of tabs) places.push(`${p.id}${tab.id === tabs[0].id ? "" : `:${tab.id}`}`);
  else places.push(p.id);
}

mkdirSync(out, { recursive: true });
const browser = await chromium.launch(process.env.CHROME ? { executablePath: process.env.CHROME } : { channel: "msedge" });
const errors = [];
for (const scheme of ["dark", "light"]) {
  const page = await browser.newPage({ viewport: { width: 1000, height: 1500 }, colorScheme: scheme });
  page.on("pageerror", (e) => errors.push(`${scheme}: ${e.message}`));
  for (const place of places) {
    const [id, tab] = place.split(":");
    await page.goto(`${base}/settings.html?page=${id}${tab ? `&tab=${tab}` : ""}`, { waitUntil: "load" });
    // Outside the app nothing grows the window out of the island: show it at once.
    await page.evaluate(() => document.body.classList.add("shown"));
    await page.waitForTimeout(1200);
    if (await page.$("vite-error-overlay")) errors.push(`${place}: the page didn't build`);
    await page.screenshot({ path: join(out, `${place.replace(":", "-")}-${scheme}.png`) });
  }
  await page.close();
}

// Round 3D item 4: switching pages in one window, by the sidebar, each under 100 ms.
const tour = await browser.newPage({ viewport: { width: 1000, height: 900 } });
// Round 3D item 9: a page's own code is fetched the first time it opens, and only then.
const LAZY = ["brains", "wardrobe", "islandpage", "system", "memory", "voice", "music", "phone"];
let round = -1;
const fetched = [];
tour.on("request", (r) => {
  const m = r.url().match(/\/src\/settings\/(\w+)\.ts/);
  if (m && LAZY.includes(m[1])) fetched.push([round, m[1]]);
});
await tour.goto(`${base}/settings.html?page=home`, { waitUntil: "load" });
await tour.evaluate(() => document.body.classList.add("shown"));
await tour.waitForTimeout(1500);
const items = await tour.$$(".hub-item");
for (round = 0; round < 2; round++) {
  for (const item of items) {
    await item.click();
    await tour.waitForTimeout(350);
  }
}
const switches = await tour.evaluate(() => window.__hubSwitchMs ?? {});
const at = (r) => fetched.filter(([n]) => n === r).map(([, m]) => m);
console.log(`page code fetched at start: ${at(-1).join(", ") || "none"}; first visits: ${[...new Set(at(0))].join(", ")}; second visits: ${at(1).join(", ") || "none"}`);
if (at(-1).length) errors.push(`opening Settings on Home fetched ${at(-1).join(", ")} (they load on their first visit)`);
for (const m of ["brains", "wardrobe", "islandpage", "system"]) if (!at(0).includes(m)) errors.push(`${m}: its code wasn't fetched on its first visit`);
if (at(1).length) errors.push(`a second visit fetched ${at(1).join(", ")} again`);

// Left untouched, the window stops drawing (near 0 CPU): no frame asked for in 3 s.
await tour.mouse.move(990, 890);
await tour.waitForTimeout(22000);
const frames = await tour.evaluate(() => new Promise((done) => {
  let n = 0;
  const raf = window.requestAnimationFrame.bind(window);
  window.requestAnimationFrame = (fn) => { n++; return raf(fn); };
  setTimeout(() => done(n), 3000);
}));
console.log(`frames asked for while untouched: ${frames}`);
if (frames > 0) errors.push(`untouched for 22 s, Settings still asked for ${frames} frames in 3 s`);
await browser.close();
const slow = Object.entries(switches).filter(([, ms]) => ms >= 100);
console.log(`page switches (ms): ${Object.entries(switches).map(([k, v]) => `${k} ${v}`).join(", ")}`);
if (!Object.keys(switches).length) errors.push("no page switch was timed");
for (const [k, ms] of slow) errors.push(`switching to ${k} took ${ms} ms (100 ms at most)`);
console.log(`${places.length * 2} screenshots in ${out}`);
if (errors.length) {
  console.log(errors.join("\n"));
  process.exit(1);
}
