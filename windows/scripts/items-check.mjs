// Round 3E item 10: every wardrobe item fits and follows his head. Renders
// each of the 45 items on Groovy at yaw -1, -0.5, 0, 0.5, 1 and pitch -0.3,
// +0.3, for all three looks and stages 0-6, and fails when an item leaves its
// place: face items off his eyes, a hat floating above or sunk into his head,
// a neck item off his front, anything sliding off his body, by more than 6 %
// of his width. Writes a contact sheet (one look, stage 3) for the owner.
//   node scripts/items-check.mjs <base url> <out dir>
// Uses playwright-core with the installed Edge (CI) or $CHROME (elsewhere).

import { mkdirSync, writeFileSync, appendFileSync } from "node:fs";
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
  // Served by vite, in the page (not files for knip to follow).
  const SRC = "/src/mochi/";
  const { BotEngine } = await import(SRC + "engine.ts");
  const { ACCESSORIES } = await import(SRC + "wardrobe.ts");
  const { SLOT_OF } = await import(SRC + "appearance.ts");
  const { drawOutfitBehind, drawOutfitFront, makeHead, itemArt, headAnchors, placeItem, ringPoints } = await import(SRC + "outfits.ts");
  const POSES = [[-1, 0], [-0.6, 0], [-0.5, 0], [0, 0], [0.5, 0], [0.6, 0], [1, 0], [0, -0.3], [0, 0.3]];
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
    // Glasses' arms run back behind him: the lenses are what sits on his eyes.
    if (SLOT_OF[item] !== "face" || Math.cos(yaw) < 0) drawOutfitBehind(p, item, H, st);
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

  // Round 3H item 1, measured: a hat's rim ends lie on his outline (2 %), a
  // neck item's collar ends too (3 %), and a lens centre sits on its eye (2 % from -0.7 to 0.7, 3 % beyond, as a warning),
  // at yaw -1…1 and pitch -0.3…0.3. Nothing in an item is clipped to his body.
  const FIT = [];
  for (const yaw of [-1, -0.7, -0.5, 0, 0.5, 0.7, 1]) for (const pitch of [-0.3, 0, 0.3]) FIT.push([yaw, pitch]);
  for (const look of ["night", "ember", "frost"]) {
    const e = new BotEngine();
    e.setSkin(look, false);
    for (const [yaw, pitch] of FIT) {
      const R = W * 0.3;
      const rx = R * 1.14 * e.shape.w;
      const ry = R * 0.88 * e.shape.h;
      e.yaw = yaw;
      e.pitch = pitch;
      const H = { ...makeHead(R, yaw, pitch, 0, 0), rx, ry, eyes: e.eyeCentres(rx, ry) };
      const A = headAnchors(H);
      const body = e.bodyPath(rx, ry, R);
      p.setTransform(1, 0, 0, 1, 0, 0);
      const where = `${look} at yaw ${yaw}, pitch ${pitch}`;
      const onOutline = (name, [px, py], share) => {
        const t = share * 2 * rx;
        const sd = Math.sign(px) || 1;
        if (!p.isPointInPath(body, px - sd * t, py) || p.isPointInPath(body, px + sd * t, py)) bad.push(`${where}: ${name} is off his outline by more than ${share * 100} %`);
      };
      onOutline("the hat rim's left end", A.rimLeft, 0.02);
      onOutline("the hat rim's right end", A.rimRight, 0.02);
      onOutline("the collar's left end", [A.collar.cx - A.collar.a, A.collar.cy], 0.03);
      onOutline("the collar's right end", [A.collar.cx + A.collar.a, A.collar.cy], 0.03);
      for (const item of ACCESSORIES) {
        if (SLOT_OF[item] !== "face") continue;
        p.setTransform(1, 0, 0, 1, 0, 0);
        placeItem(p, item, H);
        const m = p.getTransform();
        H.eyes.forEach(([ex, ey], i) => {
          const c = m.transformPoint({ x: i ? 62 : 38, y: 62 });
          const off = Math.hypot(c.x - ex, c.y - ey) / (2 * rx);
          if (off > (Math.abs(yaw) > 0.7 ? 0.03 : 0.02)) bad.push(`${Math.abs(yaw) > 0.7 ? "warning: " : ""}${where}: ${item}'s ${i ? "right" : "left"} lens is ${Math.round(off * 100)} % off its eye`);
        });
      }
    }
  }
  p.setTransform(1, 0, 0, 1, 0, 0);

  // Round 3L: no face item reaches past his body width at any turn from -0.7 to 0.7.
  for (const look of ["night", "ember", "frost"]) {
    const e = new BotEngine();
    e.setSkin(look, false);
    for (const item of ACCESSORIES) {
      if (SLOT_OF[item] !== "face" || !itemArt(item)) continue;
      for (const yaw of [-0.7, -0.35, 0, 0.35, 0.7]) {
        const b = itemBox(e, item, yaw, 0);
        const over = b ? Math.round((Math.max(-b.x0, b.x1) / b.rx - 1) * 100) : 0;
        if (b && over > 2) bad.push(`${look} ${item} at yaw ${yaw}: reaches past his body width by ${over} %`);
      }
    }
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
            // On his eyes, sideways: the lens test above holds each lens to its eye (2 %) and
            // the outline test keeps the outer edges inside him. The box's middle is no
            // longer compared with the eyes' middle: a patch or visor strap is
            // foreshortened with the turn, so its middle legitimately moves off the eyes'.
            // Up and down: it keeps its height against the eyes.
            const warn = Math.abs(yaw) > 0.7 ? "warning: " : "";
            // The top edge, not the middle: a dashed monocle chain can lose its last dash between poses.
            const dy = b.y0 - b.eyeY - (ref.y0 - ref.eyeY);
            if (Math.abs(dy) > tol) bad.push(`${warn}${where}: ${Math.round(Math.abs(dy))} px above or below his eyes`);
          } else {
            // Hats keep their seat on his head, neck items on his front: their
            // lower (hats) or upper (neck) edge stays where it was, give or take the nod.
            const edge = slot === "head" ? b.y1 - ref.y1 : b.y0 - ref.y0;
            // Round 3M: his head is a 3D model, so a nod moves the front of a ring by depth × (sin(tilt) - sin(rest tilt)).
            // A wide brim is a wider ring, so its front moves further with a nod.
            const brim = { strawSunHat: 1.9, bucketHat: 1.42 }[item] ?? 1;
            const nod = 0.8 * b.rx * brim * Math.abs(Math.sin(0.2 - pitch) - Math.sin(0.2)) * 1.15;
            if (Math.abs(edge) > tol + nod) bad.push(`${Math.abs(yaw) > 0.7 ? "warning: " : ""}${where}: ${edge < 0 ? "floats" : "sinks"} ${Math.round(Math.abs(edge))} px`);
          }
          // Never slides off him: its middle stays over his body.
          if (Math.abs(cx) > b.rx + tol) bad.push(`${where}: slid off his body`);
        }
      }
    }
  }

  // Round 3M item 5, the tests the owner asked for, at yaw -0.6..0.6 and pitch -0.2..0.2.
  const LENSLIKE = new Set(["roundSpecs", "shades", "heartGlasses", "glowVisor", "monocle", "piratePatch", "pilotGoggles"]);
  const COVER = [[-0.6, -0.2], [-0.6, 0.2], [0, -0.2], [0, 0.2], [0.6, -0.2], [0.6, 0.2], [0, 0]];
  for (const look of ["night", "ember", "frost"]) {
    const e = new BotEngine();
    e.setSkin(look, false);
    for (const item of ACCESSORIES) {
      if (!itemArt(item)) continue;
      const slot = SLOT_OF[item];
      for (const [yaw, pitch] of COVER) {
        const R = W * 0.3;
        const rx = R * 1.14 * e.shape.w;
        const ry = R * 0.88 * e.shape.h;
        e.yaw = yaw;
        e.pitch = pitch;
        const H = { ...makeHead(R, yaw, pitch, 0, 0), rx, ry, eyes: e.eyeCentres(rx, ry) };
        const A = headAnchors(H);
        p.setTransform(1, 0, 0, 1, 0, 0);
        p.clearRect(0, 0, W, W);
        p.translate(W / 2, W / 2);
        const st = { presence: 1, morph: 0 };
        drawOutfitFront(p, item, H, st);
        const alphaAt = (x, y) => p.getImageData(Math.round(W / 2 + x), Math.round(W / 2 + y), 1, 1).data[3];
        const where = `${look} ${item} at yaw ${yaw}, pitch ${pitch}`;
        // No item covers either eye, except lenses and the patch.
        if (!LENSLIKE.has(item)) {
          for (const [ex, ey] of H.eyes) if (alphaAt(ex, ey) > 60) bad.push(`${where}: covers his eye`);
        }
        // No gap between a hat and the crown: just above the rim's front arc the hat is solid.
        if (["acornCap", "mushroomCap", "pumpkinCap", "bobbleBeanie", "eggshellHat", "watermelonCap", "bucketHat", "backwardsCap", "strawSunHat"].includes(item)) {
          const front = ringPoints(H, A.rim.cy, A.rim.a).filter((q) => q.z > 0 && Math.abs(q.x) < A.rim.a * 0.6);
          for (const q of front) if (alphaAt(q.x + A.rim.cx, q.y - 3) < 200) bad.push(`${where}: a gap between the hat and his crown at x ${Math.round(q.x)}`);
        }
        // No neck item above the neck line (and so never over his eyes).
        if (slot === "neck" && item !== "poolRing") {
          const d = p.getImageData(0, 0, W, W).data;
          let top = W;
          for (let y = 0; y < W && top === W; y++) for (let x = 0; x < W; x++) if (d[(y * W + x) * 4 + 3] > 40) { top = y; break; }
          const line = A.collar.cy - 0.34 * ry; // the knot and bow rise a little above the ring, never up the face
          if (top - W / 2 < line) bad.push(`${where}: ${Math.round(line - (top - W / 2))} px above the neck line`);
          if (top - W / 2 < A.eyeLine + 0.12 * ry) bad.push(`${where}: ${Math.round(A.eyeLine + 0.12 * ry - (top - W / 2))} px into his eyes`);
        }
      }
    }
  }

  // The contact sheets, one per look at stage 3, at the open island's size: every item at
  // yaw -0.6 / 0 / +0.6 and pitch -0.2 / +0.2.
  const S = 150;
  const SHEET_POSES = [];
  for (const pitch of [-0.2, 0.2]) for (const yaw of [-0.6, 0, 0.6]) SHEET_POSES.push([yaw, pitch]);
  const sheets = {};
  for (const look of ["night", "ember", "frost"]) {
    const sheet = document.createElement("canvas");
    sheet.width = S * SHEET_POSES.length;
    sheet.height = S * ACCESSORIES.length;
    const sx = sheet.getContext("2d");
    sx.fillStyle = "#1b1d22";
    sx.fillRect(0, 0, sheet.width, sheet.height);
    ACCESSORIES.forEach((item, row) => {
      SHEET_POSES.forEach(([yaw, pitch], col) => {
        const e = new BotEngine();
        e.setSkin(look, false);
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
    sheets[look] = sheet.toDataURL("image/png");
  }

  // The motion strips: one item per kind, a hop, a nod and a turn, eight frames each, springs running.
  // Every hat (round 3N item 1), then one of each other kind.
  const KINDS = [...ACCESSORIES.filter((i) => SLOT_OF[i] === "head" && itemArt(i)), "piratePatch", "roundSpecs", "necktie", "knitStripeScarf", "seedPendant"];
  const MOVES = ["hop", "nod", "turn"];
  const M = 120;
  const FR = 8;
  const strip = document.createElement("canvas");
  strip.width = M * FR * MOVES.length;
  strip.height = M * KINDS.length;
  const mx = strip.getContext("2d");
  mx.fillStyle = "#1b1d22";
  mx.fillRect(0, 0, strip.width, strip.height);
  KINDS.forEach((item, row) => {
    MOVES.forEach((move, mi) => {
      const e = new BotEngine();
      e.setSkin("night", false);
      e.setStage(3, false);
      e.setOutfit(item, false);
      e.outfitPresence = 1;
      e.locks.add("yaw");
      e.locks.add("pitch");
      e.locks.add("sy");
      e.locks.add("sx");
      for (let f = 0; f < FR; f++) {
        const u = (f + 1) / FR;
        for (let k = 0; k < 4; k++) {
          const v = u - (3 - k) / (FR * 4);
          if (move === "hop") {
            e.sy = e.tgSy = 1 - 0.18 * Math.sin(v * Math.PI * 2) * (v < 0.5 ? 1 : 0.6);
            e.sx = e.tgSx = 2 - e.sy;
            e.oy = -Math.abs(Math.sin(v * Math.PI)) * 0.12;
          } else if (move === "nod") {
            e.pitch = e.tgPitch = 0.3 * Math.sin(v * Math.PI * 2);
          } else {
            e.yaw = e.tgYaw = 0.9 * Math.sin(v * Math.PI * 2);
          }
          e.update(1 / 30);
        }
        mx.save();
        mx.translate((mi * FR + f) * M, row * M);
        e.draw(mx, M, M);
        mx.restore();
      }
    });
  });
  const strips = strip.toDataURL("image/png");
  return { bad, sheets, strips, renders };
});
for (const [look, url] of Object.entries(found.sheets)) writeFileSync(join(out, `items-sheet-${look}.png`), Buffer.from(url.split(",")[1], "base64"));
writeFileSync(join(out, "items-motion-strips.png"), Buffer.from(found.strips.split(",")[1], "base64"));
await browser.close();
// The full list goes to a file; the log and the build summary name the worst 10.
const size = (m) => Number((m.match(/(\d+(?:\.\d+)?) (?:%|px)/) || [0, 0])[1]);
const all = [...found.bad].sort((x, y) => size(y) - size(x));
writeFileSync(join(out, "items-problems.txt"), all.join("\n"));
const fails = [...errors, ...all.filter((m) => !m.startsWith("warning:"))];
const warns = all.filter((m) => m.startsWith("warning:"));
if (warns.length) console.log(`${warns.length} warnings (turned further than -0.7..0.7), see items-problems.txt`);
if (fails.length) {
  const worst = fails.slice(0, 10);
  for (const e of worst) console.error(e);
  console.error(`${fails.length} problems in all; the worst ${worst.length} are listed, the rest are in items-problems.txt`);
  if (process.env.GITHUB_STEP_SUMMARY) {
    appendFileSync(process.env.GITHUB_STEP_SUMMARY, `### Wardrobe fit: ${fails.length} problems, worst 10\n${worst.map((m) => `- ${m}`).join("\n")}\n`);
  }
  process.exit(1);
}
console.log(`every item fits and follows his head: 45 items, 3 looks, 7 stages, 7 poses (${found.renders} renders)`);
