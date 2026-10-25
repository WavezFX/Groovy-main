// Round 3E item 9: Frost's water must fill his body at every angle. Renders
// the water layer alone for stages 1-6 at yaw -1, 0 and +1 and fails if any
// pixel inside his body, below the water line, is left unpainted; also
// screenshots the whole Groovy at those angles. (Ember's heat glow fades out
// by design; that it reaches as far as the sea is a unit test, finish.test.)
//   node scripts/water-check.mjs <base url> <out dir>
// Uses playwright-core with the installed Edge (CI) or $CHROME (elsewhere).

import { mkdirSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";

const [base = "http://127.0.0.1:1420", out = "water-check"] = process.argv.slice(2);
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
  const { drawSkinBody, styleAt } = await import("/src/mochi/skins.ts");
  const { makeHead } = await import("/src/mochi/outfits.ts");
  const { WATER_LEVEL } = await import("/src/mochi/finish.ts");
  const W = 240;
  const bad = [];
  const sheet = document.createElement("canvas");
  sheet.width = W * 3;
  sheet.height = W * 6;
  const sx = sheet.getContext("2d");
  sx.fillStyle = "#1b1d22";
  sx.fillRect(0, 0, sheet.width, sheet.height);
  const checks = [1, 2, 3, 4, 5, 6].map((stage) => ["frost", stage]);
  checks.forEach(([look, stage], row) => {
    [-1, 0, 1].forEach((yaw, col) => {
      const e = new BotEngine();
      e.setSkin(look, false);
      e.setStage(stage, false);
      const R = W * 0.3;
      const rx = R * 1.14 * e.shape.w;
      const ry = R * 0.88 * e.shape.h;
      const c = document.createElement("canvas");
      c.width = W;
      c.height = W;
      const x = c.getContext("2d");
      x.translate(W / 2, W / 2);
      const body = e.bodyPath(rx, ry, R);
      const H = { ...makeHead(R, yaw, 0), rx, ry };
      drawSkinBody(x, styleAt(look, stage), body, H, { hatless: 1, shown: 1, time: 3, lite: false });
      // Every pixel well inside his body, under the water line (with room for the wave), is painted.
      const level = WATER_LEVEL[stage] + 3;
      const levelY = ((level - 61) * ry) / 31;
      const data = x.getImageData(0, 0, W, W).data;
      const probe = document.createElement("canvas").getContext("2d");
      let empty = 0;
      for (let py = 0; py < W; py += 2) {
        for (let px = 0; px < W; px += 2) {
          const bx = px - W / 2;
          const by = py - W / 2;
          if (by < levelY) continue;
          // Inside, and not on the anti-aliased edge.
          if (![[0, 0], [2, 0], [-2, 0], [0, 2], [0, -2]].every(([dx, dy]) => probe.isPointInPath(body, bx + dx, by + dy))) continue;
          if (data[(py * W + px) * 4 + 3] === 0) empty++;
        }
      }
      if (empty) bad.push(`${look} stage ${stage}, yaw ${yaw}: ${empty} sampled pixels inside his body left blank`);
      // The whole Groovy at this angle, for the owner to see.
      e.yaw = yaw;
      e.tgYaw = yaw;
      sx.save();
      sx.translate(col * W, row * W);
      e.draw(sx, W, W);
      sx.restore();
    });
  });
  return { bad, sheet: sheet.toDataURL("image/png") };
});
const { writeFileSync } = await import("node:fs");
writeFileSync(join(out, "water-yaw-sheet.png"), Buffer.from(found.sheet.split(",")[1], "base64"));
await browser.close();
errors.push(...found.bad);
if (errors.length) {
  for (const e of errors) console.error(e);
  process.exit(1);
}
console.log("the water fills his body at yaw -1, 0 and +1, stages 1-6");
