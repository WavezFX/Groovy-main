// Round 3E item 1: the island's tiles at their normal size, dark and light,
// both tile pages, with long app names. Fails if a tile's title is clipped
// when it should fit, any text leaves its tile, or a control on the tiles
// has a hit area under 28 px. Round 3E item 8: also the island's wardrobe
// (every item, the locked ones dashed) and Groovy's final form in each look.
//   node scripts/island-screens.mjs <base url> <out dir>
// Uses playwright-core with the installed Edge (CI) or $CHROME (elsewhere).

import { mkdirSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";

const [base = "http://127.0.0.1:1420", out = "island-screens"] = process.argv.slice(2);
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || "playwright-core");

/** Names that must show whole on a 1-tile face. */
const MUST_FIT = ["ArtCraft", "VS Code", "Proton VPN", "Reminders", "This PC"];

const app = (label, target) => ({ id: `app:${target}`, kind: "app", target, label, size: "compact" });
const TILES = {
  preset: "custom",
  mailApps: [],
  tiles: [
    { id: "music", kind: "music", target: "", label: "", size: "compact" },
    { id: "reminders", kind: "reminders", target: "", label: "", size: "compact" },
    { id: "system", kind: "system", target: "", label: "", size: "compact" },
    app("", "C:\\Users\\me\\Desktop\\ArtCraft Launcher.lnk"),
    app("", "C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs\\Visual Studio Code.lnk"),
    app("", "C:\\Program Files\\Proton\\VPN\\Proton VPN.exe"),
    app("", "C:\\Program Files\\Blender Foundation\\Blender 4.2\\Blender Launcher.exe"),
    app("", "C:\\Program Files\\Mozilla Firefox\\Mozilla Firefox.lnk"),
    app("", "C:\\Program Files\\Obsidian\\Obsidian.exe"),
    { id: "site:https://github.com", kind: "site", target: "https://github.com", label: "", size: "compact" },
  ],
};

mkdirSync(out, { recursive: true });
const browser = await chromium.launch(process.env.CHROME ? { executablePath: process.env.CHROME } : { channel: "msedge" });
const errors = [];
for (const scheme of ["dark", "light"]) {
  const page = await browser.newPage({ viewport: { width: 1000, height: 420 }, colorScheme: scheme });
  page.on("pageerror", (e) => errors.push(`${scheme}: ${e.message}`));
  await page.goto(`${base}/index.html`, { waitUntil: "load" });
  await page.waitForFunction(() => window.groovy?.island, null, { timeout: 20000 });
  await page.evaluate((tiles) => {
    const { island, State, Spotify } = window.groovy;
    State.settings.islandTiles = tiles;
    State.settings.activeIntegrations = [...new Set([...(State.settings.activeIntegrations ?? []), "integration_spotify"])];
    Spotify.state = { ...Spotify.state, running: true, installed: true, playing: true, position: 10, positionAt: Date.now(), track: { id: "spotify:track:t", title: "A song with a rather long title", artist: "Somebody", album: "", duration: 200, artUrl: null } };
    island.setPinned(true);
    island.alert("home");
  }, TILES);
  await page.waitForTimeout(1500);
  for (const n of [1, 2]) {
    if (n === 2) {
      const dots = await page.$$(".home-dots button");
      if (dots.length < 2) {
        errors.push(`${scheme}: one page of tiles only, the switcher can't be checked`);
        break;
      }
      await dots[1].click();
      await page.waitForTimeout(700);
    }
    await page.screenshot({ path: join(out, `island-page${n}-${scheme}.png`) });
    const found = await page.evaluate((mustFit) => {
      const bad = [];
      for (const cell of document.querySelectorAll(".home-view .home-cell")) {
        const box = cell.getBoundingClientRect();
        for (const el of cell.querySelectorAll("b, span")) {
          const r = el.getBoundingClientRect();
          if (!r.width) continue;
          if (r.left < box.left - 0.5 || r.right > box.right + 0.5 || r.bottom > box.bottom + 0.5) bad.push(`"${el.textContent}" leaves its tile`);
        }
        const title = cell.querySelector(".home-cell-text b");
        if (title && mustFit.includes(title.textContent) && title.scrollWidth > title.clientWidth + 0.5) bad.push(`"${title.textContent}" is cut off`);
      }
      for (const b of document.querySelectorAll(".home-view button, .home-view [role=button]")) {
        const r = b.getBoundingClientRect();
        if (r.width && (r.width < 28 || r.height < 28)) bad.push(`${b.getAttribute("aria-label") || b.className || b.tagName}: ${Math.round(r.width)} × ${Math.round(r.height)} px`);
      }
      const titles = [...document.querySelectorAll(".home-view .home-cell-text b")].map((b) => b.textContent);
      return { bad, titles };
    }, MUST_FIT);
    console.log(`${scheme} page ${n}: ${found.titles.join(" · ")}`);
    for (const b of found.bad) errors.push(`${scheme} page ${n}: ${b}`);
  }
  // The wardrobe on the island, and stage 6 for Night, Ember and Frost.
  const sixMonths = Math.floor(Date.now() / 1000) - 200 * 86400;
  for (const look of ["night", "ember", "frost"]) {
    await page.evaluate(([look, hatchedAt]) => {
      const { island, State } = window.groovy;
      State.settings.groovyLook = look;
      State.settings.growth = { activeSeconds: 40 * 86400, reached: 6, seen: 6, hatchedAt, correction: 1 };
      State.notify();
      island.alert("wardrobe");
    }, [look, sixMonths]);
    await page.waitForTimeout(900);
    await page.screenshot({ path: join(out, `island-wardrobe-${look}-stage6-${scheme}.png`) });
    const shown = await page.evaluate(() => ({
      items: document.querySelectorAll(".wardrobe-slots .wardrobe-item").length,
      locked: [...document.querySelectorAll(".wardrobe-slots .wardrobe-item.locked")].map((b) => b.title),
    }));
    // 45 items and 3 Nones; at stage 6 only seasonal items not earned yet can be locked.
    if (shown.items !== 48) errors.push(`${scheme} ${look}: the wardrobe shows ${shown.items} buttons, not 48`);
    for (const title of shown.locked) if (/Stage \d/.test(title)) errors.push(`${scheme} ${look}: "${title}" is still locked at stage 6`);
  }
  await page.close();
}
await browser.close();
if (errors.length) {
  for (const e of errors) console.error(e);
  process.exit(1);
}
console.log("island tiles: every title fits, every control at least 28 px; the wardrobe and stage 6 drawn for every look");
