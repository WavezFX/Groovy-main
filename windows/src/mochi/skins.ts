// Groovy's looks: the colours, glow, eyes and nature touches drawn on the same
// squishy body, so every animation, state, emote and hand gesture works
// unchanged. Drawn in code like the rest of the character — no images.
//
// Three looks that grow (FIXES_ROUND3 item 6): Night grows into Grove (a
// sprout, leaves, a sapling, branches, a bonsai), Ember into Blaze (a spark,
// a flame, flames round him, a fire aura) and Frost into Tide (drops, then
// water filling him up). The stage, 0…5, comes from how long Groovy has run
// (./growth.ts); the colours move from the look's toward its grown one.

import { proj, type Head } from "./outfits";
import { N_ } from "../i18n/i18n";

export type Skin = "night" | "ember" | "frost";

export const SKINS: readonly Skin[] = ["night", "ember", "frost"];
export const DEFAULT_SKIN: Skin = "night";

/** The grown looks of earlier versions, now the last stage of a base look. */
export const GROWN_FROM: Record<string, Skin> = { grove: "night", blaze: "ember", tide: "frost" };

/** English names, also the i18n keys. */
export const SKIN_KEYS: Record<Skin, string> = {
  night: N_("Night"),
  ember: N_("Ember"),
  frost: N_("Frost"),
};

/** What each look grows into, for the wardrobe's cards. */
export const GROWS_INTO: Record<Skin, string> = {
  night: N_("Grove"),
  ember: N_("Blaze"),
  frost: N_("Tide"),
};

export function isSkin(v: unknown): v is Skin {
  return typeof v === "string" && (SKINS as readonly string[]).includes(v);
}

/** The saved look, or the default for anything unknown; Grove, Blaze and Tide are their base looks now. */
export function parseSkin(v: unknown): Skin {
  if (isSkin(v)) return v;
  return (typeof v === "string" && GROWN_FROM[v]) || DEFAULT_SKIN;
}

type RGB = readonly [number, number, number];

export interface SkinStyle {
  bodyTop: RGB;
  bodyBottom: RGB;
  /** The halo around the body and hands. */
  rimGlow: string;
  /** Colour and glow of the emote eye shapes (happy arcs, hearts…). */
  eyeInk: string;
  eyeGlow: string;
  /** The almond eyes' glowing iris and its bright centre. */
  iris: string;
  irisCore: string;
  /** The look's nature: leaves (Night), fire (Ember) or water (Frost). */
  nature: "leaf" | "flame" | "water" | null;
  /** How far it has grown, 0…5 (0: nothing shows yet). */
  stage: number;
  /** Head shape: width/height multipliers, superellipse exponent (2 = round,
   *  higher = boxier) and taper (< 0 narrows the chin). */
  shape: { w: number; h: number; exp: number; taper: number };
  /** Eye size multipliers, spacing (radians round the head), height offset and tilt. */
  face: { eyeW: number; eyeH: number; eyeSp: number; eyeY: number; eyeTilt: number };
}

const SHAPE = { w: 1.0, h: 1.12, exp: 3.0, taper: -0.05 };
const FACE = { eyeW: 3.0, eyeH: 1.75, eyeSp: 0.47, eyeY: 0.1, eyeTilt: -0.22 };

const night: SkinStyle = {
  bodyTop: [0.93, 0.99, 0.94],
  // Night is green, as plainly as Ember is warm and Frost is cool (round 3B item 3).
  bodyBottom: [0.7, 0.9, 0.76],
  rimGlow: "rgba(70,240,130,1)",
  eyeInk: "rgb(150,255,185)",
  eyeGlow: "rgba(80,255,140,0.95)",
  iris: "rgb(70,235,120)",
  irisCore: "rgb(215,255,225)",
  nature: "leaf",
  stage: 0,
  shape: SHAPE,
  face: FACE,
};

const ember: SkinStyle = {
  ...night,
  bodyTop: [0.99, 0.975, 0.965],
  bodyBottom: [0.88, 0.82, 0.79],
  rimGlow: "rgba(255,120,50,0.85)",
  eyeInk: "rgb(255,190,120)",
  eyeGlow: "rgba(255,120,40,0.95)",
  iris: "rgb(255,110,40)",
  irisCore: "rgb(255,230,190)",
  nature: "flame",
};

