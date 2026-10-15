// Groovy's looks: the colours, glow, eyes and nature touches drawn on the same
// squishy body, so every animation, state, emote and hand gesture works
// unchanged. Drawn in code like the rest of the character — no images.

import { proj, type Head } from "./outfits";
import { N_ } from "../i18n/i18n";

export type Skin = "night" | "ember" | "frost" | "grove" | "blaze" | "tide";

export const SKINS: readonly Skin[] = ["night", "ember", "frost", "grove", "blaze", "tide"];
export const DEFAULT_SKIN: Skin = "night";

/** English names, also the i18n keys. */
export const SKIN_KEYS: Record<Skin, string> = {
  night: N_("Night"),
  ember: N_("Ember"),
  frost: N_("Frost"),
  grove: N_("Grove"),
  blaze: N_("Blaze"),
  tide: N_("Tide"),
};

export function isSkin(v: unknown): v is Skin {
  return typeof v === "string" && (SKINS as readonly string[]).includes(v);
}

/** The saved look, or the default for anything unknown. */
export function parseSkin(v: unknown): Skin {
  return isSkin(v) ? v : DEFAULT_SKIN;
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
  /** A subtle nature touch: a leaf sprout, a small flame, or water sloshing inside. */
  nature: "leaf" | "flame" | "water" | null;
  /** Head shape: width/height multipliers, superellipse exponent (2 = round,
   *  higher = boxier) and taper (< 0 narrows the chin). */
  shape: { w: number; h: number; exp: number; taper: number };
  /** Eye size multipliers, spacing (radians round the head), height offset and tilt. */
  face: { eyeW: number; eyeH: number; eyeSp: number; eyeY: number; eyeTilt: number };
}

const SHAPE = { w: 1.0, h: 1.12, exp: 3.0, taper: -0.05 };
const FACE = { eyeW: 3.0, eyeH: 1.75, eyeSp: 0.47, eyeY: 0.1, eyeTilt: -0.22 };

const night: SkinStyle = {
  bodyTop: [0.975, 0.99, 0.978],
  bodyBottom: [0.8, 0.87, 0.82],
  rimGlow: "rgba(90,255,150,0.9)",
  eyeInk: "rgb(150,255,185)",
  eyeGlow: "rgba(80,255,140,0.95)",
  iris: "rgb(70,235,120)",
  irisCore: "rgb(215,255,225)",
  nature: null,
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
};

export const SKIN_STYLES: Record<Skin, SkinStyle> = {
  night,
  ember,
  frost,
  grove: {
    ...night,
    bodyTop: [0.97, 0.985, 0.955],
    bodyBottom: [0.78, 0.84, 0.74],
    rimGlow: "rgba(120,220,110,0.7)",
    eyeInk: "rgb(170,240,140)",
    eyeGlow: "rgba(120,220,90,0.9)",
    iris: "rgb(110,215,90)",
    irisCore: "rgb(230,255,200)",
    nature: "leaf",
  },
  blaze: { ...ember, bodyTop: [0.995, 0.97, 0.94], bodyBottom: [0.93, 0.8, 0.7], nature: "flame" },
  tide: { ...frost, bodyTop: [0.96, 0.98, 1.0], bodyBottom: [0.82, 0.88, 0.96], nature: "water" },
};

export interface SkinState {
  /** 1 normally; fades what would clash with a hat (the sprout, the flame). */
  hatless: number;
  /** 1 normally; 0 while the body is morphed into the drop box. */
  shown: number;
  time: number;
}

/** On the body (clipped to it): the nature touch inside. */
export function drawSkinBody(x: CanvasRenderingContext2D, skin: Skin, body: Path2D, H: Head, st: SkinState) {
  const s = SKIN_STYLES[skin];
  if (st.shown < 0.02 || !s.nature) return;
  x.save();
  x.clip(body);
  x.globalAlpha *= st.shown;
  drawNatureInside(x, s, H, st);
  x.restore();
}

