// Stage 6, the final forms (round 3E item 5 / round 5 item 2), drawn over the
// board art (art/stage6.ts, from docs/design/wardrobe-v2/Stage6.dc.html):
// - Grove, the Sakura bonsai: pink petals drifting round him (five at most).
// - Blaze, the golden flame: three gold sparkles floating up.
// - Tide, the Diamond sea: diamond flakes glinting round him, and golden sea
//   life swimming through his water, one creature at a time, one pass every
//   2–4 minutes: a fish most of the time, sometimes a shark, sometimes a
//   submarine.
// Everything moves with the engine's clock, in board units (Groovy's body is
// x 14–86, y 30–92), so it costs nothing while the island isn't drawing.

import type { Skin } from "./skins";

export const GOLD = "#E0B84A";
const GOLD_HI = "#FFE08A";

/** Fish 70 %, shark 20 %, submarine 10 %. */
export type Creature = "fish" | "shark" | "submarine";

/** A steady pseudo-random number in [0, 1) for an integer. */
function hash(i: number): number {
  const s = Math.sin(i * 12.9898 + 78.233) * 43758.5453;
  return s - Math.floor(s);
}

export function creatureFor(pass: number): Creature {
  const r = hash(pass * 7 + 1);
  return r < 0.7 ? "fish" : r < 0.9 ? "shark" : "submarine";
}

/** How long one pass takes across his sea (seconds). */
export const PASS_SECONDS = 9;

/** When pass `n` starts: one every 2–4 minutes. */
export function passStart(n: number): number {
  let t = 20;
  for (let i = 0; i < n; i++) t += 120 + hash(i * 3 + 5) * 120;
  return t;
}

/**
 * The creature in his sea at time `t` (seconds), and how far across (0–1),
 * or null between passes. Passes are found from a cached table, so this is
 * cheap at every frame.
 */
const starts: number[] = [];
export function creatureAt(t: number): { kind: Creature; progress: number; dir: 1 | -1 } | null {
  if (t < 0) return null;
  while (!starts.length || starts[starts.length - 1] <= t) starts.push(passStart(starts.length));
  // The last pass that began at or before t.
  let n = starts.length - 1;
  while (n > 0 && starts[n] > t) n--;
  const into = t - starts[n];
  if (starts[n] > t || into > PASS_SECONDS) return null;
  return { kind: creatureFor(n), progress: into / PASS_SECONDS, dir: hash(n * 11 + 2) < 0.5 ? 1 : -1 };
}

/** Eased: slow in, slow out, so it swims rather than slides. */
function ease(p: number): number {
  return p < 0.5 ? 2 * p * p : 1 - Math.pow(-2 * p + 2, 2) / 2;
}

function fish(x: CanvasRenderingContext2D, wag: number, big: boolean) {
  const L = big ? 16 : 9;
  const H = big ? 5 : 3.6;
  x.fillStyle = GOLD;
  x.beginPath();
  x.moveTo(L * 0.55, 0);
  x.quadraticCurveTo(0, -H, -L * 0.45, 0);
  x.quadraticCurveTo(0, H, L * 0.55, 0);
  x.fill();
  // The tail swishes.
  x.beginPath();
  x.moveTo(-L * 0.4, 0);
  x.lineTo(-L * 0.75, -H * 0.8 + wag);
  x.lineTo(-L * 0.75, H * 0.8 + wag);
  x.closePath();
  x.fill();
  if (big) {
    // The shark's fin, and a darker back.
    x.beginPath();
    x.moveTo(-1, -H * 0.85);
    x.lineTo(2.5, -H * 1.9);
    x.lineTo(4, -H * 0.7);
    x.fill();
  }
  x.fillStyle = GOLD_HI;
  x.globalAlpha *= 0.6;
  x.beginPath();
  x.ellipse(L * 0.05, -H * 0.3, L * 0.28, H * 0.22, 0, 0, Math.PI * 2);
  x.fill();
  x.globalAlpha /= 0.6;
  x.fillStyle = "#3A2A08";
  x.beginPath();
  x.arc(L * 0.32, -H * 0.12, big ? 0.8 : 0.6, 0, Math.PI * 2);
  x.fill();
}

function submarine(x: CanvasRenderingContext2D, t: number) {
  x.fillStyle = GOLD;
  x.beginPath();
  x.ellipse(0, 0, 8, 3.6, 0, 0, Math.PI * 2);
  x.fill();
  x.fillRect(-1.5, -6.5, 4, 3.4);
  x.strokeStyle = GOLD;
  x.lineWidth = 0.8;
  x.beginPath();
  x.moveTo(1, -6.5);
  x.lineTo(1, -8.5);
  x.lineTo(3, -8.5);
  x.stroke();
  x.fillStyle = "#CFF3FF";
  for (const px of [-3.5, 0, 3.5]) {
    x.beginPath();
    x.arc(px, 0, 1, 0, Math.PI * 2);
    x.fill();
  }
  // The propeller turns.
  x.fillStyle = GOLD_HI;
  x.save();
  x.translate(-8.6, 0);
  x.scale(1, Math.abs(Math.sin(t * 12)) + 0.15);
  x.fillRect(-0.6, -2.4, 1.2, 4.8);
  x.restore();
}

