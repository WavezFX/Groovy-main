// The finish on Ember's fire and Frost's water (round 3E item 6), so they look
// as clean as Night: the boards' flat flame tongues and water are drawn here
// with soft layered gradients instead, in board units over the same shapes.
// - Flame: each tongue fades from its colour at the base to nothing at the
//   tip, with a faint feathered edge; the whole flame sways from its base on
//   slow precomputed noise; a little heat glows on his body and a few embers
//   rise.
// - Water: the level follows the stage; the surface moves in a gentle wave,
//   the water deepens downward, light ripples through it (caustics) and a few
//   bubbles rise and pop at the surface.
// Gradients and shape bounds are built once and kept, so a frame only fills.

import type { Shape } from "./art/draw";

// ── Noise ───────────────────────────────────────────────────────────────────

const TABLE = Array.from({ length: 64 }, (_, i) => {
  const s = Math.sin(i * 91.17 + 3.7) * 43758.5453;
  return s - Math.floor(s);
});

/** Smooth value noise in [0, 1): slow, natural wandering for the flicker. */
export function noise(t: number): number {
  const i = Math.floor(t);
  const f = t - i;
  const a = TABLE[((i % 64) + 64) % 64];
  const b = TABLE[(((i + 1) % 64) + 64) % 64];
  const k = f * f * (3 - 2 * f);
  return a + (b - a) * k;
}

