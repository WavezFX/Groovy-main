// Round 3H item 2: Settings → Groovy, one look switch and a clean growth
// screen. Screenshots the page for each look (Night growing, Ember and Frost
// not started) at the smallest and the normal window width, dark and light;
// fails if starting a stage preview moves anything on the page, if the stage
// strip wraps or overlaps, if the page has a second look switch, or if the
// frames while previewing are slow (the frame test's own limits).
//   node scripts/growth-screens.mjs <base url> <out dir>
// Uses playwright-core with the installed Edge (CI) or $CHROME (elsewhere).

import { mkdirSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";

const [base = "http://127.0.0.1:1420", out = "growth-screens"] = process.argv.slice(2);
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || "playwright-core");

/** The Settings window: its smallest size (lib.rs min_inner_size) and the one it opens at. */
const WIDTHS = { min: [460, 900], normal: [920, 1300] };
const LOOKS = ["night", "ember", "frost"];

mkdirSync(out, { recursive: true });
const browser = await chromium.launch(process.env.CHROME ? { executablePath: process.env.CHROME } : { channel: "msedge" });
const errors = [];

/** Mounts the Groovy page on settings that have grown Night for 4 days (Ember and Frost not started). */
async function mount(page) {
  await page.goto(`${base}/settings.html?page=wardrobe`, { waitUntil: "load" });
  await page.evaluate(() => document.body.classList.add("shown"));
  await page.waitForSelector("#wardrobe", { timeout: 20000 });
  await page.evaluate(async () => {
    // Served by vite, in the page (not files for knip to follow).
    const SRC = "/src/";
    const { DEFAULT_SETTINGS } = await import(SRC + "core/state.ts");
    const { wardrobeSection } = await import(SRC + "settings/wardrobe.ts");
    const now = Math.floor(Date.now() / 1000);
    const settings = {
      ...DEFAULT_SETTINGS,
      groovyLook: "night",
      hatchDone: true,
      growths: { night: { activeSeconds: 4 * 86400, reached: 2, seen: 2, shown: null, hatchedAt: now - 9 * 86400, correction: 2 } },
    };
    const old = document.getElementById("wardrobe");
    old.replaceWith(wardrobeSection(settings, () => {}));
  });
  await page.waitForTimeout(600);
}

for (const scheme of ["dark", "light"]) {
  for (const [size, [w, hgt]] of Object.entries(WIDTHS)) {
    const page = await browser.newPage({ viewport: { width: w, height: hgt }, colorScheme: scheme });
    page.on("pageerror", (e) => errors.push(`${scheme} ${size}: ${e.message}`));
    await mount(page);
    const where = `${scheme}, ${size} width`;
    const switches = await page.$$eval("#wardrobe [role=tablist][aria-label], #wardrobe [role=radiogroup]", (els) => els.filter((e) => e.querySelector(".hub-w-look")).length);
    if (switches !== 1) errors.push(`${where}: ${switches} look switches (one only)`);
    for (const look of LOOKS) {
      await page.click(`#wardrobe .hub-w-look:nth-child(${LOOKS.indexOf(look) + 1})`);
      await page.waitForTimeout(500);
      const state = await page.evaluate(() => {
        const cells = [...document.querySelectorAll("#wardrobe .hub-w-strip-stage")].map((c) => c.getBoundingClientRect());
        const rows = new Set(cells.map((r) => Math.round(r.top))).size;
        const overlap = cells.some((r, i) => i > 0 && r.left < cells[i - 1].right - 0.5);
        const widths = cells.map((r) => Math.round(r.width));
        const page = document.documentElement;
        return {
          cells: cells.length, rows, overlap, equal: Math.max(...widths) - Math.min(...widths) <= 1,
          wearBtn: !!document.querySelector("#wardrobe .hub-w-wear button"),
          wearing: document.querySelector("#wardrobe .hub-w-look.on .hub-w-wearing") != null,
          tabState: document.querySelector("#wardrobe .hub-w-look.on small")?.textContent ?? "",
          sideways: page.scrollWidth > page.clientWidth + 1,
        };
      });
      const at = `${where}, ${look}`;
      if (state.cells !== 6 || state.rows !== 1 || state.overlap || !state.equal) errors.push(`${at}: the stage strip isn't six equal cells in one row (${JSON.stringify(state)})`);
      if (state.sideways) errors.push(`${at}: the page scrolls sideways`);
      // Viewing isn't wearing: only another look offers "Wear this look".
      if (state.wearBtn === (look === "night")) errors.push(`${at}: "Wear this look" ${state.wearBtn ? "shown for the look he wears" : "missing"}`);
      if (state.wearing !== (look === "night")) errors.push(`${at}: the "Wearing" badge is on the wrong look`);
      if (look !== "night" && state.tabState !== "Not started") errors.push(`${at}: its tab says "${state.tabState}", not "Not started"`);
      await page.screenshot({ path: join(out, `groovy-${look}-${size}-${scheme}.png`), fullPage: true });
    }

    // A stage preview starts and ends: nothing on the page moves.
    await page.click("#wardrobe .hub-w-look:nth-child(1)");
    await page.waitForTimeout(400);
    const boxes = () => page.evaluate(() => {
      // Against the page itself: clicking a cell may scroll it into view.
      const o = document.getElementById("wardrobe").getBoundingClientRect();
      return [...document.querySelectorAll("#wardrobe *")].filter((e) => e.offsetParent !== null && !e.closest(".hub-w-preview-only")).map((e) => {
        const r = e.getBoundingClientRect();
        return `${Math.round(r.left - o.left)},${Math.round(r.top - o.top)},${Math.round(r.width)},${Math.round(r.height)}`;
      });
    });
    const before = await boxes();
    // Frames while previewing: every frame's time, the same limits as the frame test.
    await page.evaluate(() => {
      const d = [];
      let last = performance.now();
      const step = (now) => {
        d.push(now - last);
        last = now;
        if (d.length < 600) window.__previewRaf = requestAnimationFrame(step);
      };
      window.__previewFrames = d;
      requestAnimationFrame((now) => {
        last = now;
        requestAnimationFrame(step);
      });
    });
    await page.click("#wardrobe .hub-w-strip-stage.locked");
    await page.waitForTimeout(400);
    const during = await boxes();
    if (!(await page.isVisible("#wardrobe .hub-w-preview-only"))) errors.push(`${where}: a locked stage didn't say "Preview only"`);
    await page.waitForTimeout(2600);
    const deltas = await page.evaluate(() => window.__previewFrames.slice(2));
    await page.waitForTimeout(2600);
    const after = await boxes();
    const moved = (a, b) => a.length !== b.length || a.some((v, i) => v !== b[i]);
    if (moved(before, during)) errors.push(`${where}: the page moved when a preview started`);
    if (moved(before, after)) errors.push(`${where}: the page moved when the preview ended`);
    const { frameStats, frameVerdict } = await import("../src/core/frametest.ts");
    const stats = frameStats(deltas);
    const verdict = frameVerdict(stats);
    console.log(`${where}: frames while previewing: ${JSON.stringify(stats)}`);
    if (verdict) errors.push(`${where}: previewing: ${verdict}`);
    await page.close();
  }
}
await browser.close();
if (errors.length) {
  for (const e of errors) console.error(e);
  process.exit(1);
}
console.log(`the Groovy page: one look switch, no layout shift, 3 looks × 2 widths × 2 schemes in ${out}`);
