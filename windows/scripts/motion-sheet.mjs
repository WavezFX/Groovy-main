// Round 4E item 1: contact sheet of Groovy's own motion (CHARACTER_MOTION.md §2).
// One sheet per look (Night, Ember, Frost); one row per state or emote, eight
// frames across its first 1.6 s, on a fixed clock so the frames are repeatable.
//   node scripts/motion-sheet.mjs <base url> <out dir>
// Uses playwright-core with $CHROME (or the installed Edge in CI).

import { mkdirSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";

const [base = "http://127.0.0.1:1420", out = "motion-sheet"] = process.argv.slice(2);
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || "playwright-core");

mkdirSync(out, { recursive: true });
const browser = await chromium.launch(process.env.CHROME ? { executablePath: process.env.CHROME } : { channel: "msedge" });
const page = await browser.newPage({ viewport: { width: 1200, height: 900 } });
const errors = [];
page.on("pageerror", (e) => errors.push(e.message));
await page.goto(`${base}/index.html`, { waitUntil: "load" });

for (const look of ["night", "ember", "frost"]) {
  const png = await page.evaluate(async (look) => {
    let T = 100000;
    performance.now = () => T;
    const { BotEngine } = await import("/src/mochi/engine.ts");
    const ROWS = [
      ["idle (breath)", (e) => e.setState("idle", true)],
      ["blink", (e) => e.blink()],
      ["happy (puff)", (e) => e.triggerEmote("happy")],
      ["surprised", (e) => e.triggerEmote("surprised")],
      ["love", (e) => e.triggerEmote("love")],
      ["proud", (e) => e.triggerEmote("proud")],
      ["wink", (e) => e.triggerEmote("wink")],
      ["yawn", (e) => e.triggerEmote("yawn")],
      ["annoyed", (e) => e.triggerEmote("annoyed")],
      ["hello", (e) => e.greet()],
      ["squash", (e) => e.squash()],
      ["working (knit)", (e) => e.setState("working")],
      ["thinking", (e) => e.setState("thinking")],
      ["searching", (e) => e.setState("searching")],
      ["question", (e) => e.setState("question")],
      ["approval", (e) => e.setState("approval")],
      ["finished", (e) => e.setState("finished")],
      ["sleeping", (e) => e.setState("sleeping")],
    ];
    const CELL = 120, LABEL = 130, FR = 8, SPAN = 1.6;
    const c = document.createElement("canvas");
    c.width = LABEL + CELL * FR;
    c.height = CELL * ROWS.length + 30;
    const x = c.getContext("2d");
    x.fillStyle = "#101418";
    x.fillRect(0, 0, c.width, c.height);
    x.fillStyle = "#cfe";
    x.font = "600 14px system-ui";
    x.fillText(`Groovy's motion: ${look}  (frames every ${(SPAN / (FR - 1)).toFixed(2)} s)`, 10, 20);
    ROWS.forEach(([name, start], row) => {
      const e = new BotEngine();
      e.setSkin(look, false);
      e.setStage(1, false);
      e.particleOverhang = 0;
      for (let i = 0; i < 30; i++) { T += 16; e.update(0.016); }
      start(e);
      let shot = 0;
      for (let f = 0; f <= Math.round(SPAN / 0.016); f++) {
        const tt = f * 0.016;
        if (tt >= (shot * SPAN) / (FR - 1) - 1e-6 && shot < FR) {
          x.save();
          x.translate(LABEL + shot * CELL, 30 + row * CELL);
          x.beginPath(); x.rect(0, 0, CELL, CELL); x.clip();
          e.applyDance(x, CELL, CELL);
          e.draw(x, CELL, CELL);
          x.restore();
          shot++;
        }
        T += 16;
        e.update(0.016);
      }
      x.fillStyle = "#cfe";
      x.font = "13px system-ui";
      x.fillText(name, 10, 30 + row * CELL + CELL / 2);
    });
    return c.toDataURL("image/png");
  }, look);
  writeFileSync(join(out, `motion-${look}.png`), Buffer.from(png.split(",")[1], "base64"));
  console.log(`wrote ${join(out, `motion-${look}.png`)}`);
}
await browser.close();
if (errors.length) {
  console.error(errors.join("\n"));
  process.exit(1);
}
