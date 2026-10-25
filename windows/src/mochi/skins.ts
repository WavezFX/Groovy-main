// Groovy's looks: the colours, glow, eyes and nature touches drawn on the same
// squishy body, so every animation, state, emote and hand gesture works
// unchanged. Drawn in code like the rest of the character — no images.
//
// Three looks that grow (FIXES_ROUND3 item 6): Night grows into Grove (a
// sprout, leaves, a sapling, branches, a bonsai), Ember into Blaze (a spark,
// a flame, flames round him, a fire aura) and Frost into Tide (drops, then
// water filling him up). The stage, 0…6, comes from how long Groovy has run
// and, for 6, six months since he hatched (./growth.ts); the colours move
// from the look's toward its grown one. Stage 6 is each look's final form
// (round 3E item 5, ./stage6.ts): the Sakura bonsai, the golden flame and
// the Diamond sea, with gem eyes.

import type { Head } from "./outfits";
import { N_ } from "../i18n/i18n";
import { drawShapes, toBoard, type Art, type Shape } from "./art/draw";
import { GROWTH_ART } from "./art/growth";
import { STAGE6_ART } from "./art/stage6";
import { GEM_EYES, GOLD, drawGemFacets, drawStage6Body, drawStage6Front } from "./stage6";

/** The last stage: the final forms. */
const FINAL = 6;

/** A piece of one of the board's golden swimmers: gold, a fish's eye, a submarine's portholes. */
function boardSwimmer(s: Shape): boolean {
  const gold = GOLD.toUpperCase();
  if (s.f?.toUpperCase() === gold || s.s?.toUpperCase() === gold) return true;
  if (s.f?.toUpperCase() === "#13241A") return true;
  return s.f?.toUpperCase() === "#7FD4FF" && !s.s && /A1\.3 1\.3/.test(s.d);
}

/**
 * The Diamond sea's board draws all three golden swimmers at once; on him
 * one swims through at a time (stage6.ts), so the board's are left out.
 */
const FINAL_ART: Record<Skin, Art> = {
  ...STAGE6_ART,
  frost: { ...STAGE6_ART.frost, body: (STAGE6_ART.frost.body ?? []).filter((s) => !boardSwimmer(s)) },
};

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
  /** How far it has grown, 0…6 (0: nothing shows yet; 6: the final form). */
  stage: number;
  /** Stage 6: the look whose gem eyes and final-form touches he has. */
  gem?: Skin;
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

