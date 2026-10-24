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
await browser.close();
console.log(`${places.length * 2} screenshots in ${out}`);
if (errors.length) {
  console.log(errors.join("\n"));
  process.exit(1);
}