const frost: SkinStyle = {
  ...night,
  bodyTop: [0.97, 0.98, 0.995],
  bodyBottom: [0.8, 0.85, 0.93],
  rimGlow: "rgba(80,170,255,0.85)",
  eyeInk: "rgb(150,220,255)",
  eyeGlow: "rgba(70,170,255,0.95)",
  iris: "rgb(60,160,255)",
  irisCore: "rgb(215,240,255)",
  nature: "water",
};

export const SKIN_STYLES: Record<Skin, SkinStyle> = { night, ember, frost };

/** The colours each look grows toward: the old Grove, Blaze and Tide, now stage 5. */
export const GROWN_STYLES: Record<Skin, SkinStyle> = {
  night: {
    ...night,
    bodyTop: [0.9, 0.97, 0.86],
    bodyBottom: [0.6, 0.82, 0.56],
    rimGlow: "rgba(110,225,95,0.95)",
    eyeInk: "rgb(170,240,140)",
    eyeGlow: "rgba(120,220,90,0.9)",
    iris: "rgb(110,215,90)",
    irisCore: "rgb(230,255,200)",
    stage: 5,
  },
  ember: { ...ember, bodyTop: [0.995, 0.97, 0.94], bodyBottom: [0.93, 0.8, 0.7], stage: 5 },
  frost: { ...frost, bodyTop: [0.96, 0.98, 1.0], bodyBottom: [0.82, 0.88, 0.96], stage: 5 },
};

/** "rgb(…)" / "rgba(…)" → [r, g, b, a]. */
function channels(css: string): [number, number, number, number] {
  const n = css.match(/[\d.]+/g)?.map(Number) ?? [0, 0, 0, 1];
  return [n[0] ?? 0, n[1] ?? 0, n[2] ?? 0, n[3] ?? 1];
}

function mixCss(a: string, b: string, k: number): string {
  if (a === b) return a;
  const x = channels(a);
  const y = channels(b);
  const m = x.map((v, i) => v + (y[i] - v) * k);
  const rgb = m.slice(0, 3).map((v) => Math.round(v)).join(",");
  return a.startsWith("rgba") || b.startsWith("rgba") ? `rgba(${rgb},${Math.round(m[3] * 100) / 100})` : `rgb(${rgb})`;
}

function mixRgb(a: RGB, b: RGB, k: number): RGB {
  return [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, a[2] + (b[2] - a[2]) * k];
}

const atCache = new Map<string, SkinStyle>();

/** A look at a stage: its colours moved toward the grown look's, stage 0…5. */
export function styleAt(skin: Skin, stage: number): SkinStyle {
  const st = Math.max(0, Math.min(5, Math.round(stage)));
  const key = `${skin}:${st}`;
  const hit = atCache.get(key);
  if (hit) return hit;
  const a = SKIN_STYLES[skin];
  const b = GROWN_STYLES[skin];
  const k = st / 5;
  const style: SkinStyle = {
    ...a,
    bodyTop: mixRgb(a.bodyTop, b.bodyTop, k),
    bodyBottom: mixRgb(a.bodyBottom, b.bodyBottom, k),
    rimGlow: mixCss(a.rimGlow, b.rimGlow, k),
    eyeInk: mixCss(a.eyeInk, b.eyeInk, k),
    eyeGlow: mixCss(a.eyeGlow, b.eyeGlow, k),
    iris: mixCss(a.iris, b.iris, k),
    irisCore: mixCss(a.irisCore, b.irisCore, k),
    stage: st,
  };
  atCache.set(key, style);
  return style;
}

export interface SkinState {
  /** 1 normally; fades what would clash with a hat (the crown's sprout or flame). */
  hatless: number;
  /** 1 normally; 0 while the body is morphed into the drop box. */
  shown: number;
  time: number;
  /** Fewer particles and calmer motion: Lightweight profile, or a small Groovy. */
  lite?: boolean;
}

/** Behind the body: Blaze's fire round him (stages 4 and 5). */
export function drawSkinBehind(x: CanvasRenderingContext2D, skin: Skin | SkinStyle, H: Head, st: SkinState) {
  const s = typeof skin === "string" ? SKIN_STYLES[skin] : skin;
  if (st.shown < 0.02 || s.nature !== "flame" || s.stage < 4) return;
  x.save();
  x.globalAlpha *= st.shown;
  drawFireAround(x, H, st, s.stage);
  x.restore();
}