/** Inside his body (the caller clipped to it): the golden creature of the Diamond sea. */
export function drawStage6Body(x: CanvasRenderingContext2D, look: Skin, t: number) {
  if (look !== "frost") return;
  const c = creatureAt(t);
  if (!c) return;
  const p = ease(c.progress);
  const from = c.dir === 1 ? 6 : 94;
  const to = c.dir === 1 ? 94 : 6;
  const cx = from + (to - from) * p;
  // Along a gentle wave, low in his water.
  const cy = 74 + Math.sin(c.progress * Math.PI * 2) * 3;
  x.save();
  x.translate(cx, cy);
  if (c.dir === -1) x.scale(-1, 1);
  x.rotate(Math.cos(c.progress * Math.PI * 2) * 0.12);
  const wag = Math.sin(t * 9) * 1.4;
  if (c.kind === "submarine") submarine(x, t);
  else fish(x, wag, c.kind === "shark");
  x.restore();
}

/** Round him: petals, sparkles or diamond flakes. */
export function drawStage6Front(x: CanvasRenderingContext2D, look: Skin, t: number, lite: boolean) {
  const n = look === "night" ? 5 : look === "ember" ? 3 : 4;
  const count = lite ? Math.min(n, 2) : n;
  for (let i = 0; i < count; i++) {
    const seed = hash(i + (look === "night" ? 10 : look === "ember" ? 20 : 30));
    if (look === "night") {
      // A petal: drifts down and sideways, turning slowly, then starts again.
      const life = 11 + seed * 5;
      const k = ((t + seed * life) % life) / life;
      const px = 18 + seed * 64 + Math.sin(t * 0.6 + i) * 6;
      const py = 6 + k * 70;
      x.save();
      x.globalAlpha *= Math.min(1, k * 5, (1 - k) * 4) * 0.9;
      x.translate(px, py);
      x.rotate(t * 0.7 + i);
      x.fillStyle = i % 2 ? "#FF9FC6" : "#FFC1DA";
      x.beginPath();
      x.ellipse(0, 0, 2.4, 1.3, 0, 0, Math.PI * 2);
      x.fill();
      x.restore();
    } else if (look === "ember") {
      // A gold sparkle rising from the flame.
      const life = 3 + seed * 2;
      const k = ((t + seed * life) % life) / life;
      const px = 38 + seed * 24 + Math.sin(t * 2 + i * 2) * 2;
      const py = 24 - k * 26;
      star(x, px, py, 1.4 + seed, Math.sin(k * Math.PI), "#FFD84A");
    } else {
      // A diamond flake that glints and fades where it hangs.
      const life = 4 + seed * 3;
      const k = ((t + seed * life) % life) / life;
      const px = i % 2 ? 6 + seed * 8 : 86 + seed * 8;
      const py = 30 + hash(i + 40) * 50;
      star(x, px, py, 1.6 + seed, Math.sin(k * Math.PI), "#CFF3FF");
    }
  }
}

function star(x: CanvasRenderingContext2D, cx: number, cy: number, r: number, a: number, color: string) {
  if (a <= 0.02) return;
  x.save();
  x.globalAlpha *= a;
  x.fillStyle = color;
  x.beginPath();
  x.moveTo(cx, cy - r * 2);
  x.lineTo(cx + r * 0.5, cy - r * 0.5);
  x.lineTo(cx + r * 2, cy);
  x.lineTo(cx + r * 0.5, cy + r * 0.5);
  x.lineTo(cx, cy + r * 2);
  x.lineTo(cx - r * 0.5, cy + r * 0.5);
  x.lineTo(cx - r * 2, cy);
  x.lineTo(cx - r * 0.5, cy - r * 0.5);
  x.closePath();
  x.fill();
  x.restore();
}

/** Faceted gem eyes at stage 6: emerald (Grove), gold (Blaze), diamond (Tide). */
export const GEM_EYES: Record<Skin, { iris: string; core: string; edge: string }> = {
  night: { iris: "rgb(15,169,88)", core: "rgb(124,242,176)", edge: "rgba(210,255,228,0.85)" },
  ember: { iris: "rgb(224,161,0)", core: "rgb(255,216,74)", edge: "rgba(255,244,200,0.85)" },
  frost: { iris: "rgb(127,212,255)", core: "rgb(207,243,255)", edge: "rgba(255,255,255,0.9)" },
};

/**
 * The facets over a gem eye (the caller has clipped to the eye, which is `w`
 * wide): a few bright cut lines and two small sparkles, in the eye's own
 * almond shape.
 */
export function drawGemFacets(x: CanvasRenderingContext2D, look: Skin, w: number, h: number, t: number) {
  const gem = GEM_EYES[look];
  x.save();
  x.strokeStyle = gem.edge;
  x.globalAlpha *= 0.55;
  x.lineWidth = w * 0.03;
  x.beginPath();
  x.moveTo(-w * 0.35, -h * 0.05);
  x.lineTo(0, h * 0.25);
  x.lineTo(w * 0.35, -h * 0.05);
  x.moveTo(0, h * 0.25);
  x.lineTo(0, -h * 0.3);
  x.moveTo(-w * 0.2, -h * 0.25);
  x.lineTo(0, h * 0.25);
  x.lineTo(w * 0.2, -h * 0.25);
  x.stroke();
  x.restore();
  const glint = 0.5 + 0.5 * Math.sin(t * 2.2);
  star(x, -w * 0.12, -h * 0.05, w * 0.05, 0.6 + 0.4 * glint, "#FFFFFF");
  star(x, w * 0.16, h * 0.12, w * 0.035, 0.4 + 0.6 * (1 - glint), "#FFFFFF");
}
