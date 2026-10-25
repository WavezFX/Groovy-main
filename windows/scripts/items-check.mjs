// Round 3E item 10: every wardrobe item fits and follows his head. Renders
// each of the 45 items on Groovy at yaw -1, -0.5, 0, 0.5, 1 and pitch -0.3,
// +0.3, for all three looks and stages 0-6, and fails when an item leaves its
// place: face items off his eyes, a hat floating above or sunk into his head,
// a neck item off his front, anything sliding off his body, by more than 6 %
// of his width. Writes a contact sheet (one look, stage 3) for the owner.
//   node scripts/items-check.mjs <base url> <out dir>
// Uses playwright-core with the installed Edge (CI) or $CHROME (elsewhere).

import { mkdirSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";

const [base = "http://127.0.0.1:1420", out = "items-check"] = process.argv.slice(2);
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
  const { ACCESSORIES } = await import("/src/mochi/wardrobe.ts");
  const { SLOT_OF } = await import("/src/mochi/appearance.ts");
  const { drawOutfitBehind, drawOutfitFront, makeHead, itemArt } = await import("/src/mochi/outfits.ts");
  const POSES = [[-1, 0], [-0.5, 0], [0, 0], [0.5, 0], [1, 0], [0, -0.3], [0, 0.3]];
  const W = 160;
  const bad = [];
  let renders = 0;
  const probe = document.createElement("canvas");
  probe.width = W;
  probe.height = W;
  const p = probe.getContext("2d", { willReadFrequently: true });

  /** The item's own pixels, drawn alone: their box in body space, or null. */
  function itemBox(e, item, yaw, pitch) {
    const R = W * 0.3;
    const rx = R * 1.14 * e.shape.w;
    const ry = R * 0.88 * e.shape.h;
    e.yaw = yaw;
    e.pitch = pitch;
    const H = { ...makeHead(R, yaw, pitch, 0, 0), rx, ry, eyes: e.eyeCentres(rx, ry) };
    p.setTransform(1, 0, 0, 1, 0, 0);
    p.clearRect(0, 0, W, W);
    p.translate(W / 2, W / 2);
    const st = { presence: 1, morph: 0 };
    drawOutfitBehind(p, item, H, st);
    drawOutfitFront(p, item, H, st);
    renders++;
    const d = p.getImageData(0, 0, W, W).data;
    let x0 = W, y0 = W, x1 = -1, y1 = -1;
    for (let y = 0; y < W; y++) for (let x = 0; x < W; x++) if (d[(y * W + x) * 4 + 3] > 40) {
      if (x < x0) x0 = x;
      if (x > x1) x1 = x;
      if (y < y0) y0 = y;
      if (y > y1) y1 = y;
    }
    if (x1 < 0) return null;
    const [l, r] = H.eyes;
    return { x0: x0 - W / 2, x1: x1 - W / 2, y0: y0 - W / 2, y1: y1 - W / 2, rx, ry, eyeX: (l[0] + r[0]) / 2, eyeY: (l[1] + r[1]) / 2 };
  }

  for (const look of ["night", "ember", "frost"]) {
    for (let stage = 0; stage <= 6; stage++) {
      const e = new BotEngine();
      e.setSkin(look, false);
      e.setStage(stage, false);
      for (const item of ACCESSORIES) {
        if (!itemArt(item)) continue;
        const slot = SLOT_OF[item];
        const ref = itemBox(e, item, 0, 0);
        if (!ref) {
          bad.push(`${item}: draws nothing`);
          continue;
        }
        // A hat touches the top of his head (the cloud buddy floats, by design).
        if (slot === "head" && item !== "cloudBuddy" && ref.y1 < -ref.ry - 0.06 * 2 * ref.rx) {
          bad.push(`${look} ${stage} ${item}: floats ${Math.round(-ref.ry - ref.y1)} px above his head`);
        }
        for (const [yaw, pitch] of POSES) {
          const b = itemBox(e, item, yaw, pitch);
          const where = `${look} ${stage} ${item} at yaw ${yaw}, pitch ${pitch}`;
          if (!b) {
            // Turned away, his glasses are behind him: drawn behind, still there.
            bad.push(`${where}: vanished`);
            continue;
          }
          const width = 2 * b.rx;
          const tol = 0.06 * width;
          const cx = (b.x0 + b.x1) / 2;
          const refCx = (ref.x0 + ref.x1) / 2;
          if (slot === "face") {
            // On his eyes: the item's middle keeps its place against their middle.
            const k = Math.max(0.6, Math.cos(yaw));
            const want = b.eyeX + (refCx - ref.eyeX) * k;
            if (Math.abs(cx - want) > tol) bad.push(`${where}: ${Math.round(Math.abs(cx - want))} px off his eyes`);
            const dy = (b.y0 + b.y1) / 2 - b.eyeY - ((ref.y0 + ref.y1) / 2 - ref.eyeY);
            if (Math.abs(dy) > tol) bad.push(`${where}: ${Math.round(Math.abs(dy))} px above or below his eyes`);
          } else {
            // Hats keep their seat on his head, neck items on his front: their
            // lower (hats) or upper (neck) edge stays where it was, give or take the nod.
            const edge = slot === "head" ? b.y1 - ref.y1 : b.y0 - ref.y0;
            const nod = Math.abs(Math.sin(pitch)) * b.ry * 0.35;
            if (Math.abs(edge) > tol + nod) bad.push(`${where}: ${edge < 0 ? "floats" : "sinks"} ${Math.round(Math.abs(edge))} px`);
          }
          // Never slides off him: its middle stays over his body.
          if (Math.abs(cx) > b.rx + tol) bad.push(`${where}: slid off his body`);
        }
      }
    }
  }

  // The contact sheet: Night at stage 3, every item, every pose.
  const S = 96;
  const sheet = document.createElement("canvas");
  sheet.width = S * POSES.length;
  sheet.height = S * ACCESSORIES.length;
  const sx = sheet.getContext("2d");
  sx.fillStyle = "#1b1d22";
  sx.fillRect(0, 0, sheet.width, sheet.height);
  ACCESSORIES.forEach((item, row) => {
    POSES.forEach(([yaw, pitch], col) => {
      const e = new BotEngine();
      e.setSkin("night", false);
      e.setStage(3, false);
      e.setOutfit(item, false);
      e.yaw = e.tgYaw = yaw;
      e.pitch = e.tgPitch = pitch;
      sx.save();
      sx.translate(col * S, row * S);
      e.draw(sx, S, S);
      sx.restore();
    });
  });
  return { bad, sheet: sheet.toDataURL("image/png"), renders };
});
writeFileSync(join(out, "items-sheet.png"), Buffer.from(found.sheet.split(",")[1], "base64"));
await browser.close();
// One line per item and problem is enough to read.
const seen = new Set();
for (const b of found.bad) {
  const key = b.replace(/^(\w+) \d /, "").replace(/ at yaw.*?:/, ":").replace(/\d+ px/, "");
  if (!seen.has(key)) errors.push(b);
  seen.add(key);
}
if (errors.length) {
  for (const e of errors) console.error(e);
  console.error(`${found.bad.length} problems in all`);
  process.exit(1);
}
console.log(`every item fits and follows his head: 45 items, 3 looks, 7 stages, 7 poses (${found.renders} renders)`);