/** On the body (clipped to it): what grows inside — moss, embers, water. */
export function drawSkinBody(x: CanvasRenderingContext2D, skin: Skin | SkinStyle, body: Path2D, H: Head, st: SkinState) {
  const s = typeof skin === "string" ? SKIN_STYLES[skin] : skin;
  if (st.shown < 0.02 || !s.nature || s.stage < 1) return;
  x.save();
  x.clip(body);
  x.globalAlpha *= st.shown;
  drawNatureInside(x, s, H, st);
  x.restore();
}

/** In front: what grows on the crown (Night's plant, Ember's flame). */
export function drawSkinFront(x: CanvasRenderingContext2D, skin: Skin | SkinStyle, H: Head, st: SkinState) {
  const s = typeof skin === "string" ? SKIN_STYLES[skin] : skin;
  if (st.shown < 0.02 || s.stage < 1) return;
  x.save();
  x.globalAlpha *= st.shown;
  if (s.nature === "leaf") drawPlant(x, H, st, s.stage);
  if (s.nature === "flame") drawCrownFire(x, H, st, s.stage);
  x.restore();
}

export type MouthMood = "calm" | "happy" | "sad" | "open" | "none";

/** The mouth, at (mx, my) in body space, squeezed by fx as the head turns. */
export function drawSkinMouth(
  x: CanvasRenderingContext2D, skin: Skin | SkinStyle, R: number,
  mx: number, my: number, fx: number, mood: MouthMood,
) {
  const s = typeof skin === "string" ? SKIN_STYLES[skin] : skin;
  // Calm, Groovy shows no mouth; it appears only to smile, gasp or frown.
  if (mood === "none" || mood === "calm") return;
  x.save();
  x.translate(mx, my);
  x.scale(fx, 1);
  const ink = "rgb(14,20,18)";
  x.strokeStyle = ink;
  x.fillStyle = ink;
  x.lineCap = "round";
  x.lineWidth = R * 0.05;
  const w = R * 0.13;
  x.beginPath();
  if (mood === "open") {
    x.ellipse(0, 0, w * 0.45, w * 0.55, 0, 0, Math.PI * 2);
    x.fill();
  } else if (mood === "sad") {
    x.arc(0, w * 0.9, w * 0.8, Math.PI * 1.25, Math.PI * 1.75);
    x.stroke();
  } else if (mood === "happy") {
    // An open D-shaped smile with a little tongue.
    x.moveTo(-w * 0.75, -w * 0.15);
    x.quadraticCurveTo(0, -w * 0.05, w * 0.75, -w * 0.15);
    x.quadraticCurveTo(w * 0.6, w * 0.85, 0, w * 0.85);
    x.quadraticCurveTo(-w * 0.6, w * 0.85, -w * 0.75, -w * 0.15);
    x.fill();
    x.save();
    x.clip();
    x.fillStyle = s.iris;
    x.beginPath();
    x.ellipse(0, w * 0.8, w * 0.45, w * 0.35, 0, 0, Math.PI * 2);
    x.fill();
    x.restore();
  }
  x.restore();
}

/** Almond cut-out eye with a glowing iris, slanting down toward the nose.
 *  Drawn already translated to the eye; `open` 0…1 is the blink. */
export function drawAlmondEye(x: CanvasRenderingContext2D, skin: Skin | SkinStyle, sd: number, w: number, h: number, open: number) {
  const s = typeof skin === "string" ? SKIN_STYLES[skin] : skin;
  const hh = Math.max(h * open, w * 0.05);
  const inner = -sd; // toward the middle of the face
  // Flat, sharp top edge sloping down toward the nose, a pointed inner tip,
  // and a full curved bottom that rounds off at the outer corner.
  const H = hh;
  const ox = -inner * w * 0.5;
  const ix = inner * w * 0.5;
  const p = new Path2D();
  p.moveTo(ox, -H * 0.45);
  p.lineTo(ix, -H * 0.02);
  p.bezierCurveTo(inner * w * 0.2, H * 0.6, -inner * w * 0.5, H * 0.62, ox, -H * 0.45);
  p.closePath();
  x.save();
  x.shadowBlur = 0;
  // A thin bright rim, as if the eye is cut into the shell.
  x.lineWidth = w * 0.035;
  x.strokeStyle = "rgba(255,255,255,0.7)";
  x.stroke(p);
  x.fillStyle = "rgb(6,12,10)";
  x.fill(p);
  x.clip(p);
  const cx = inner * w * 0.08;
  const cy = hh * 0.2;
  const g = x.createRadialGradient(cx, cy, 0, cx, cy, w * 0.36);
  g.addColorStop(0, s.irisCore);
  g.addColorStop(0.22, s.iris);
  g.addColorStop(0.6, s.iris.replace("rgb(", "rgba(").replace(")", ",0.35)"));
  g.addColorStop(1, "rgba(0,0,0,0)");
  x.fillStyle = g;
  x.fillRect(-w, -h * 2, w * 2, h * 4);
  x.restore();
  // The glow spills out of the eye a little.
  if (open > 0.3) {
    x.save();
    x.globalCompositeOperation = "lighter";
    const gg = x.createRadialGradient(cx, cy, 0, cx, cy, w * 0.55);
    gg.addColorStop(0, s.iris.replace("rgb(", "rgba(").replace(")", ",0.22)"));
    gg.addColorStop(1, "rgba(0,0,0,0)");
    x.fillStyle = gg;
    x.beginPath();
    x.arc(cx, cy, w * 0.55, 0, Math.PI * 2);
    x.fill();
    x.restore();
  }
}

