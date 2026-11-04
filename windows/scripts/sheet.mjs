// Dev helper: render chosen items at chosen poses, larger. node scripts/sheet.mjs URL out.png item,item yaw,yaw [pitch,pitch]
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || "playwright-core");
const [url, out, items, yaws, pitches = "0"] = process.argv.slice(2);
const b = await chromium.launch(process.env.CHROME ? { executablePath: process.env.CHROME } : { channel: "msedge" });
const page = await b.newPage();
await page.goto(url);
const png = await page.evaluate(async ([items, yaws, pitches]) => {
  const { BotEngine } = await import("/src/mochi/engine.ts");
  const S = 150;
  const poses = [];
  for (const p of pitches.split(",").map(Number)) for (const y of yaws.split(",").map(Number)) poses.push([y, p]);
  const list = items.split(",");
  const c = document.createElement("canvas");
  c.width = S * poses.length; c.height = S * list.length;
  const x = c.getContext("2d"); x.fillStyle = "#2a2d34"; x.fillRect(0, 0, c.width, c.height);
  list.forEach((it, r) => poses.forEach(([yaw, pitch], k) => {
    const e = new BotEngine(); e.setSkin("night", false); e.setStage(3, false); e.setOutfit(it, false);
    e.yaw = e.tgYaw = yaw; e.pitch = e.tgPitch = pitch;
    x.save(); x.translate(k * S, r * S); e.draw(x, S, S); x.restore();
  }));
  return c.toDataURL("image/png");
}, [items, yaws, pitches]);
(await import("node:fs")).writeFileSync(out, Buffer.from(png.split(",")[1], "base64"));
await b.close();
