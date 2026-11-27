// Round 4E item 4: draws the app and tray icon from Groovy's own engine (Night
// look, pillow body and leaf wisps) at 1024 px, on the dark rounded tile.
//   node scripts/gen-icons.mjs <base url> <out png>
// then: python3 scripts/gen-icons.py <out png>   (sizes and icon.ico)
// Uses playwright-core with $CHROME (or the installed Edge).
import { writeFileSync } from "node:fs";
import { createRequire } from "node:module";

const [base = "http://127.0.0.1:1420", out = "src-tauri/icons/icon-1024.png"] = process.argv.slice(2);
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || "playwright-core");
const browser = await chromium.launch(process.env.CHROME ? { executablePath: process.env.CHROME } : { channel: "msedge" });
const page = await browser.newPage();
await page.goto(`${base}/index.html`, { waitUntil: "load" });
const png = await page.evaluate(async () => {
  const T = 100000;
  performance.now = () => T;
  const { BotEngine } = await import("/src/mochi/engine.ts");
  const S = 1024;
  const c = document.createElement("canvas");
  c.width = c.height = S;
  const x = c.getContext("2d");
  // The tile: near-black with a soft green light from below.
  const r = S * 0.22;
  x.beginPath();
  x.roundRect(S * 0.04, S * 0.04, S * 0.92, S * 0.92, r);
  const g = x.createRadialGradient(S / 2, S * 0.62, S * 0.05, S / 2, S * 0.55, S * 0.62);
  g.addColorStop(0, "#14301f");
  g.addColorStop(1, "#070b09");
  x.fillStyle = g;
  x.fill();
  const e = new BotEngine();
  e.setSkin("night", false);
  e.setStage(0, false);
  e.hands = 1;
  for (let i = 0; i < 10; i++) e.update(0.016);
  x.save();
  x.translate(S * 0.14, S * 0.16);
  e.draw(x, S * 0.72, S * 0.72);
  x.restore();
  return c.toDataURL("image/png");
});
writeFileSync(out, Buffer.from(png.split(",")[1], "base64"));
await browser.close();
console.log(`wrote ${out}`);