// ── What grows (FIXES_ROUND3 item 6) ─────────────────────────────────────────

/** Water's fill for Frost's stages: drops only, then a quarter, half, three quarters, full. */
export const WATER_FILL = [0, 0, 0.25, 0.5, 0.75, 0.97] as const;

/** Inside the body: moss (Night), embers (Ember) or water (Frost) by stage. */
function drawNatureInside(x: CanvasRenderingContext2D, s: SkinStyle, H: Head, st: SkinState) {
  const { rx, ry, R } = H;
  const t = st.lite ? st.time * 0.5 : st.time;
  if (s.nature === "leaf") {
    // Moss at the bottom from the sapling on, a little more each stage.
    if (s.stage < 3) return;
    const top = ry * (0.55 - (s.stage - 3) * 0.12);
    const g = x.createLinearGradient(0, top, 0, ry);
    g.addColorStop(0, "rgba(90,150,70,0)");
    g.addColorStop(1, `rgba(90,150,70,${0.22 + (s.stage - 3) * 0.08})`);
    x.fillStyle = g;
    x.fillRect(-rx * 1.5, top, rx * 3, ry);
  } else if (s.nature === "flame") {
    if (s.stage < 2) return;
    const flick = 0.85 + 0.15 * Math.sin(t * 7.3) * Math.sin(t * 3.1);
    const top = ry * (0.5 - s.stage * 0.08);
    const g = x.createLinearGradient(0, top, 0, ry);
    g.addColorStop(0, "rgba(255,140,40,0)");
    g.addColorStop(1, `rgba(255,110,30,${(0.12 + s.stage * 0.05) * flick})`);
    x.fillStyle = g;
    x.fillRect(-rx * 1.5, top, rx * 3, ry);
  } else if (s.nature === "water") {
    const fill = WATER_FILL[s.stage] ?? 0;
    if (fill <= 0) {
      // Stage 1: a few drops resting at the bottom.
      x.fillStyle = "rgba(90,170,255,0.45)";
      const drops: [number, number, number][] = [[-0.35, 0.78, 0.07], [0.1, 0.86, 0.055], [0.42, 0.74, 0.045]];
      for (const [dx, dy, r] of drops) {
        const wob = Math.sin(t * 1.3 + dx * 4) * R * 0.006;
        x.beginPath();
        x.ellipse(dx * rx, dy * ry + wob, R * r, R * r * 1.15, 0, 0, Math.PI * 2);
        x.fill();
      }
      return;
    }
    // A water level that tilts and ripples as he moves.
    const tiltK = (-H.physDx * 0.35 - H.yaw * 0.15) * (fill > 0.9 ? 0.3 : 1);
    const level = ry - fill * 2 * ry;
    const ripple = st.lite ? 0.5 : 1;
    const surf = (px: number) =>
      level + px * tiltK + (Math.sin(px / rx * 3 + t * 2.2) * R * 0.025 + Math.sin(px / rx * 5 - t * 1.7) * R * 0.012) * ripple;
    const steps = st.lite ? 16 : 40;
    const p = new Path2D();
    p.moveTo(-rx * 1.3, surf(-rx * 1.3));
    for (let i = 1; i <= steps; i++) {
      const px = -rx * 1.3 + (i / steps) * rx * 2.6;
      p.lineTo(px, surf(px));
    }
    p.lineTo(rx * 1.3, ry * 1.2);
    p.lineTo(-rx * 1.3, ry * 1.2);
    p.closePath();
    const g = x.createLinearGradient(0, level, 0, ry);
    g.addColorStop(0, "rgba(90,170,255,0.28)");
    g.addColorStop(1, "rgba(40,110,230,0.45)");
    x.fillStyle = g;
    x.fill(p);
    // The bright surface line.
    x.beginPath();
    for (let i = 0; i <= steps; i++) {
      const px = -rx * 1.3 + (i / steps) * rx * 2.6;
      if (i === 0) x.moveTo(px, surf(px));
      else x.lineTo(px, surf(px));
    }
    x.strokeStyle = "rgba(220,240,255,0.7)";
    x.lineWidth = R * 0.02;
    x.stroke();
    // Bubbles drifting up: more as he fills.
    x.fillStyle = "rgba(230,245,255,0.55)";
    const count = st.lite ? 1 : 1 + s.stage;
    for (let i = 0; i < count; i++) {
      const ph = (t * 0.35 + i / count) % 1;
      const bx = (((i * 0.37) % 1) * 1.3 - 0.65) * rx + Math.sin(t * 2 + i) * R * 0.03;
      const by = ry * 0.95 - ph * (ry * 0.95 - level);
      x.beginPath();
      x.arc(bx, by, R * (0.022 + 0.008 * (i % 3)), 0, Math.PI * 2);
      x.fill();
    }
  }
}