function rgba(hex: string, a: number): string {
  const n = parseInt(hex.slice(1, 7), 16);
  return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${a})`;
}

// ── Flame ───────────────────────────────────────────────────────────────────

/** A flame tongue on the board: a filled curve (sparks and stars stay flat). */
export function isTongue(s: Shape): boolean {
  return !!s.f && !s.s && /C/.test(s.d) && !/A/.test(s.d);
}

const bounds = new WeakMap<Shape, { top: number; bottom: number }>();
function boundsOf(s: Shape) {
  let b = bounds.get(s);
  if (!b) {
    const ys = [...s.d.matchAll(/-?\d+(?:\.\d+)?/g)].map((m) => Number(m[0])).filter((_, i) => i % 2 === 1);
    b = { top: Math.min(...ys), bottom: Math.max(...ys) };
    bounds.set(s, b);
  }
  return b;
}

type Cache = WeakMap<object, CanvasGradient>;
const gradients = new WeakMap<CanvasRenderingContext2D, Cache>();
/** A gradient in board space, made once per canvas and kept (board space doesn't move under it). */
function kept(x: CanvasRenderingContext2D, key: object, make: () => CanvasGradient): CanvasGradient {
  let c = gradients.get(x);
  if (!c) gradients.set(x, (c = new WeakMap()));
  let g = c.get(key);
  if (!g) c.set(key, (g = make()));
  return g;
}

/** The flame sways from its base (board y `base`): the tip wanders, the base stays put. */
export function swayFlame(x: CanvasRenderingContext2D, t: number, base = 31) {
  const lean = (noise(t * 0.55) - 0.5) * 0.16 + (noise(t * 1.7 + 20) - 0.5) * 0.05;
  const lift = 1 + (noise(t * 1.1 + 40) - 0.5) * 0.09;
  x.translate(50, base);
  x.transform(1, 0, lean, lift, 0, 0);
  x.translate(-50, -base);
}

/** Draws a flame layer: tongues soft and translucent, everything else as on the board. */
export function drawFlame(x: CanvasRenderingContext2D, shapes: readonly Shape[], flat: (s: Shape) => void) {
  for (const s of shapes) {
    if (!isTongue(s) || !s.f) {
      flat(s);
      continue;
    }
    const { top, bottom } = boundsOf(s);
    const f = s.f;
    const p = new Path2D(s.d);
    x.save();
    if (s.o !== undefined) x.globalAlpha *= s.o;
    const aura = bottom - top > 50;
    if (!aura) {
      // A feathered edge: a wide, faint stroke of the tongue's own colour.
      x.strokeStyle = rgba(f, 0.16);
      x.lineWidth = 2.4;
      x.lineJoin = "round";
      x.stroke(p);
    }
    x.fillStyle = kept(x, s, () => {
      if (aura) {
        // An aura all round him: it fades outward from behind his body on every side.
        const g = x.createRadialGradient(50, 58, 26, 50, 52, 40 + (58 - top) * 0.2);
        g.addColorStop(0, rgba(f, 0.95));
        g.addColorStop(0.45, rgba(f, 0.8));
        g.addColorStop(1, rgba(f, 0));
        return g;
      }
      const g = x.createLinearGradient(0, bottom, 0, top);
      g.addColorStop(0, rgba(f, 0.95));
      g.addColorStop(0.55, rgba(f, 0.85));
      g.addColorStop(1, rgba(f, 0));
      return g;
    });
    x.fill(p);
    x.restore();
  }
}

const HEAT = {};
/** A faint warmth on his body under the flame (the caller clipped to the body). */
export function drawHeat(x: CanvasRenderingContext2D, stage: number) {
  x.save();
  x.globalAlpha *= Math.min(1, 0.35 + stage * 0.12);
  x.fillStyle = kept(x, HEAT, () => {
    const g = x.createRadialGradient(50, 30, 0, 50, 30, 42);
    g.addColorStop(0, "rgba(255,150,60,0.28)");
    g.addColorStop(0.5, "rgba(255,120,40,0.1)");
    g.addColorStop(1, "rgba(255,110,30,0)");
    return g;
  });
  // Far wider than his body: the board slides as he turns (round 3E item 9).
  x.fillRect(SEA_L, 0, SEA_R - SEA_L, SEA_FLOOR);
  x.restore();
}

/** A few tiny embers drifting up off the flame and fading. */
export function drawEmbers(x: CanvasRenderingContext2D, stage: number, t: number, lite: boolean) {
  const n = lite ? 2 : stage >= 4 ? 5 : 3;
  const height = 10 + stage * 6;
  for (let i = 0; i < n; i++) {
    const life = 2.6 + TABLE[i + 3] * 1.8;
    const k = ((t + TABLE[i + 9] * life) % life) / life;
    const px = 44 + TABLE[i] * 12 + (noise(t * 0.9 + i * 7) - 0.5) * 6 * k;
    const py = 26 - k * height;
    const a = Math.sin(k * Math.PI) * 0.85;
    if (a < 0.03) continue;
    x.fillStyle = k < 0.5 ? `rgba(255,200,90,${a})` : `rgba(255,140,50,${a})`;
    x.beginPath();
    x.arc(px, py, 0.45 + TABLE[i + 5] * 0.4, 0, Math.PI * 2);
    x.fill();
  }
}

// ── Water ───────────────────────────────────────────────────────────────────

/** Where his water stands at each stage (board y; his body is y 30–92). */
export const WATER_LEVEL: readonly (number | null)[] = [null, 88, 77, 61, 45, 34, 34];

/** The board's water, surface lines and bubbles, which are drawn here instead. */
export function isWaterPart(s: Shape): boolean {
  if (!s.clip) return false;
  if (/^M10 /.test(s.d)) return true;
  return s.f?.toUpperCase() === "#FFFFFF" && /A/.test(s.d);
}

/** The surface at board x, time t. */
function wave(px: number, t: number, calm: number): number {
  return (Math.sin(px * 0.16 + t * 1.3) * 1.0 + Math.sin(px * 0.07 - t * 0.8) * 0.6) * calm;
}

/**
 * How far the sea reaches, in board x (round 3E item 9). His body is x 14-86,
 * but the board slides with his head by up to 0.8 of his half-width (about
 * 29 board units) each way, so a sea only as wide as the body left a blank
 * strip on the side he turned from. The clip to his body decides what shows.
 */
export const SEA_L = -70;
export const SEA_R = 170;
/** Well under his body (y 92) at any squash. */
export const SEA_FLOOR = 140;

function surface(level: number, t: number, calm: number, closed: boolean): Path2D {
  const p = new Path2D();
  p.moveTo(SEA_L, level + wave(SEA_L, t, calm));
  for (let px = SEA_L + 6; px <= SEA_R; px += 6) p.lineTo(px, level + wave(px, t, calm));
  if (closed) {
    p.lineTo(SEA_R, SEA_FLOOR);
    p.lineTo(SEA_L, SEA_FLOOR);
    p.closePath();
  }
  return p;
}

const levelKeys = WATER_LEVEL.map(() => ({}));

/** His water (the caller clipped to his body): before the board's sea life. */
export function drawWater(x: CanvasRenderingContext2D, stage: number, deep: string, t: number, lite: boolean) {
  const level = WATER_LEVEL[Math.min(stage, 6)];
  if (level == null) return;
  const calm = stage <= 1 ? 0.4 : 1;
  const tt = lite ? t * 0.5 : t;
  x.save();
  const water = surface(level, tt, calm, true);
  x.fillStyle = kept(x, levelKeys[Math.min(stage, 6)], () => {
    const g = x.createLinearGradient(0, level - 2, 0, 96);
    g.addColorStop(0, rgba("#8FD0FF", 0.55));
    g.addColorStop(0.18, rgba(deep, 0.5));
    g.addColorStop(1, rgba(deep, 0.75));
    return g;
  });
  x.fill(water);
  // Light rippling through the water: a few soft bright bands that drift.
  x.save();
  x.clip(water);
  x.globalCompositeOperation = "lighter";
  x.strokeStyle = "rgba(200,235,255,0.13)";
  x.lineCap = "round";
  const bands = lite ? 2 : 4;
  for (let i = 0; i < bands; i++) {
    const y = level + 5 + i * ((96 - level) / (bands + 0.5));
    const sx = (noise(tt * 0.25 + i * 13) - 0.5) * 18;
    x.lineWidth = 1.2 + TABLE[i + 20] * 1.4;
    x.beginPath();
    x.moveTo(-50 + sx, y);
    x.quadraticCurveTo(0 + sx, y - 3 + Math.sin(tt * 0.9 + i) * 2, 50 + sx, y);
    x.quadraticCurveTo(100 + sx, y + 3 - Math.sin(tt * 0.7 + i) * 2, 150 + sx, y);
    x.stroke();
  }
  x.restore();
  // The surface: a soft bright line on the wave.
  x.strokeStyle = "rgba(235,248,255,0.7)";
  x.lineWidth = 0.9;
  x.lineJoin = "round";
  x.stroke(surface(level, tt, calm, false));
  // A few bubbles, rising and popping at the surface.
  if (stage >= 2) drawBubbles(x, level, tt, lite ? 2 : 4);
  x.restore();
}

function drawBubbles(x: CanvasRenderingContext2D, level: number, t: number, n: number) {
  for (let i = 0; i < n; i++) {
    const life = 5 + TABLE[i + 30] * 4;
    const k = ((t + TABLE[i + 34] * life) % life) / life;
    const y = 93 - k * (93 - level - 2);
    const px = 22 + TABLE[i + 38] * 56 + Math.sin(t * 1.6 + i * 2) * 1.2;
    const r = 0.7 + TABLE[i + 42] * 0.9;
    const a = Math.min(1, k * 6, (1 - k) * 10) * 0.75;
    if (a < 0.03) continue;
    x.fillStyle = `rgba(255,255,255,${a * 0.18})`;
    x.strokeStyle = `rgba(255,255,255,${a})`;
    x.lineWidth = 0.35;
    x.beginPath();
    x.arc(px, y, r, 0, Math.PI * 2);
    x.fill();
    x.stroke();
    x.fillStyle = `rgba(255,255,255,${a})`;
    x.beginPath();
    x.arc(px - r * 0.35, y - r * 0.35, r * 0.25, 0, Math.PI * 2);
    x.fill();
  }
}

/** Under the water the board's shapes waver a little, as if seen through it. */
export function refract(x: CanvasRenderingContext2D, t: number) {
  x.translate(Math.sin(t * 1.1) * 0.35, Math.sin(t * 0.8) * 0.3);
}
