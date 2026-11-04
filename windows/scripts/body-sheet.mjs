// Round 3K item 4: the contact sheet of Groovy's pillow body and element wisps
// for the owner. Each look at stages 1 to 6 with the expressions idle, happy,
// thinking, surprised and sleepy at the open island's size, then collapsed
// (no wisps at that size); one row per look with a hat and one holding (the
// working pose). Fails on a page error or when the wisps don't show at the
// open size.
//   node scripts/body-sheet.mjs <base url> <out dir>
// Uses playwright-core with the installed Edge (CI) or $CHROME (elsewhere).

import { mkdirSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";

const [base = "http://127.0.0.1:1420", out = "body-sheet"] = process.argv.slice(2);
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || "playwright-core");

mkdirSync(out, { recursive: true });
const browser = await chromium.launch(process.env.CHROME ? { executablePath: process.env.CHROME } : { channel: "msedge" });
const page = await browser.newPage({ viewport: { width: 1200, height: 900 } });
const errors = [];
page.on("pageerror", (e) => errors.push(e.message));
await page.goto(`${base}/index.html`, { waitUntil: "load" });
await page.waitForFunction(() => window.groovy?.island, null, { timeout: 20000 });

const found = await page.evaluate(async () => {
  const { BotEngine } = await import("/src/mochi/engine.ts");
  const OPEN = 132;
  const SMALL = 40;
  const EXPR = [
    ["idle", (e) => e.setState("idle", true)],
    ["happy", (e) => e.triggerEmote("happy", 10)],
    ["thinking", (e) => e.setState("thinking", true)],
    ["surprised", (e) => e.triggerEmote("surprised", 10)],
    ["sleepy", (e) => e.setState("sleeping", true)],
  ];
  const LOOKS = ["night", "ember", "frost"];
  const rows = [];
  for (const look of LOOKS) for (let stage = 1; stage <= 6; stage++) rows.push({ look, stage, label: `${look} ${stage}` });
  for (const look of LOOKS) rows.push({ look, stage: 3, outfit: "acornCap", label: `${look} hat` });
  for (const look of LOOKS) rows.push({ look, stage: 3, working: true, label: `${look} holding` });
  const sheet = document.createElement("canvas");
  sheet.width = 90 + OPEN * EXPR.length + SMALL + 20;
  sheet.height = OPEN * rows.length;
  const sx = sheet.getContext("2d");
  sx.fillStyle = "#1b1d22";
  sx.fillRect(0, 0, sheet.width, sheet.height);
  sx.font = "13px sans-serif";
  const make = (r, expr) => {
    const e = new BotEngine();
    e.setSkin(r.look, false);
    e.setStage(r.stage, false);
    if (r.outfit) e.setOutfit(r.outfit, false);
    if (r.working) e.setState("working", true);
    else expr?.(e);
    for (let i = 0; i < 90; i++) e.update(1 / 60);
    return e;
  };
  // The wisps at the open size: the leaf, flame or droplet pixels beside his body.
  const probe = document.createElement("canvas");
  probe.width = probe.height = OPEN;
  const pc = probe.getContext("2d", { willReadFrequently: true });
  const noWisps = [];
  rows.forEach((r, row) => {
    sx.fillStyle = "#cfd3da";
    sx.fillText(r.label, 6, row * OPEN + OPEN / 2);
    EXPR.forEach(([, fn], col) => {
      const e = make(r, fn);
      sx.save();
      sx.translate(90 + col * OPEN, row * OPEN);
      e.draw(sx, OPEN, OPEN);
      sx.restore();
    });
    const e = make(r, EXPR[0][1]);
    sx.save();
    sx.translate(90 + OPEN * EXPR.length + 10, row * OPEN + (OPEN - SMALL) / 2);
    e.draw(sx, SMALL, SMALL);
    sx.restore();
    if (r.outfit || r.working) return;
    pc.clearRect(0, 0, OPEN, OPEN);
    e.draw(pc, OPEN, OPEN);
    // Outside his body's sides, low down: only a wisp is drawn there.
    const d = pc.getImageData(0, Math.round(OPEN * 0.55), Math.round(OPEN * 0.12), Math.round(OPEN * 0.3)).data;
    let lit = 0;
    for (let i = 3; i < d.length; i += 4) if (d[i] > 160) lit++;
    if (lit < 12) noWisps.push(r.label);
  });
  return { sheet: sheet.toDataURL("image/png"), noWisps, rows: rows.length };
});
writeFileSync(join(out, "body-sheet.png"), Buffer.from(found.sheet.split(",")[1], "base64"));
await browser.close();
for (const r of found.noWisps) errors.push(`${r}: the wisps don't show at the open island size`);
if (errors.length) {
  for (const e of errors) console.error(e);
  process.exit(1);
}
console.log(`body contact sheet: ${found.rows} rows, 5 expressions, open and collapsed -> ${join(out, "body-sheet.png")}`);