/** Top of the head, in body space. */
function crown(H: Head) {
  return proj(H, [0, 1.0, 0.05]);
}

function leaf(x: CanvasRenderingContext2D, L: number, W: number, light = "rgb(150,215,100)", dark = "rgb(80,150,60)") {
  const p = new Path2D();
  p.moveTo(0, 0);
  p.quadraticCurveTo(W, -L * 0.5, 0, -L);
  p.quadraticCurveTo(-W, -L * 0.5, 0, 0);
  const g = x.createLinearGradient(0, 0, 0, -L);
  g.addColorStop(0, dark);
  g.addColorStop(1, light);
  x.fillStyle = g;
  x.fill(p);
}

/**
 * Night → Grove on the crown: 1 a tiny sprout, 2 two small leaves, 3 a
 * sapling with a few leaves, 4 leafy branches, 5 a small bonsai.
 */
function drawPlant(x: CanvasRenderingContext2D, H: Head, st: SkinState, stage: number) {
  const a = st.hatless;
  if (a < 0.02) return;
  const R = H.R;
  const c = crown(H);
  const t = st.lite ? 0 : st.time;
  const sway = Math.sin(t * 1.8) * 0.1 - H.physDx * 0.8;
  x.save();
  x.globalAlpha *= a;
  x.translate(c.x, c.y + R * 0.03);
  x.rotate(sway * (stage >= 5 ? 0.4 : 1));
  x.lineCap = "round";
  const stem = (h: number, w: number, bend = 0.03) => {
    x.strokeStyle = stage >= 5 ? "rgb(120,85,55)" : "rgb(96,140,70)";
    x.lineWidth = R * w;
    x.beginPath();
    x.moveTo(0, 0);
    x.quadraticCurveTo(R * bend, -R * h * 0.55, 0, -R * h);
    x.stroke();
  };
  if (stage === 1) {
    stem(0.1, 0.035);
    x.translate(0, -R * 0.1);
    x.rotate(0.7 + Math.sin(t * 2.4) * 0.08);
    leaf(x, R * 0.12, R * 0.055);
  } else if (stage === 2) {
    x.scale(1.45, 1.45);
    stem(0.22, 0.045);
    for (const sd of [-1, 1]) {
      x.save();
      x.translate(0, -R * 0.2);
      x.rotate(sd * (0.9 + Math.sin(t * 2.4 + sd) * 0.08));
      leaf(x, R * 0.26, R * 0.11);
      x.restore();
    }
  } else if (stage === 3) {
    // A sapling: a taller stem and four leaves up it.
    x.scale(1.5, 1.5);
    stem(0.42, 0.05, 0.05);
    const spots: [number, number, number][] = [[0.2, -1, 0.2], [0.28, 1, 0.22], [0.38, -1, 0.24], [0.44, 1, 0.2]];
    for (const [h, sd, L] of spots) {
      x.save();
      x.translate(0, -R * h);
      x.rotate(sd * (1.0 + Math.sin(t * 2.2 + h * 9) * 0.07));
      leaf(x, R * L, R * L * 0.42);
      x.restore();
    }
  } else if (stage === 4) {
    // Leafy branches: the stem forks, each branch tipped with leaves.
    x.scale(1.55, 1.55);
    stem(0.3, 0.06);
    for (const sd of [-1, 1]) {
      x.save();
      x.translate(0, -R * 0.28);
      x.rotate(sd * 0.55);
      x.strokeStyle = "rgb(96,140,70)";
      x.lineWidth = R * 0.04;
      x.beginPath();
      x.moveTo(0, 0);
      x.lineTo(0, -R * 0.2);
      x.stroke();
      x.translate(0, -R * 0.2);
      for (const k of [-1, 0, 1]) {
        x.save();
        x.rotate(k * 0.75 + Math.sin(t * 2.4 + k + sd) * 0.06);
        leaf(x, R * 0.2, R * 0.085);
        x.restore();
      }
      x.restore();
    }
  } else {
    // A small bonsai: a curved trunk and three round pads of leaves.
    x.scale(1.5, 1.5);
    x.strokeStyle = "rgb(120,85,55)";
    x.lineWidth = R * 0.075;
    x.beginPath();
    x.moveTo(0, 0);
    x.bezierCurveTo(R * 0.12, -R * 0.12, -R * 0.12, -R * 0.24, R * 0.02, -R * 0.38);
    x.stroke();
    x.lineWidth = R * 0.04;
    x.beginPath();
    x.moveTo(-R * 0.02, -R * 0.2);
    x.quadraticCurveTo(-R * 0.14, -R * 0.24, -R * 0.24, -R * 0.26);
    x.moveTo(R * 0.02, -R * 0.28);
    x.quadraticCurveTo(R * 0.14, -R * 0.3, R * 0.25, -R * 0.36);
    x.stroke();
    const pads: [number, number, number][] = [[-0.25, -0.3, 0.13], [0.25, -0.4, 0.12], [0.02, -0.48, 0.16]];
    for (const [px, py, r] of pads) {
      const g = x.createRadialGradient(px * R, (py - r * 0.4) * R, 0, px * R, py * R, r * R * 1.3);
      g.addColorStop(0, "rgb(150,215,100)");
      g.addColorStop(1, "rgb(70,135,55)");
      x.fillStyle = g;
      x.beginPath();
      x.ellipse(px * R, py * R, r * R * 1.25, r * R * 0.8, 0, 0, Math.PI * 2);
      x.fill();
    }
  }
  x.restore();
}