/** A look at a stage: its colours moved toward the grown look's, stage 0…6. */
export function styleAt(skin: Skin, stage: number): SkinStyle {
  const st = Math.max(0, Math.min(FINAL, Math.round(stage)));
  const key = `${skin}:${st}`;
  const hit = atCache.get(key);
  if (hit) return hit;
  const a = SKIN_STYLES[skin];
  const b = GROWN_STYLES[skin];
  const k = Math.min(st, 5) / 5;
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
  // The eyes take the board's colours for the stage (round 5 item 1); at
  // stage 6 they are gems: emerald, gold or diamond (round 3E item 5).
  const art = st === FINAL ? FINAL_ART[skin] : GROWTH_ART[skin][st];
  if (art?.iris) style.iris = art.iris;
  if (art?.core) style.irisCore = art.core;
  if (st === FINAL) {
    style.gem = skin;
    style.iris = GEM_EYES[skin].iris;
    style.irisCore = GEM_EYES[skin].core;
  }
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

/** Behind the body: Blaze's fire round him. */
export function drawSkinBehind(x: CanvasRenderingContext2D, skin: Skin | SkinStyle, H: Head, st: SkinState) {
  const s = typeof skin === "string" ? SKIN_STYLES[skin] : skin;
  if (st.shown < 0.02 || s.stage < 1) return;
  drawLayer(x, s, H, st, "behind");
}

/** On the body: what grows inside it (Tide's water, clipped to his body). */
export function drawSkinBody(x: CanvasRenderingContext2D, skin: Skin | SkinStyle, body: Path2D, H: Head, st: SkinState) {
  const s = typeof skin === "string" ? SKIN_STYLES[skin] : skin;
  if (st.shown < 0.02 || s.stage < 1) return;
  drawLayer(x, s, H, st, "body", body);
  if (s.gem) {
    // The Diamond sea's golden swimmer, inside his body.
    x.save();
    x.globalAlpha *= st.shown;
    x.clip(body);
    toBoard(x, H.rx, H.ry, Math.sin(H.yaw) * H.rx * 0.8);
    drawStage6Body(x, s.gem, st.time);
    x.restore();
  }
}

/** In front: what grows on top (Grove's plant, Blaze's flame, Tide's crest). */
export function drawSkinFront(x: CanvasRenderingContext2D, skin: Skin | SkinStyle, H: Head, st: SkinState) {
  const s = typeof skin === "string" ? SKIN_STYLES[skin] : skin;
  if (st.shown < 0.02 || s.stage < 1) return;
  drawLayer(x, s, H, st, "front");
  if (s.gem) {
    // Petals, gold sparkles or diamond flakes round him.
    x.save();
    x.globalAlpha *= st.shown * st.hatless;
    toBoard(x, H.rx, H.ry, Math.sin(H.yaw) * H.rx * 0.35);
    drawStage6Front(x, s.gem, st.time, !!st.lite);
    x.restore();
  }
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
  // Stage 6: the eye is a cut gem.
  if (s.gem && open > 0.3) drawGemFacets(x, s.gem, w, hh, performance.now() / 1000);
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

// ── What grows (round 5 item 1) ──────────────────────────────────────────────
//
// Each stage is drawn from docs/design/wardrobe-v2/Main.dc.html (art/growth.ts):
// Grove's sprout → bonsai, Blaze's spark → aura, Tide's droplets → full sea.
// One shared phase moves them, from the engine's clock: leaves sway, the flame
// flickers, the water bobs. The engine stops drawing while the island is
// hidden, so they cost nothing then.

const LOOK_OF: Record<NonNullable<SkinStyle["nature"]>, Skin> = { leaf: "night", flame: "ember", water: "frost" };

/** The card for a style's look and stage, if it has grown at all. */
export function artFor(s: SkinStyle): Art | null {
  if (!s.nature) return null;
  const look = LOOK_OF[s.nature];
  if (s.stage >= FINAL) return FINAL_ART[look] ?? null;
  return GROWTH_ART[look][Math.max(0, Math.min(5, s.stage))] ?? null;
}

/** The shared phase, slower in the Lightweight profile. */
function phase(st: SkinState): number {
  return (st.lite ? st.time * 0.5 : st.time) * 1.6;
}

/** Leaves sway round the plant's foot, the flame flickers from its base, the water bobs. */
function animate(x: CanvasRenderingContext2D, s: SkinStyle, st: SkinState, layer: "behind" | "body" | "front") {
  const p = phase(st);
  if (s.nature === "leaf" && layer === "front") {
    x.translate(50, 30);
    x.rotate(Math.sin(p) * 0.045);
    x.translate(-50, -30);
  } else if (s.nature === "flame" && layer !== "body") {
    const k = 1 + Math.sin(p * 3.1) * 0.035 + Math.sin(p * 5.3) * 0.02;
    x.translate(50, 31);
    x.scale(1 + (k - 1) * 0.4, k);
    x.translate(-50, -31);
  } else if (s.nature === "water" && layer === "body") {
    x.translate(0, Math.sin(p) * 0.8);
  }
}

function drawLayer(x: CanvasRenderingContext2D, s: SkinStyle, H: Head, st: SkinState, layer: "behind" | "body" | "front", body?: Path2D) {
  const art = artFor(s);
  const shapes = art?.[layer];
  if (!shapes?.length) return;
  x.save();
  x.globalAlpha *= st.shown;
  // The art turns a little with his head, as the eyes do.
  const shift = Math.sin(H.yaw) * H.rx * (layer === "body" ? 0.8 : 0.35);
  if (layer === "body" && body) {
    // What the board clips to his body is clipped to the real body.
    x.save();
    x.clip(body);
    toBoard(x, H.rx, H.ry, shift);
    animate(x, s, st, layer);
    drawShapes(x, shapes, "clipped");
    x.restore();
    toBoard(x, H.rx, H.ry, shift);
    drawShapes(x, shapes, "free");
  } else {
    toBoard(x, H.rx, H.ry, shift);
    animate(x, s, st, layer);
    drawShapes(x, shapes);
  }
  x.restore();
}