/** In front: the sprout or the flame on the crown. */
export function drawSkinFront(x: CanvasRenderingContext2D, skin: Skin, H: Head, st: SkinState) {
  const s = SKIN_STYLES[skin];
  if (st.shown < 0.02) return;
  x.save();
  x.globalAlpha *= st.shown;
  if (s.nature === "leaf") drawSprout(x, H, st);
  if (s.nature === "flame") drawFlame(x, H, st);
  x.restore();
}

export type MouthMood = "calm" | "happy" | "sad" | "open" | "none";

/** The mouth, at (mx, my) in body space, squeezed by fx as the head turns. */
export function drawSkinMouth(
  x: CanvasRenderingContext2D, skin: Skin, R: number,
  mx: number, my: number, fx: number, mood: MouthMood,
) {
  const s = SKIN_STYLES[skin];
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
export function drawAlmondEye(x: CanvasRenderingContext2D, skin: Skin, sd: number, w: number, h: number, open: number) {
  const s = SKIN_STYLES[skin];
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

// ── Nature touches (kept subtle) ─────────────────────────────────────────────

/** Inside the body: moss at the bottom, embers, or water sloshing with the motion. */
function drawNatureInside(x: CanvasRenderingContext2D, s: SkinStyle, H: Head, st: SkinState) {
  const { rx, ry, R } = H;
  const t = st.time;
  if (s.nature === "leaf") {
    const g = x.createLinearGradient(0, ry * 0.3, 0, ry);
    g.addColorStop(0, "rgba(90,150,70,0)");
    g.addColorStop(1, "rgba(90,150,70,0.35)");
    x.fillStyle = g;
    x.fillRect(-rx * 1.5, ry * 0.3, rx * 3, ry);
  } else if (s.nature === "flame") {
    const flick = 0.85 + 0.15 * Math.sin(t * 7.3) * Math.sin(t * 3.1);
    const g = x.createLinearGradient(0, ry * 0.2, 0, ry);
    g.addColorStop(0, "rgba(255,140,40,0)");
    g.addColorStop(1, `rgba(255,110,30,${0.32 * flick})`);
    x.fillStyle = g;
    x.fillRect(-rx * 1.5, ry * 0.2, rx * 3, ry);
  } else if (s.nature === "water") {
    // Water level that tilts and ripples as he moves.
    const tiltK = -H.physDx * 0.35 - H.yaw * 0.15;
    const level = ry * 0.42;
    const surf = (px: number) =>
      level + px * tiltK + Math.sin(px / rx * 3 + t * 2.2) * R * 0.025 + Math.sin(px / rx * 5 - t * 1.7) * R * 0.012;
    const p = new Path2D();
    p.moveTo(-rx * 1.3, surf(-rx * 1.3));
    for (let i = 1; i <= 40; i++) {
      const px = -rx * 1.3 + (i / 40) * rx * 2.6;
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
    for (let i = 0; i <= 40; i++) {
      const px = -rx * 1.3 + (i / 40) * rx * 2.6;
      if (i === 0) x.moveTo(px, surf(px));
      else x.lineTo(px, surf(px));
    }
    x.strokeStyle = "rgba(220,240,255,0.7)";
    x.lineWidth = R * 0.02;
    x.stroke();
    // Two small bubbles drifting up.
    x.fillStyle = "rgba(230,245,255,0.55)";
    for (let i = 0; i < 2; i++) {
      const ph = (t * 0.35 + i * 0.5) % 1;
      const bx = (i ? 0.35 : -0.3) * rx + Math.sin(t * 2 + i) * R * 0.03;
      const by = ry * 0.95 - ph * (ry * 0.95 - level);
      x.beginPath();
      x.arc(bx, by, R * (0.022 + 0.01 * i), 0, Math.PI * 2);
      x.fill();
    }
  }
}

/** Top of the head, in body space. */
function crown(H: Head) {
  return proj(H, [0, 1.0, 0.05]);
}

/** A small two-leaf sprout that sways. */
function drawSprout(x: CanvasRenderingContext2D, H: Head, st: SkinState) {
  const a = st.hatless;
  if (a < 0.02) return;
  const R = H.R;
  const c = crown(H);
  const sway = Math.sin(st.time * 1.8) * 0.12 - H.physDx * 0.8;
  x.save();
  x.globalAlpha *= a;
  x.translate(c.x, c.y + R * 0.03);
  x.rotate(sway);
  x.scale(1.45, 1.45);
  // Stem.
  x.strokeStyle = "rgb(96,140,70)";
  x.lineWidth = R * 0.045;
  x.lineCap = "round";
  x.beginPath();
  x.moveTo(0, 0);
  x.quadraticCurveTo(R * 0.03, -R * 0.12, 0, -R * 0.22);
  x.stroke();
  for (const sd of [-1, 1]) {
    x.save();
    x.translate(0, -R * 0.2);
    x.rotate(sd * (0.9 + Math.sin(st.time * 2.4 + sd) * 0.08));
    const L = R * 0.26;
    const W = R * 0.11;
    const leaf = new Path2D();
    leaf.moveTo(0, 0);
    leaf.quadraticCurveTo(W, -L * 0.5, 0, -L);
    leaf.quadraticCurveTo(-W, -L * 0.5, 0, 0);
    const g = x.createLinearGradient(0, 0, 0, -L);
    g.addColorStop(0, "rgb(80,150,60)");
    g.addColorStop(1, "rgb(150,215,100)");
    x.fillStyle = g;
    x.fill(leaf);
    x.strokeStyle = "rgba(40,90,30,0.5)";
    x.lineWidth = R * 0.012;
    x.beginPath();
    x.moveTo(0, -L * 0.1);
    x.lineTo(0, -L * 0.85);
    x.stroke();
    x.restore();
  }
  x.restore();
}

/** A small flickering flame on the crown. */
function drawFlame(x: CanvasRenderingContext2D, H: Head, st: SkinState) {
  const a = st.hatless;
  if (a < 0.02) return;
  const R = H.R;
  const t = st.time;
  const c = crown(H);
  const lean = -H.physDx * 0.6 + Math.sin(t * 2.3) * 0.05;
  x.save();
  x.globalAlpha *= a;
  x.translate(c.x, c.y + R * 0.05);
  // A warm glow behind the flame.
  const glow = x.createRadialGradient(0, -R * 0.15, 0, 0, -R * 0.15, R * 0.45);
  glow.addColorStop(0, "rgba(255,140,40,0.35)");
  glow.addColorStop(1, "rgba(255,120,30,0)");
  x.fillStyle = glow;
  x.beginPath();
  x.arc(0, -R * 0.15, R * 0.45, 0, Math.PI * 2);
  x.fill();
  const tongues: Array<[number, number, number, string, string]> = [
    // offset, height, width, base colour, tip colour
    [-0.1, 0.36, 0.12, "rgba(235,70,20,0.95)", "rgba(255,60,0,0.15)"],
    [0.1, 0.32, 0.12, "rgba(235,70,20,0.95)", "rgba(255,60,0,0.15)"],
    [0, 0.5, 0.16, "rgba(255,130,30,0.95)", "rgba(255,90,0,0.2)"],
    [0, 0.28, 0.09, "rgba(255,220,120,0.95)", "rgba(255,190,70,0.3)"],
  ];
  tongues.forEach(([off, ht, wd, base, tip], i) => {
    const flick = 1 + 0.18 * Math.sin(t * (9 + i * 2.7) + i) + 0.08 * Math.sin(t * 17 + i * 3);
    const L = R * ht * flick;
    const W = R * wd;
    const ox = R * off;
    const p = new Path2D();
    p.moveTo(ox - W, 0);
    p.bezierCurveTo(ox - W, -L * 0.45, ox + lean * L - W * 0.2, -L * 0.7, ox + lean * L, -L);
    p.bezierCurveTo(ox + lean * L + W * 0.2, -L * 0.7, ox + W, -L * 0.45, ox + W, 0);
    p.quadraticCurveTo(ox, W * 0.5, ox - W, 0);
    const g = x.createLinearGradient(0, 0, 0, -L);
    g.addColorStop(0, base);
    g.addColorStop(1, tip);
    x.fillStyle = g;
    x.fill(p);
  });
  x.restore();
}