/** One flame tongue from (0,0) up to height L, half width W, leaning by `lean`. */
function tongue(x: CanvasRenderingContext2D, L: number, W: number, lean: number, base: string, tip: string) {
  const p = new Path2D();
  p.moveTo(-W, 0);
  p.bezierCurveTo(-W, -L * 0.45, lean * L - W * 0.2, -L * 0.7, lean * L, -L);
  p.bezierCurveTo(lean * L + W * 0.2, -L * 0.7, W, -L * 0.45, W, 0);
  p.quadraticCurveTo(0, W * 0.5, -W, 0);
  const g = x.createLinearGradient(0, 0, 0, -L);
  g.addColorStop(0, base);
  g.addColorStop(1, tip);
  x.fillStyle = g;
  x.fill(p);
}

const TONGUES: Array<[number, number, number, string, string]> = [
  // offset, height, width, base colour, tip colour
  [-0.1, 0.36, 0.12, "rgba(235,70,20,0.95)", "rgba(255,60,0,0.15)"],
  [0.1, 0.32, 0.12, "rgba(235,70,20,0.95)", "rgba(255,60,0,0.15)"],
  [0, 0.5, 0.16, "rgba(255,130,30,0.95)", "rgba(255,90,0,0.2)"],
  [0, 0.28, 0.09, "rgba(255,220,120,0.95)", "rgba(255,190,70,0.3)"],
];

/** Ember → Blaze on the crown: 1 a spark, 2 a small flame, 3 a bigger one, 4–5 a big flame. */
function drawCrownFire(x: CanvasRenderingContext2D, H: Head, st: SkinState, stage: number) {
  const a = st.hatless;
  if (a < 0.02) return;
  const R = H.R;
  const t = st.lite ? st.time * 0.6 : st.time;
  const c = crown(H);
  x.save();
  x.globalAlpha *= a;
  x.translate(c.x, c.y + R * 0.05);
  if (stage === 1) {
    // A spark: a glowing point and a few tiny embers rising off it.
    const glow = x.createRadialGradient(0, -R * 0.05, 0, 0, -R * 0.05, R * 0.16);
    glow.addColorStop(0, "rgba(255,230,150,0.95)");
    glow.addColorStop(0.4, "rgba(255,140,40,0.6)");
    glow.addColorStop(1, "rgba(255,120,30,0)");
    x.fillStyle = glow;
    x.beginPath();
    x.arc(0, -R * 0.05, R * 0.16, 0, Math.PI * 2);
    x.fill();
    const embers = st.lite ? 1 : 3;
    for (let i = 0; i < embers; i++) {
      const ph = (t * 0.6 + i / embers) % 1;
      x.fillStyle = `rgba(255,180,80,${0.9 * (1 - ph)})`;
      x.beginPath();
      x.arc(Math.sin(t * 3 + i * 2) * R * 0.06, -R * (0.08 + ph * 0.3), R * 0.018, 0, Math.PI * 2);
      x.fill();
    }
    x.restore();
    return;
  }
  const size = stage === 2 ? 0.6 : stage === 3 ? 0.95 : 1.25;
  const lean = -H.physDx * 0.6 + Math.sin(t * 2.3) * 0.05;
  const glow = x.createRadialGradient(0, -R * 0.15 * size, 0, 0, -R * 0.15 * size, R * 0.45 * size);
  glow.addColorStop(0, "rgba(255,140,40,0.35)");
  glow.addColorStop(1, "rgba(255,120,30,0)");
  x.fillStyle = glow;
  x.beginPath();
  x.arc(0, -R * 0.15 * size, R * 0.45 * size, 0, Math.PI * 2);
  x.fill();
  TONGUES.forEach(([off, ht, wd, base, tip], i) => {
    const flick = 1 + 0.18 * Math.sin(t * (9 + i * 2.7) + i) + 0.08 * Math.sin(t * 17 + i * 3);
    x.save();
    x.translate(R * off * size, 0);
    tongue(x, R * ht * flick * size, R * wd * size, lean, base, tip);
    x.restore();
  });
  x.restore();
}

/** Ember → Blaze round the body: 4 flames climbing his sides, 5 a fire aura all round. */
function drawFireAround(x: CanvasRenderingContext2D, H: Head, st: SkinState, stage: number) {
  const { rx, ry, R } = H;
  const t = st.lite ? st.time * 0.6 : st.time;
  if (stage >= 5) {
    // The aura: a soft orange halo, then a ring of tongues licking outward.
    const halo = x.createRadialGradient(0, 0, Math.min(rx, ry) * 0.8, 0, 0, Math.max(rx, ry) * 1.55);
    halo.addColorStop(0, "rgba(255,130,40,0.45)");
    halo.addColorStop(0.6, "rgba(255,90,20,0.18)");
    halo.addColorStop(1, "rgba(255,80,10,0)");
    x.fillStyle = halo;
    x.beginPath();
    x.ellipse(0, 0, rx * 1.6, ry * 1.6, 0, 0, Math.PI * 2);
    x.fill();
  }
  const count = stage >= 5 ? (st.lite ? 8 : 14) : st.lite ? 4 : 6;
  const half = Math.ceil(count / 2);
  for (let i = 0; i < count; i++) {
    // Stage 5: all round. Stage 4: up both sides, from the bottom to the shoulders
    // (angles in body space, y down: π/2 is the bottom, 0 the right side).
    let ang: number;
    if (stage >= 5) ang = (i / count) * Math.PI * 2;
    else {
      const k = (i % half) / Math.max(1, half - 1);
      const right = Math.PI * 0.38 - k * Math.PI * 0.58;
      ang = i < half ? right : Math.PI - right;
    }
    const ex = Math.cos(ang) * rx * 0.98;
    const ey = Math.sin(ang) * ry * 0.98;
    const flick = 1 + 0.25 * Math.sin(t * (7 + i * 1.3) + i * 2);
    x.save();
    x.translate(ex, ey);
    x.rotate(ang + Math.PI / 2);
    tongue(x, R * (stage >= 5 ? 0.34 : 0.26) * flick, R * 0.09, Math.sin(t * 2 + i) * 0.08, "rgba(255,110,30,0.85)", "rgba(255,70,0,0)");
    x.restore();
  }
}
