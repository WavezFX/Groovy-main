// Groovy's character engine (round 4E item 1). Written from Groovy's own
// motion design, docs/plan/CHARACTER_MOTION.md §2 (approved by the owner,
// 20 Nov 2026): he is a soft pillow with an element living in him. The body
// moves like something stuffed (it bows and puffs at the corners), and the
// element wisps (leaf, flame, droplet) do the expressive work.
//
// Motion uses three tools only, from ./motion: a spring, an uneven breath and
// a cubic ease-out. A move ("act") is a short function of its age that adds
// offsets; the springs chase base + offsets, which gives anticipation,
// overshoot and follow-through without keyframe tables. Every spring snaps to
// exactly its target, so at rest nothing asks for another frame.

import { lerp } from "../core/anim";
import { Sound } from "../core/sound";
import type { BotEmoteName, BotStateName } from "../core/layout";
import { drawOutfitBehind, drawOutfitFront, growthLift, makeHead } from "./outfits";
import type { Outfit } from "./wardrobe";
import { DEFAULT_SKIN, SHAPE, drawAlmondEye, drawSkinBehind, drawSkinBody, styleAt, drawSkinFront, drawSkinMouth, type MouthMood, type Skin, type SkinState, type SkinStyle } from "./skins";
import { styleWith, type Appearance } from "./appearance";
import { PILLOW_BOW } from "./pillow";
import { breath, easeOut, envelope, flicker, stepSpring, type Spring } from "./motion";

// ── Types ─────────────────────────────────────────────────────────────────────

export type EyeShape =
  | "pill" | "wide" | "dot" | "line" | "flat" | "happy" | "closed"
  | "spiral" | "heart" | "star" | "tired" | "wink" | "cup";

export type BadgeKind = "dots" | "bang" | "question" | "dot";

export interface Badge {
  kind: BadgeKind;
  color: RGB;
}

export type RGB = readonly [number, number, number]; // components 0…1

/** Values a glide can move (eased, one target at a time). */
type GlideKey = "roll" | "morph" | "hands" | "blush" | "badgeS" | "outfitPresence" | "tint" | "open";

/** The values the springs chase; a lock (items-check, tests) holds one still. */
type LockKey = GlideKey | "yaw" | "pitch" | "tilt" | "sx" | "sy" | "oy" | "ox" | "es";

interface Glide { from: number; to: number; t0: number; dur: number; done?: () => void }

/** What every act adds this frame. */
interface Offsets {
  yaw: number; pitch: number; tilt: number; sx: number; sy: number; oy: number; ox: number;
  es: number; bow: number; close: number; over: number; glow: number;
  flare: number; drop: number; wave: number; flickL: number; flickR: number; knit: number;
}

interface Act { t0: number; dur: number; fx: (age: number, o: Offsets) => void; done?: () => void }

interface StateLook {
  color: RGB;
  tint: number;
  eye: EyeShape;
  badge: Badge | null;
  /** Leans toward the user (pitch). */
  lean: number;
  /** Head tilt, radians. */
  tilt: number;
  /** A looping state: keeps the frame loop going while it is shown. */
  loops: boolean;
}

type ParticleKind = "fleck" | "ember" | "bubble" | "drop" | "z" | "ring" | "glint";

interface Particle {
  kind: ParticleKind;
  x: number; y: number; vx: number; vy: number;
  age: number; life: number; seed: number; size: number;
  /** Spirals up instead of drifting (love). */
  spiral: boolean;
}

// ── Groovy's palette and states ───────────────────────────────────────────────

const INK = "rgb(14,22,18)";
const MINI_INK = "rgb(12,18,24)";

const P = {
  calm: [0.88, 0.95, 0.9] as RGB,
  work: [0.27, 0.66, 0.98] as RGB,
  think: [0.6, 0.42, 0.95] as RGB,
  seek: [0.36, 0.5, 0.96] as RGB,
  ask: [0.98, 0.7, 0.2] as RGB,
  query: [0.2, 0.84, 0.86] as RGB,
  fault: [0.95, 0.33, 0.36] as RGB,
  done: [0.3, 0.88, 0.55] as RGB,
  limit: [0.98, 0.56, 0.26] as RGB,
  rest: [0.55, 0.64, 0.74] as RGB,
  dizzy: [0.93, 0.5, 0.74] as RGB,
};

const st = (color: RGB, tint: number, eye: EyeShape, badge: BadgeKind | null, extra: Partial<StateLook> = {}): StateLook => ({
  color, tint, eye, badge: badge ? { kind: badge, color } : null, lean: 0, tilt: 0, loops: false, ...extra,
});

export const BOT_STATES: Record<BotStateName, StateLook> = {
  idle: st(P.calm, 0, "pill", null),
  working: st(P.work, 0.66, "pill", "dots"),
  thinking: st(P.think, 0.66, "pill", "dots"),
  searching: st(P.seek, 0.66, "pill", "dots", { loops: true }),
  approval: st(P.ask, 0.72, "wide", "bang", { lean: 0.08, loops: true }),
  question: st(P.query, 0.7, "pill", "question", { tilt: 0.122 }),
  error: st(P.fault, 0.74, "flat", "dot"),
  finished: st(P.done, 0.3, "happy", "dot"),
  ratelimit: st(P.limit, 0.66, "tired", "dot", { loops: true }),
  sleeping: st(P.rest, 0.3, "closed", null, { loops: true }),
  dizzy: st(P.dizzy, 0.64, "spiral", null),
};

const EMOTE_EYE: Record<BotEmoteName, EyeShape> = {
  love: "happy", surprised: "wide", proud: "pill", wink: "wink",
  yawn: "tired", happy: "happy", annoyed: "line",
};

const now = () => performance.now() / 1000;
// Eye placement on the head model (shared with the wardrobe's face items).
const EYE_SP = 0.37;
const EYE_P = -0.12;
const clamp = (v: number, lo: number, hi: number) => (v < lo ? lo : v > hi ? hi : v);
const rand = (a: number, b: number) => a + Math.random() * (b - a);

/** Size of the element wisps that replace his hands (round 3K): one number to tune. */
export const WISP_SCALE = 0.85;

const WISP_COLOURS = {
  leaf: { top: "rgb(190,255,215)", bottom: "rgb(60,190,100)", glow: "rgba(70,240,130,0.9)" },
  flame: { top: "rgb(255,245,170)", bottom: "rgb(255,140,40)", glow: "rgba(255,150,40,0.9)" },
  water: { top: "rgb(255,255,255)", bottom: "rgb(120,200,255)", glow: "rgba(90,180,255,0.9)" },
} as const;

/** The element wisp's outline, pointing up, `S` tall from its centre. */
export function wispPath(kind: "leaf" | "flame" | "water", S: number): Path2D {
  const p = new Path2D();
  if (kind === "leaf") {
    p.moveTo(0, -S);
    p.bezierCurveTo(S * 0.62, -S * 0.45, S * 0.62, S * 0.45, 0, S);
    p.bezierCurveTo(-S * 0.62, S * 0.45, -S * 0.62, -S * 0.45, 0, -S);
  } else if (kind === "flame") {
    p.moveTo(0, -S);
    p.bezierCurveTo(S * 0.2, -S * 0.55, S * 0.75, -S * 0.25, S * 0.6, S * 0.35);
    p.bezierCurveTo(S * 0.5, S * 0.85, -S * 0.5, S * 0.85, -S * 0.6, S * 0.35);
    p.bezierCurveTo(-S * 0.7, -S * 0.05, -S * 0.3, -S * 0.2, -S * 0.15, -S * 0.5);
    p.bezierCurveTo(-S * 0.05, -S * 0.7, -S * 0.05, -S * 0.85, 0, -S);
  } else {
    p.moveTo(0, -S);
    p.bezierCurveTo(S * 0.25, -S * 0.5, S * 0.68, -S * 0.05, S * 0.68, S * 0.32);
    p.arc(0, S * 0.32, S * 0.68, 0, Math.PI, false);
    p.bezierCurveTo(-S * 0.68, -S * 0.05, -S * 0.25, -S * 0.5, 0, -S);
  }
  p.closePath();
  return p;
}

/** One element wisp (leaf, flame lick or droplet) where a hand used to be. */
function drawWisp(x: CanvasRenderingContext2D, kind: "leaf" | "flame" | "water", S: number, R: number, flat: RGB | null) {
  const c = WISP_COLOURS[kind];
  const path = wispPath(kind, S);
  const g = x.createLinearGradient(0, -S, 0, S);
  g.addColorStop(0, flat ? rgba(mix3(flat, [1, 1, 1], 0.35)) : c.top);
  g.addColorStop(1, flat ? rgba(flat) : c.bottom);
  x.fillStyle = g;
  if (!flat) {
    x.shadowColor = c.glow;
    x.shadowBlur = R * 0.3;
  }
  x.fill(path);
  x.shadowBlur = 0;
  if (kind === "leaf") {
    x.beginPath();
    x.moveTo(0, -S * 0.8);
    x.lineTo(0, S * 0.8);
    x.strokeStyle = "rgba(30,120,60,0.55)";
    x.lineWidth = Math.max(0.8, S * 0.08);
    x.stroke();
  }
  // A small soft highlight.
  const hx = -S * 0.18;
  const hy = kind === "water" ? S * 0.15 : -S * 0.2;
  const hg = x.createRadialGradient(hx, hy, 0, hx, hy, S * 0.3);
  hg.addColorStop(0, "rgba(255,255,255,0.75)");
  hg.addColorStop(1, "rgba(255,255,255,0)");
  x.fillStyle = hg;
  x.fill(path);
}

export function hexToRGB(hex: string): RGB {
  const v = parseInt(hex.replace("#", ""), 16);
  return [((v >> 16) & 255) / 255, ((v >> 8) & 255) / 255, (v & 255) / 255];
}

const rgba = (c: RGB, a = 1) =>
  `rgba(${Math.round(c[0] * 255)},${Math.round(c[1] * 255)},${Math.round(c[2] * 255)},${a})`;

const mix3 = (a: RGB, b: RGB, t: number): RGB => [
  lerp(a[0], b[0], t), lerp(a[1], b[1], t), lerp(a[2], b[2], t),
];

/** A capsule centred on 0,0. */
function capsule(x: CanvasRenderingContext2D, w: number, h: number) {
  const r = Math.max(0, Math.min(w, h) / 2);
  x.beginPath();
  x.moveTo(-w / 2 + r, -h / 2);
  x.lineTo(w / 2 - r, -h / 2);
  x.arc(w / 2 - r, 0, r, -Math.PI / 2, Math.PI / 2);
  x.lineTo(-w / 2 + r, h / 2);
  x.arc(-w / 2 + r, 0, r, Math.PI / 2, Math.PI * 1.5);
  x.closePath();
}

/** A rounded rectangle from its top-left corner. */
function rrect(x: CanvasRenderingContext2D, X: number, Y: number, W: number, H: number, r: number) {
  x.beginPath();
  x.roundRect(X, Y, W, H, Math.min(r, W / 2, H / 2));
}

const FONT = `system-ui, "Segoe UI Variable Text", "Segoe UI", sans-serif`;

// ── Dance: a sway from the base, the element keeping the beat ─────────────────

/** The dance fades in over 0.25 s and out over 0.45 s. */
export function stepDanceLevel(level: number, dancing: boolean, dt: number): number {
  return dancing ? Math.min(1, level + dt / 0.25) : Math.max(0, level - dt / 0.45);
}

/** Beats per minute of Groovy's sway. */
export const DANCE_BPM = 104;

/**
 * The sway at `seconds`, for a body of radius R and a dance `level` (0…1): he
 * rocks from his base, side to side once every two beats, and puffs a little
 * just after each beat.
 */
export function danceTransform(seconds: number, level: number, R: number) {
  const beat = (seconds * DANCE_BPM) / 60;
  const rock = Math.sin(Math.PI * beat);
  const frac = beat - Math.floor(beat);
  const puff = (1 - frac) * (1 - frac) * (1 - frac);
  return {
    dx: 0.05 * R * rock * level,
    dy: -0.035 * R * (1 - Math.abs(Math.cos(Math.PI * beat))) * level,
    rotate: 0.07 * rock * level,
    sx: 1 + 0.02 * puff * level,
    sy: 1 + 0.03 * puff * level,
  };
}

// ── Engine ────────────────────────────────────────────────────────────────────

export class BotEngine {
  isMini = false;
  /** Solid body colour for mini bots / integration pills (null = Groovy gradient). */
  bodyColor: RGB | null = null;

  // What is drawn. Springs and glides write these; tests may set them under a lock.
  yaw = 0; pitch = 0; roll = 0; tilt = 0; open = 1;
  sx = 1; sy = 1; oy = 0; ox = 0;
  tint = 0; morph = 0; hands = 0; blush = 0; es = 1; badgeS = 0;

  outfit: Outfit = "none";
  /** Groovy's look (the main character only — minis keep their flat colour). */
  skin: Skin = DEFAULT_SKIN;
  /** 0 = gone, 1 = fully on. */
  outfitPresence = 0;
  private outfitTarget: Outfit = "none";
  /** Accessories worn with the outfit (appearance.ts combine()); they come and go with it. */
  layers: Outfit[] = [];
  /** The user's colours over the look (the main Groovy only). */
  appearance: Appearance | null = null;
  /** How far the look has grown, 0–6 (growth.ts). */
  stage = 0;
  /** Fewer particles and calmer motion (Lightweight profile, small portraits). */
  lite = false;
  private styleCache: { skin: Skin; stage: number; appearance: Appearance | null; style: SkinStyle } | null = null;

  /** The look at its stage, with the user's colours. */
  get style(): SkinStyle {
    const c = this.styleCache;
    if (c && c.skin === this.skin && c.stage === this.stage && c.appearance === this.appearance) return c.style;
    const style = styleWith(styleAt(this.skin, this.stage), this.isMini ? null : this.appearance);
    this.styleCache = { skin: this.skin, stage: this.stage, appearance: this.appearance, style };
    return style;
  }

  /** Shows the look at another stage, with a little squash when animated. */
  setStage(next: number, animated = true) {
    const stage = Math.max(0, Math.min(6, Math.round(next)));
    if (stage === this.stage) return;
    this.stage = stage;
    if (animated) this.squash();
  }

  // Soft parts (pompoms, hat tips, scarf end) lag on a spring, −1…1.
  physDx = 0;
  physDy = 0;
  get physVx() { return this.softX.vel; }
  get physVy() { return this.softY.vel; }
  private softX: Spring = { v: 0, vel: 0 };
  private softY: Spring = { v: 0, vel: 0 };
  /** Dangling worn parts' pendulum: angle; exactly 0 at rest. */
  swing = 0;
  private swingS: Spring = { v: 0, vel: 0 };
  get swingV() { return this.swingS.vel; }
  private prevYaw = 0;
  private prevOy = 0;
  private prevRoll = 0;

  // Base targets (before acts are added).
  tgYaw = 0; tgPitch = 0; tgTilt = 0; tgSy = 1; tgSx = 1; tgEs = 1;

  /** Extra canvas height above the body so particles can fly out without clipping. */
  particleOverhang = 0;

  isDancing = false;
  dancingLevel = 0;

  /** The drop slot (file drag): height as a fraction of R, on a spring. */
  slotH = 0; slotHTarget = 0; isChewing = false;
  private slotV = 0;
  /** The slot spring's speed (the island zeroes it when it resets the slot). */
  get slotHVel() { return this.slotV; }
  set slotHVel(v: number) { this.slotV = v; }

  col: RGB = P.calm;
  colT: RGB = P.calm;

  state: BotStateName = "idle";
  cfg: StateLook = BOT_STATES.idle;

  eyeOverride: EyeShape | null = null;
  eyeOverrideUntil = 0;
  permanentEye: EyeShape | null = null;
  permanentEmote: BotEmoteName | null = null;
  miniNextBehavior = 0;

  badge: Badge | null = null;
  private badgeKey = "none";
  private badgeSwing: Spring = { v: 0, vel: 0 };

  /** Springs' speeds, by value. */
  private vel: Record<string, number> = {};
  private bow = PILLOW_BOW;
  private glides = new Map<GlideKey, Glide>();
  /** Values held still: no spring or act moves them (tests and items-check pin poses). */
  locks = new Set<LockKey>();
  private acts: Act[] = [];
  private particles: Particle[] = [];

  /** Where the eyes look, ahead of the head (fractions of R). */
  private eyeX: Spring = { v: 0, vel: 0 };
  private eyeY: Spring = { v: 0, vel: 0 };
  private glance = { x: 0, y: 0, next: 0 };
  private glow = 1;

  /** The two wisps' poses on springs: x, y in units of rx/ry; r radians. */
  private wisp = [
    { x: { v: -1.08, vel: 0 }, y: { v: 0.7, vel: 0 }, r: { v: 0, vel: 0 } },
    { x: { v: 1.08, vel: 0 }, y: { v: 0.7, vel: 0 }, r: { v: 0, vel: 0 } },
  ];

  lookX = 0;
  lookY = 0;
  private lastLook = { x: 0, y: 0, still: 0 };

  lastTime = now();
  private t0 = now() - Math.random() * 5;
  private nextBlink = now() + rand(1.5, 3.5);
  private nextGesture = now() + rand(20, 40);
  private nextAmbient = 0;
  waveUntil = 0;
  waveStart = 0;
  private slapTimes: number[] = [];
  private heldBreath = 0;
  private greetActs: Act[] = [];
  private rollTotal = 0;

  /** Fired when three slaps land inside 1.7 s (→ dizzy + confused view). */
  onDizzy: (() => void) | null = null;
  /** Something started moving: whoever draws him should be running (island.ts). */
  onWake: (() => void) | null = null;

  private static zero(): Offsets {
    return { yaw: 0, pitch: 0, tilt: 0, sx: 0, sy: 0, oy: 0, ox: 0, es: 0, bow: 0, close: 0, over: 0, glow: 0, flare: 0, drop: 0, wave: 0, flickL: 0, flickR: 0, knit: 0 };
  }

  /** The look's element: leaf (Night), flame (Ember), water (Frost). */
  private get element(): "leaf" | "flame" | "water" {
    return this.isMini ? "leaf" : this.style.nature ?? "leaf";
  }

  // ── Building blocks ─────────────────────────────────────────────────────────

  /** Plays a move: `fx(age, offsets)` for `dur` seconds, starting after `delay`. */
  private act(dur: number, fx: Act["fx"], delay = 0, done?: () => void): Act {
    const a = { t0: now() + delay, dur, fx, done };
    this.acts.push(a);
    this.onWake?.();
    return a;
  }

  /** Eases one value to `to` over `ms` on the cubic ease-out. */
  glide(prop: GlideKey, to: number, ms: number, done?: () => void) {
    this.glides.set(prop, { from: this[prop], to, t0: now(), dur: Math.max(0.001, ms / 1000), done });
    this.locks.add(prop);
    this.onWake?.();
  }

  /** Chases `target` on a spring for an engine value (unless locked). */
  private chase(prop: "yaw" | "pitch" | "tilt" | "sx" | "sy" | "oy" | "ox" | "es", target: number, hz: number, zeta: number, dt: number) {
    if (this.locks.has(prop)) return;
    const s = { v: this[prop], vel: this.vel[prop] ?? 0 };
    stepSpring(s, target, hz, zeta, dt);
    this[prop] = s.v;
    this.vel[prop] = s.vel;
  }

  // ── Public API ──────────────────────────────────────────────────────────────

  setState(next: BotStateName, force = false) {
    if (this.state === next && !force) return;
    const prev = this.state;
    this.state = next;
    this.cfg = BOT_STATES[next];
    this.colT = this.cfg.color;
    if (!this.locks.has("tint")) this.glide("tint", this.cfg.tint, 320);
    this.tgTilt = this.cfg.tilt;
    this.setBadge(this.cfg.badge);

    switch (next) {
      case "finished":
        this.puff();
        this.act(0.01, () => {}, 0.25, () => this.pop(5));
        break;
      case "error":
        this.twitch(3);
        break;
      case "approval":
        // A small dip, then he leans in.
        this.act(0.5, (a, o) => { o.oy += 0.05 * envelope(a, 0.12, 0, 0.38); });
        break;
      case "dizzy":
        this.doRoll(1400, 2);
        break;
      case "question":
        this.act(0.6, (a, o) => { o.flickR += envelope(a, 0.15, 0.1, 0.35); });
        this.blink();
        break;
      case "ratelimit":
        this.emit("drop", 1);
        break;
      case "sleeping":
        this.yawnMove(1.2);
        break;
      default:
        if (prev !== next) this.blink();
    }
  }

  setBadge(b: Badge | null) {
    const key = b ? `${b.kind}-${b.color.join(",")}` : "none";
    if (key === this.badgeKey) return;
    this.badgeKey = key;
    // The old tag lifts away; the new one drops in and swings on its string.
    this.glide("badgeS", 0, 110, () => {
      this.badge = b;
      if (b) {
        this.badgeSwing.vel += 3.2;
        this.glide("badgeS", 1, 260);
      }
    });
  }

  /** A blink in the look's own manner (CHARACTER_MOTION.md §2). */
  blink() {
    if (this.locks.has("open")) return;
    const look = this.isMini ? "night" : this.skin;
    if (look === "ember") {
      // The glow dips first, then the lid.
      this.act(0.36, (a, o) => {
        o.glow += envelope(a, 0.09, 0.06, 0.2);
        o.close += envelope(a - 0.09, 0.07, 0, 0.12);
      });
    } else if (look === "frost") {
      // Slow, with one ripple across the water line as it opens.
      this.act(0.62, (a, o) => {
        o.close += envelope(a, 0.16, 0, 0.22);
        if (a > 0.3) o.es += 0.04 * Math.sin((a - 0.3) * 30) * Math.max(0, 1 - (a - 0.3) / 0.3);
      });
    } else {
      // Night: a shutter, 70 ms down, 110 ms up with a little overshoot.
      this.act(0.26, (a, o) => {
        o.close += envelope(a, 0.07, 0, 0.11);
        if (a > 0.15) o.over += 0.07 * Math.sin(((a - 0.15) / 0.11) * Math.PI);
      });
    }
  }

  /** The pillow set piece for a squash: the sides bow in, then pop back full. */
  squash() {
    this.softY.vel += 0.5;
    this.act(0.5, (a, o) => {
      const k = envelope(a, 0.07, 0, 0.3);
      o.sy -= 0.12 * k;
      o.sx += 0.09 * k;
      o.bow += 0.06 * envelope(a, 0.05, 0.02, 0.2) - 0.04 * envelope(a - 0.12, 0.08, 0, 0.25);
    });
  }

  /** File drop: the slot opens, the sides bow in, he swallows and puffs back. */
  gulp() {
    this.slotHTarget = 0.42;
    this.act(0.01, () => {}, 0.46, () => {
      this.slotHTarget = 0;
      this.isChewing = true;
      this.act(0.01, () => {}, 0.8, () => { this.isChewing = false; });
    });
    this.act(0.7, (a, o) => {
      o.bow += 0.09 * envelope(a, 0.1, 0.12, 0.2);
      o.sy += 0.06 * envelope(a - 0.25, 0.1, 0, 0.3);
      o.sx -= 0.05 * envelope(a, 0.1, 0.12, 0.2);
    });
    this.blink();
  }

  slap() {
    this.interruptGreet();
    if (this.state === "dizzy") return;
    const t = now();
    this.slapTimes = this.slapTimes.filter((s) => t - s < 1.7);
    this.slapTimes.push(t);
    Sound.play("slap");
    this.squash();
    this.softY.vel -= 1.1;
    this.softX.vel += Math.random() < 0.5 ? 0.8 : -0.8;
    this.vel.ox = (this.vel.ox ?? 0) + (Math.random() < 0.5 ? 1.6 : -1.6);
    if (this.slapTimes.length >= 3) {
      this.slapTimes = [];
      this.onDizzy?.();
    } else {
      this.eyeOverride = "line";
      this.eyeOverrideUntil = t + 0.8;
      this.act(0.01, () => {}, 0.06, () => Sound.play("annoyed"));
    }
  }

  /** A tumble: eased out, the wisps trail a quarter-turn behind (drawWisps). */
  doRoll(durationMs: number, turns: number) {
    this.roll = 0;
    this.rollTotal = Math.PI * 2 * turns;
    this.act(0.25, (a, o) => { o.sy -= 0.06 * envelope(a, 0.08, 0, 0.17); o.sx += 0.04 * envelope(a, 0.08, 0, 0.17); });
    this.glide("roll", Math.PI * 2 * turns, durationMs, () => { this.roll = 0; });
  }

  /** Hello: a puff, then a two-beat wisp wave. */
  greet() {
    const t = now();
    this.waveStart = t + 0.3;
    this.waveUntil = t + 1.5;
    this.eyeOverride = "happy";
    this.eyeOverrideUntil = t + 1.9;
    this.puff();
    this.glide("hands", 1, 260);
    this.greetActs = [
      this.act(1.2, (a, o) => { o.wave += envelope(a, 0.18, 0.8, 0.22); }, 0.3),
      this.act(0.01, () => {}, 0.25, () => Sound.play("greet")),
      this.act(0.01, () => {}, 1.5, () => this.glide("hands", 0, 220)),
    ];
  }

  interruptGreet() {
    if (this.hands <= 0.01 && now() >= this.waveUntil) return;
    this.waveUntil = 0;
    this.waveStart = 0;
    const gone = new Set(this.greetActs);
    this.acts = this.acts.filter((a) => !gone.has(a));
    this.greetActs = [];
    this.glide("hands", 0, 150);
  }

  setPermanentEmote(emote: BotEmoteName | null) {
    this.permanentEmote = emote;
    this.miniNextBehavior = now() + rand(0.8, 2.5);
    if (emote === "wink") return;
    this.permanentEye = emote ? EMOTE_EYE[emote] : null;
    if (this.permanentEye) {
      this.eyeOverride = this.permanentEye;
      this.eyeOverrideUntil = Number.POSITIVE_INFINITY;
    } else if (this.eyeOverrideUntil === Number.POSITIVE_INFINITY) {
      this.eyeOverride = null;
      this.eyeOverrideUntil = 0;
    }
  }

  triggerEmote(emote: BotEmoteName, duration = 1.8) {
    const t = now();
    this.eyeOverride = EMOTE_EYE[emote];
    this.eyeOverrideUntil = t + duration;
    const d = Math.max(0.6, duration);

    switch (emote) {
      case "happy":
        this.puff();
        this.act(0.01, () => {}, 0.12, () => this.pop(3));
        this.glide("blush", 0.5, 200, () => this.glide("blush", 0, 600));
        break;
      case "love":
        this.glide("blush", 0.9, 300, () => this.act(0.01, () => {}, d - 0.6, () => this.glide("blush", 0, 300)));
        this.emit(this.elementParticle, 5, true);
        this.act(d, (a, o) => { o.tilt -= 0.06 * envelope(a, 0.3, d - 0.7, 0.4); o.oy -= 0.03 * envelope(a, 0.3, d - 0.7, 0.4); });
        break;
      case "surprised":
        // Eyes snap open, the outline goes taut, wisps flare; each settles at its own speed.
        this.act(0.9, (a, o) => {
          o.es += 0.25 * envelope(a, 0.06, 0.12, 0.6);
          o.bow -= 0.08 * envelope(a, 0.06, 0.05, 0.35);
          o.flare += envelope(a, 0.07, 0.15, 0.7);
          o.oy -= 0.08 * envelope(a, 0.05, 0.05, 0.3);
        });
        break;
      case "proud":
        // He rises on the in-breath, holds it, and one halo ring opens outward.
        this.heldBreath = t + d;
        this.act(d, (a, o) => {
          const k = envelope(a, 0.45, Math.max(0.8, d - 0.9), 0.45);
          o.oy -= 0.05 * k; o.sy += 0.035 * k; o.sx += 0.02 * k; o.pitch += 0.06 * k; o.flare += 0.4 * k;
        });
        this.emit("ring", 1);
        this.glide("blush", 0.5, 250, () => this.act(0.01, () => {}, d - 0.5, () => this.glide("blush", 0, 300)));
        break;
      case "wink": {
        // One eye shutters; the wisp on the other side flicks.
        this.act(d, (a, o) => {
          o.tilt += 0.09 * envelope(a, 0.14, d - 0.4, 0.26);
          o.flickL += envelope(a - 0.05, 0.08, 0.05, 0.3);
        });
        break;
      }
      case "yawn":
        this.yawnMove(d);
        break;
      case "annoyed":
        this.eyeOverrideUntil = t + Math.min(d, 0.9);
        this.twitch(3);
        this.act(0.01, () => {}, 0.06, () => Sound.play("annoyed"));
        break;
    }
  }

  /** The element's own particle: leaf flecks, embers or bubbles. */
  private get elementParticle(): ParticleKind {
    const e = this.element;
    return e === "flame" ? "ember" : e === "water" ? "bubble" : "fleck";
  }

  /** Happy puff: corners fill fast, he lifts 6 % and settles on the spring. */
  private puff() {
    this.act(0.7, (a, o) => {
      o.bow -= 0.05 * envelope(a, 0.09, 0.08, 0.4);
      o.oy -= 0.06 * envelope(a, 0.09, 0.05, 0.25);
      o.sy += 0.04 * envelope(a, 0.09, 0, 0.3);
      o.flare += 0.6 * envelope(a, 0.1, 0.05, 0.4);
    });
  }

  /** The element pops: two leaves, a burst of sparks or three bubbles. */
  private pop(count: number) {
    const kind = this.elementParticle;
    this.emit(kind, kind === "fleck" ? 2 : kind === "bubble" ? 3 : count);
  }

  /** Three short sideways twitches 40 ms apart; the wisps drop. */
  private twitch(n: number) {
    const step = 0.04 + 0.05;
    this.act(n * step + 0.5, (a, o) => {
      const i = Math.floor(a / step);
      if (i < n && a - i * step < 0.05) o.ox += (i % 2 ? -1 : 1) * 0.05;
      o.drop += envelope(a, 0.08, n * step, 0.4);
    });
  }

  /** Yawn: the whole pillow sags, eyes squeeze, the element droops, then a slow refill. */
  private yawnMove(d: number) {
    const sag = Math.min(1.2, d * 0.66);
    this.act(sag + 1.1, (a, o) => {
      const k = envelope(a, 0.35, sag - 0.35, 1.1);
      o.bow += 0.06 * k; o.sy -= 0.05 * k; o.sx += 0.035 * k; o.drop += k; o.pitch -= 0.05 * k;
    });
    this.act(0.01, () => {}, sag * 0.6, () => this.emit("z", this.lite ? 1 : 2));
  }

  /** Throws `count` particles of `kind` (at most 24 alive; half in lite). */
  emit(kind: ParticleKind, count: number, spiral = false) {
    this.onWake?.();
    const cap = this.lite ? 12 : 24;
    const n = Math.min(count, this.lite ? Math.ceil(count / 2) : count, cap - this.particles.length);
    for (let i = 0; i < n; i++) {
      const rises = kind !== "drop";
      this.particles.push({
        kind,
        x: kind === "z" ? 0.55 : kind === "ring" ? 0 : rand(-0.45, 0.45),
        y: kind === "ring" ? 0 : kind === "drop" ? -0.55 : -0.65 - Math.random() * 0.15,
        vx: kind === "z" ? 0.16 : kind === "ring" ? 0 : rand(-0.2, 0.2),
        vy: kind === "ring" ? 0 : rises ? -(kind === "ember" ? rand(0.6, 0.9) : kind === "bubble" ? rand(0.35, 0.5) : rand(0.25, 0.4)) : 0,
        age: -i * 0.11,
        life: kind === "ring" ? 0.9 : kind === "ember" ? rand(0.8, 1.1) : rand(1.3, 1.8),
        seed: Math.random() * 10,
        size: kind === "ember" ? rand(0.05, 0.08) : rand(0.1, 0.15),
        spiral,
      });
    }
  }

  animateMorph(target: number, durationMs?: number) {
    this.glide("morph", target, durationMs ?? (target > 0.5 ? 520 : 600));
  }

  resetMorph() {
    this.glides.delete("morph");
    this.locks.delete("morph");
    this.morph = 0;
  }

  /** Switches Groovy's look, with a little squash when animated. */
  setSkin(next: Skin, animated = true) {
    if (next === this.skin) return;
    this.skin = next;
    if (animated) this.squash();
  }

  /** Dresses Groovy: the old outfit lifts off, the new one settles on with a squash. */
  setOutfit(next: Outfit, animated = true) {
    if (next === this.outfitTarget) return;
    this.outfitTarget = next;
    this.glides.delete("outfitPresence");
    this.locks.delete("outfitPresence");
    const enter = () => {
      this.outfit = next;
      this.glide("outfitPresence", 1, 340, () => this.squash());
    };
    if (!animated) {
      this.outfit = next;
      this.outfitPresence = next !== "none" ? 1 : 0;
    } else if (next === "none") {
      this.glide("outfitPresence", 0, 170, () => { this.outfit = "none"; });
    } else if (this.outfit === "none") {
      this.outfitPresence = 0;
      enter();
    } else {
      this.glide("outfitPresence", 0, 170, enter);
    }
  }

  /** Wearing something visible: the body then turns as one piece when it rolls. */
  private get rigidRoll(): boolean {
    return !this.isMini && this.outfit !== "none" && this.outfitPresence > 0.05;
  }

  /** True while anything is still moving — lets the island stop its frame loop. */
  get busy(): boolean {
    return this.ambient || this.moving;
  }

  /** A looping state (the scan, the lean's halo beat, sleep, the rate-limit drops): it never ends by itself. */
  get ambient(): boolean {
    return this.cfg.loops || this.isMini;
  }

  /** Something on its way somewhere: a glide, an act, particles, a spring not yet still… */
  get moving(): boolean {
    const off = (p: "yaw" | "pitch" | "tilt" | "sx" | "sy" | "es", tg: number) => this[p] !== tg || (this.vel[p] ?? 0) !== 0;
    return (
      this.glides.size > 0 || this.acts.length > 0 || this.particles.length > 0 ||
      this.isDancing || this.dancingLevel > 0 ||
      off("yaw", this.tgYaw) || off("pitch", this.tgPitch) || off("tilt", this.tgTilt) ||
      off("sx", this.tgSx) || off("sy", this.tgSy) || off("es", this.tgEs) ||
      this.oy !== 0 || this.ox !== 0 || this.bow !== PILLOW_BOW ||
      this.slotH > 0 || this.slotV !== 0 ||
      this.eyeX.vel !== 0 || this.eyeY.vel !== 0 || this.badgeSwing.v !== 0 ||
      this.col !== this.colT ||
      (this.outfit !== "none" && (this.softX.vel !== 0 || this.softY.vel !== 0 || this.physDx !== 0 || this.physDy !== 0 || this.swing !== 0 || this.swingS.vel !== 0))
    );
  }

  // ── Update ──────────────────────────────────────────────────────────────────

  update(dt: number) {
    const n = now();
    dt = clamp(dt, 0, 0.1);

    for (const [prop, g] of [...this.glides]) {
      const k = (n - g.t0) / g.dur;
      this[prop] = g.from + (g.to - g.from) * easeOut(k);
      if (k >= 1) {
        this[prop] = g.to;
        this.glides.delete(prop);
        this.locks.delete(prop);
        g.done?.();
      }
    }

    const o = BotEngine.zero();
    if (this.acts.length) {
      const live: Act[] = [];
      const finished: Act[] = [];
      for (const a of this.acts) {
        const age = n - a.t0;
        if (age < 0) live.push(a);
        else if (age >= a.dur) finished.push(a);
        else { a.fx(age, o); live.push(a); }
      }
      this.acts = live;
      for (const a of finished) a.done?.();
    }

    const t = n - this.t0;
    const shown: EyeShape = this.eyeOverride ?? this.cfg.eye;

    // Where he looks: the pointer, or what the state is doing.
    let gx = this.lookX;
    let gy = this.lookY;
    const moved = Math.abs(gx - this.lastLook.x) + Math.abs(gy - this.lastLook.y) > 0.004;
    this.lastLook.x = gx; this.lastLook.y = gy;
    this.lastLook.still = moved ? 0 : this.lastLook.still + dt;
    if (this.state === "thinking") {
      const ph = (t / 2.8) * Math.PI * 2;
      gx = gx * 0.25 + Math.sin(ph) * 0.35;
      gy = gy * 0.25 - 0.55 + Math.sin(ph * 2) * 0.12;
    } else if (this.state === "searching") {
      gx = Math.sin((t / 2.4) * Math.PI * 2) * 0.7;
      gy = -0.08 + Math.cos((t / 2.4) * Math.PI * 4) * 0.05;
    } else if (this.state === "sleeping") {
      gx = 0; gy = -0.3;
    } else if (this.state === "dizzy") {
      gx = Math.sin(t * 7.5) * 0.3;
    } else if (this.isMini) {
      if (n > this.glance.next) {
        this.glance.x = rand(-0.85, 0.85);
        this.glance.y = rand(-0.5, 0.45);
        this.glance.next = n + rand(0.6, 2.2);
      }
      gx = this.glance.x; gy = this.glance.y;
    }

    // Eyes lead (fast and crisp), the head follows a beat later on a softer spring.
    this.tgYaw = gx * 0.62 + o.yaw;
    this.tgPitch = gy * 0.5 + this.cfg.lean + o.pitch;
    this.tgTilt = this.cfg.tilt;
    this.tgSx = 1;
    this.tgSy = 1;
    this.tgEs = 1;
    this.chase("yaw", this.tgYaw, 1.9, 0.82, dt);
    this.chase("pitch", this.tgPitch, 1.9, 0.82, dt);
    stepSpring(this.eyeX, clamp((this.tgYaw - this.yaw) * 0.22, -0.1, 0.1), 9, 0.9, dt, 0.0004);
    stepSpring(this.eyeY, clamp((this.tgPitch - this.pitch) * 0.18, -0.08, 0.08), 9, 0.9, dt, 0.0004);

    this.chase("tilt", this.tgTilt + o.tilt, 1.5, 0.5, dt);
    this.chase("sx", 1 + o.sx, 4.2, 0.42, dt);
    this.chase("sy", 1 + o.sy, 4.2, 0.42, dt);
    this.chase("oy", o.oy, 3.4, 0.5, dt);
    this.chase("ox", o.ox, 9, 0.45, dt);
    this.chase("es", 1 + o.es, 5, 0.5, dt);
    const bow = { v: this.bow, vel: this.vel.bow ?? 0 };
    stepSpring(bow, PILLOW_BOW + o.bow, 3.2, 0.55, dt, 0.0003);
    this.bow = bow.v; this.vel.bow = bow.vel;
    if (!this.locks.has("open")) this.open = clamp(1 - o.close + o.over, 0.04, 1.08);
    this.glow = 1 - 0.75 * clamp(o.glow, 0, 1);

    // The colour drifts to the state's.
    if (this.col !== this.colT) {
      const k = 1 - Math.exp(-dt * 3.2);
      const c = mix3(this.col, this.colT, k);
      this.col = Math.abs(c[0] - this.colT[0]) + Math.abs(c[1] - this.colT[1]) + Math.abs(c[2] - this.colT[2]) < 0.006 ? this.colT : c;
    }

    // Blinks every 3.5–9 s, never at the top of a breath; a double one time in eight.
    if (n > this.nextBlink) {
      const calm = shown === "pill" || shown === "wide" || shown === "tired";
      if (calm && this.state !== "sleeping" && this.state !== "dizzy") {
        this.blink();
        if (Math.random() < 1 / 8) this.act(0.01, () => {}, 0.24, () => this.blink());
      }
      this.nextBlink = n + rand(3.5, 9);
      if (breath(this.nextBlink - this.t0) > 0.97) this.nextBlink += 0.4;
    }

    // At rest, the element makes one small gesture every 20–40 s.
    if (!this.isMini && this.state === "idle" && n > this.nextGesture) {
      this.nextGesture = n + rand(20, 40);
      this.act(1.6, (a, o2) => { o2.flickR += envelope(a, 0.4, 0.5, 0.7); o2.drop -= 0.3 * envelope(a, 0.4, 0.5, 0.7); });
    }
    // Eyes glance: a small saccade when the pointer comes to rest.
    if (!this.isMini && this.lastLook.still > 0.6 && this.lastLook.still - dt <= 0.6) {
      this.eyeX.vel += (Math.random() - 0.5) * 0.6;
    }

    if (this.eyeOverride && n > this.eyeOverrideUntil) {
      this.eyeOverride = this.permanentEye;
      if (this.permanentEye) this.eyeOverrideUntil = Number.POSITIVE_INFINITY;
    }

    if (this.isMini && n > this.miniNextBehavior) this.miniCue();

    if (n > this.nextAmbient) {
      this.nextAmbient = n + 1.6;
      if (this.state === "sleeping") this.emit("z", 1);
      if (!this.isMini && this.state === "ratelimit" && Math.random() < 0.5) this.emit("drop", 1);
    }

    this.stepParticles(dt);

    // The drop slot.
    const slot = { v: this.slotH, vel: this.slotV };
    stepSpring(slot, this.slotHTarget, 3.6, 0.6, dt, 0.0005);
    this.slotH = Math.max(0, slot.v);
    this.slotV = slot.vel;

    this.dancingLevel = stepDanceLevel(this.dancingLevel, this.isDancing, dt);

    // Wisps chase their pose on springs: they arrive late and overshoot.
    const targets = this.wispTargets(t, o);
    for (let i = 0; i < 2; i++) {
      const w = this.wisp[i];
      stepSpring(w.x, targets[i].x, 3.4, 0.55, dt, 0.0008);
      stepSpring(w.y, targets[i].y, 3.4, 0.55, dt, 0.0008);
      stepSpring(w.r, targets[i].r, 2.8, 0.5, dt, 0.001);
    }

    // Badge string: a pendulum driven by the soft-part lag.
    stepSpring(this.badgeSwing, this.badge ? clamp(-this.physDx * 0.5 - this.tilt, -0.6, 0.6) : 0, 1.1, 0.25, dt, 0.0008);

    // Worn soft parts lag behind turns, hops and rolls; the pendulum swings past rest.
    if (dt > 0) {
      const yawVel = (this.yaw - this.prevYaw) / dt;
      const oyVel = (this.oy - this.prevOy) / dt;
      const dRoll = this.roll - this.prevRoll;
      const rollVel = Math.abs(dRoll) > Math.PI ? 0 : dRoll / dt;
      const spin = this.rigidRoll ? rollVel * 0.16 : 0;
      const tDx = clamp(-yawVel * 0.32 - this.tilt * 1.8 + spin, -1, 1);
      const tDy = clamp(oyVel * 0.45, -1, 1);
      this.softX.v = this.physDx;
      this.softY.v = this.physDy;
      stepSpring(this.softX, tDx, 1.3, 0.55, dt, 0.0004);
      stepSpring(this.softY, tDy, 1.3, 0.55, dt, 0.0004);
      this.physDx = this.softX.v;
      this.physDy = this.softY.v;
      if (this.outfit === "none" || this.lite) {
        this.swing = 0;
        this.swingS.v = this.swingS.vel = 0;
      } else {
        this.swingS.v = this.swing;
        stepSpring(this.swingS, clamp(-this.physDx * 0.9, -0.6, 0.6), 0.95, 0.27, dt, 0.0015);
        this.swing = this.swingS.v;
      }
    }
    this.prevYaw = this.yaw;
    this.prevOy = this.oy;
    this.prevRoll = this.roll;

    this.lastTime = n;
  }

  private stepParticles(dt: number) {
    if (!this.particles.length) return;
    for (const p of this.particles) {
      p.age += dt;
      if (p.age <= 0) continue;
      if (p.kind === "drop") p.vy += 1.6 * dt;
      else if (p.kind === "bubble") p.vx = Math.sin(p.age * 5 + p.seed) * 0.12;
      else if (p.kind === "fleck") p.vx = Math.sin(p.age * 3.2 + p.seed) * 0.22;
      else if (p.kind === "ember") p.vx = flicker(p.age * 0.6, p.seed) * 0.18;
      p.x += p.vx * dt;
      p.y += p.vy * dt;
    }
    this.particles = this.particles.filter((p) => p.age < p.life);
  }

  /** Minis show their emote in small cues of their own. */
  private miniCue() {
    const n = now();
    switch (this.permanentEmote) {
      case "happy":
        this.puff();
        this.miniNextBehavior = n + rand(2.2, 3.4);
        break;
      case "annoyed":
        this.twitch(3);
        this.miniNextBehavior = n + rand(3, 5.5);
        break;
      case "wink":
        this.eyeOverride = "wink";
        this.eyeOverrideUntil = n + 0.55;
        this.act(0.7, (a, o) => { o.tilt += 0.12 * envelope(a, 0.1, 0.3, 0.3); });
        this.miniNextBehavior = n + rand(2.2, 4.2);
        break;
      case "love":
        this.emit("fleck", 2, true);
        this.act(0.8, (a, o) => { o.tilt += 0.09 * Math.sin(a * Math.PI * 2.5) * (1 - a / 0.8); });
        this.miniNextBehavior = n + rand(2.6, 4.1);
        break;
      default:
        this.miniNextBehavior = n + rand(3, 5);
    }
  }

  // ── Dancing ─────────────────────────────────────────────────────────────────

  setDancing(dancing: boolean) {
    this.isDancing = dancing;
  }

  /** Rocks him from the base of his body. Call on a saved context before draw(). */
  applyDance(x: CanvasRenderingContext2D, W: number, H: number) {
    if (this.dancingLevel <= 0) return;
    const R = W * 0.3;
    const px = W / 2 + this.ox * R;
    const py = H / 2 + this.particleOverhang / 2 + this.oy * R + R * 0.06 + R * 0.88;
    const d = danceTransform(now(), this.dancingLevel, R);
    x.translate(px + d.dx, py + d.dy);
    x.rotate(d.rotate);
    x.scale(d.sx, d.sy);
    x.translate(-px, -py);
  }

  // ── Draw ────────────────────────────────────────────────────────────────────

  /** The head shape of the current look (minis use the same pillow shape). */
  private get shape() {
    return this.isMini ? SHAPE : this.style.shape;
  }

  /**
   * Draws wisps, body, eyes, mouth, tag and particles into a canvas of `W`×`H`
   * CSS pixels (the caller has already applied the DPR transform).
   */
  draw(x: CanvasRenderingContext2D, W: number, H: number) {
    const R = W * 0.3;
    const rx = R * 1.14 * this.shape.w;
    const ry = R * 0.88 * this.shape.h;
    const cx = W / 2 + this.ox * R;
    const cy = H / 2 + this.particleOverhang / 2 + this.oy * R + R * 0.06;
    const n = now();
    const t = n - this.t0;
    // The pillow breath: the height changes under 1.5 %; he holds it while proud.
    const held = n < this.heldBreath;
    const b = this.lite ? 0 : held ? 1 : breath(t, this.state === "sleeping" ? 6.5 : 4.2);
    const bAmp = this.isMini ? 2.5 : 1;

    x.save();
    if (this.rigidRoll && Math.abs(this.roll) > 0.001) {
      x.translate(cx, cy);
      x.rotate(this.roll);
      x.translate(-cx, -cy);
    }

    if (!this.isMini) this.drawHalo(x, R, rx, ry, cx, cy, b, t);
    this.drawWisps(x, R, rx, ry, cx, cy, t);

    x.save();
    x.translate(cx, cy);
    if (this.tilt !== 0) x.rotate(this.tilt);
    x.scale(this.sx * (1 + 0.008 * b * bAmp), this.sy * (1 + 0.012 * b * bAmp));

    const dressed = !this.isMini && this.outfit !== "none";
    const head = dressed ? makeHead(R, this.yaw, this.pitch, this.physDx, this.physDy) : null;
    const outfitState = { presence: this.outfitPresence, morph: this.morph };
    const skinHead = this.isMini ? null : { ...makeHead(R, this.yaw, this.pitch, this.physDx, this.physDy), rx, ry };
    if (head) {
      head.rx = rx;
      head.ry = ry;
      head.eyes = this.eyeCentres(rx, ry);
      head.swing = this.swing;
    }
    // What grows on his head comes up through a hat, raised by the hat's height.
    const lift = head && dressed ? growthLift([this.outfit, ...this.layers], head) * Math.min(1, this.outfitPresence) : 0;
    const skinState: SkinState = { hatless: 1, shown: Math.max(0, 1 - this.morph * 2), time: n, lite: this.lite };
    if (head) {
      drawOutfitBehind(x, this.outfit, head, outfitState);
      for (const layer of this.layers) if (layer !== this.outfit) drawOutfitBehind(x, layer, head, outfitState);
    }

    const body = this.bodyPath(rx, ry, R);
    if (skinHead) drawSkinBehind(x, this.style, skinHead, skinState);
    this.drawBody(x, body, R, rx, ry);
    if (skinHead) drawSkinBody(x, this.style, body, skinHead, skinState);
    this.drawGlowCheeks(x, body, R, rx, ry);

    this.drawEyes(x, body, R, rx, ry);
    if (this.morph > 0.05) this.drawSlot(x, body, R);

    if (skinHead && skinState.shown > 0.05) this.drawSkinMouth(x, R, rx, ry);
    if (skinHead && lift < 0.5) drawSkinFront(x, this.style, skinHead, skinState);
    if (head) {
      drawOutfitFront(x, this.outfit, head, outfitState);
      for (const layer of this.layers) if (layer !== this.outfit) drawOutfitFront(x, layer, head, outfitState);
    }
    if (skinHead && lift >= 0.5) {
      x.save();
      x.translate(0, -lift);
      drawSkinFront(x, this.style, skinHead, skinState);
      x.restore();
    }

    x.restore();
    x.restore();

    if (this.badge && this.badgeS > 0.01 && this.morph < 0.25) this.drawTag(x, this.badge, R, rx, ry, cx, cy);
    this.drawParticles(x, R, cx, cy);
  }

  /** The pillow outline at the current bow (the look's own outline when at rest). */
  private bodyPath(rx: number, ry: number, R: number): Path2D {
    const steps = 72;
    const exp = 2 / this.shape.exp;
    const taper = this.shape.taper;
    const bow = this.bow;
    const m = this.morph;
    const p = new Path2D();
    for (let i = 0; i <= steps; i++) {
      const a = (i / steps) * Math.PI * 2;
      const ca = Math.cos(a);
      const sa = Math.sin(a);
      const u = Math.sign(ca) * Math.pow(Math.abs(ca), exp);
      const v = Math.sign(sa) * Math.pow(Math.abs(sa), exp);
      let px = rx * u * (1 - bow * (1 - v * v)) * (1 + taper * Math.max(0, v));
      let py = ry * v * (1 - bow * (1 - u * u));
      if (m >= 0.005) {
        const rr = rrPoint(ca, sa, R, R * 0.94, R * 0.42);
        px = lerp(px, rr.x, m);
        py = lerp(py, rr.y, m);
      }
      if (i === 0) p.moveTo(px, py);
      else p.lineTo(px, py);
    }
    p.closePath();
    return p;
  }

  private drawBody(x: CanvasRenderingContext2D, body: Path2D, R: number, rx: number, ry: number) {
    if (this.bodyColor) {
      // Minis: one flat colour.
      x.fillStyle = rgba(this.bodyColor, 1);
      x.fill(body);
      return;
    }
    if (!this.drawCachedBody(x, body, R, rx, ry)) this.fillBody(x, body, R, rx, ry);
    const wash = this.tint * (1 - this.morph);
    if (wash > 0.01) {
      // The state's colour rises from below like light through the stuffing.
      x.save();
      const g = x.createRadialGradient(0, ry * 0.9, R * 0.1, 0, ry * 0.4, R * 1.4);
      g.addColorStop(0, rgba(this.col, 0.62 * wash));
      g.addColorStop(1, rgba(this.col, 0));
      x.fillStyle = g;
      x.fill(body);
      x.restore();
    }
  }

  /** The body's gradient fill with its two glows (the costly part: two shadow blurs). */
  private fillBody(x: CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D, body: Path2D, R: number, rx: number, ry: number) {
    const sk = this.style;
    const g = x.createLinearGradient(rx * 0.7, -ry * 0.85, -rx * 0.8, ry * 0.9);
    g.addColorStop(0, rgba(sk.bodyTop));
    g.addColorStop(1, rgba(sk.bodyBottom));
    x.fillStyle = g;
    x.save();
    x.shadowColor = sk.rimGlow;
    // A wide soft halo, then a tighter bright one.
    x.shadowBlur = R * 0.6 * (1 - this.morph);
    x.fill(body);
    x.shadowBlur = R * 0.18 * (1 - this.morph);
    x.fill(body);
    x.restore();
    x.fill(body);
  }

  /** The key of the body last drawn live, and the cached picture of a body that held still. */
  private bodyKey = "";
  private bodyCache: { key: string; canvas: OffscreenCanvas; left: number; top: number; scale: number } | null = null;

  /**
   * Round 3B item 8b: the glowing body is drawn once into a bitmap and reused
   * while its shape, size, colours and scale stay the same (breathing and
   * bouncing move it by transform, which the bitmap follows). A body that is
   * changing shape is drawn live, and cached again once it holds still for
   * two frames. False when it must be drawn live.
   */
  private drawCachedBody(x: CanvasRenderingContext2D, body: Path2D, R: number, rx: number, ry: number): boolean {
    if (typeof OffscreenCanvas === "undefined" || typeof x.getTransform !== "function") return false;
    const m = x.getTransform();
    // Bitmaps a quarter step apart: breathing doesn't re-cache every frame.
    const scale = Math.max(0.25, Math.round(Math.hypot(m.a, m.b) * 4) / 4);
    const sk = this.style;
    const key = `${rx.toFixed(2)}|${ry.toFixed(2)}|${R.toFixed(2)}|${this.morph.toFixed(3)}|${this.shape.exp}|${this.shape.taper}|${scale}|${String(sk.bodyTop)}|${String(sk.bodyBottom)}|${sk.rimGlow}|${this.bow.toFixed(4)}`;
    if (this.bodyCache?.key !== key) {
      if (this.bodyKey !== key) {
        this.bodyKey = key;
        return false;
      }
      // Shadow blur is in device pixels: in drawing units it grows as the scale shrinks.
      const pad = (R * 0.6 * 2) / Math.min(1, scale) + 4;
      const halfW = Math.max(rx * (1 + Math.max(0, this.shape.taper)), R) + pad;
      const halfH = Math.max(ry, R) + pad;
      const canvas = new OffscreenCanvas(Math.ceil(halfW * 2 * scale), Math.ceil(halfH * 2 * scale));
      const c = canvas.getContext("2d");
      if (!c) return false;
      c.setTransform(scale, 0, 0, scale, halfW * scale, halfH * scale);
      this.fillBody(c, body, R, rx, ry);
      this.bodyCache = { key, canvas, left: -halfW, top: -halfH, scale };
    }
    const b = this.bodyCache!;
    x.drawImage(b.canvas, b.left, b.top, b.canvas.width / b.scale, b.canvas.height / b.scale);
    return true;
  }

  /** Where his eyes are (body space), as drawEyes puts them: left, then right. */
  eyeCentres(rx: number, ry: number): [[number, number], [number, number]] {
    const face = this.isMini ? null : this.style.face;
    const at = (sd: number): [number, number] => {
      const eyeYaw = sd * (face?.eyeSp ?? EYE_SP) + this.yaw;
      const eyePitch = this.eyePitch(face);
      return [Math.sin(eyeYaw) * Math.cos(eyePitch) * rx, -Math.sin(eyePitch) * ry + (this.morph > 0 ? ry * 0.14 * this.morph : 0)];
    };
    return [at(-1), at(1)];
  }

  private eyePitch(face: SkinStyle["face"] | null | undefined): number {
    // Rolling with an outfit on, the whole body turns: the eyes must not roll again.
    const p = EYE_P - (face?.eyeY ?? 0) + this.pitch + (this.rigidRoll ? 0 : this.roll);
    return (((p + Math.PI) % (Math.PI * 2)) + Math.PI * 2) % (Math.PI * 2) - Math.PI;
  }

  /** Soft cheeks in the state colour (or pink for love), under the eyes. */
  private drawGlowCheeks(x: CanvasRenderingContext2D, body: Path2D, R: number, rx: number, ry: number) {
    const k = Math.max(this.blush, this.tint * 0.35) * (1 - this.morph);
    if (k <= 0.02) return;
    x.save();
    x.clip(body);
    const shift = Math.sin(this.yaw) * rx * 0.8;
    const c: RGB = this.blush > this.tint * 0.35 ? [1, 0.5, 0.62] : this.col;
    for (const sd of [-1, 1]) {
      const px = sd * rx * 0.52 + shift;
      const py = ry * 0.22;
      const g = x.createRadialGradient(px, py, 0, px, py, R * 0.2);
      g.addColorStop(0, rgba(c, 0.45 * k));
      g.addColorStop(1, rgba(c, 0));
      x.fillStyle = g;
      x.fillRect(px - R * 0.2, py - R * 0.2, R * 0.4, R * 0.4);
    }
    x.restore();
  }

  private drawEyes(x: CanvasRenderingContext2D, body: Path2D, R: number, rx: number, ry: number) {
    let shape: EyeShape = this.eyeOverride ?? this.cfg.eye;
    if (this.isDancing && this.dancingLevel > 0.15 && !this.isMini && (this.state === "idle" || this.state === "finished")) shape = "happy";
    if (this.morph > 0.5) {
      if (this.isChewing) shape = "happy";
      else if (this.slotHTarget > 0.05 || this.slotH > 0.1) shape = "cup";
    }

    x.save();
    x.clip(body);
    const sk = this.isMini ? null : this.style;
    const ink = this.isMini ? MINI_INK : sk?.eyeInk ?? INK;
    x.fillStyle = ink;
    x.strokeStyle = ink;
    if (sk?.eyeGlow) {
      x.shadowColor = sk.eyeGlow;
      x.shadowBlur = R * 0.16 * this.glow;
    }

    const face = sk?.face;
    const [l, r] = this.eyeCentres(rx, ry);
    const lead = [this.eyeX.v * R, this.eyeY.v * R];
    for (const sd of [-1, 1]) {
      const eyeYaw = sd * (face?.eyeSp ?? EYE_SP) + this.yaw;
      const cp = Math.cos(this.eyePitch(face));
      if (Math.cos(eyeYaw) * cp <= 0.04) continue;
      const [ex, ey] = sd < 0 ? l : r;
      const fx = lerp(Math.max(0.18, Math.cos(eyeYaw)), 1, this.morph * 0.7);
      const fy = lerp(Math.max(0.18, cp), 1, this.morph * 0.7);
      const size = this.isMini ? 1.9 : 1;
      const calm = shape === "pill" || shape === "wide" || (shape === "wink" && sd < 0);
      const fw = sk && calm ? face!.eyeW : Math.min(face?.eyeW ?? 1, 1.15);
      const fh = sk && calm ? face!.eyeH : Math.min(face?.eyeH ?? 1, 1.15);
      const ew = R * 0.25 * this.es * size * fw * (shape === "wide" ? 1.12 : 1);
      const eh = R * 0.27 * this.es * size * fh * (shape === "wide" ? 1.1 : 1);

      x.save();
      x.translate(ex + lead[0], ey + lead[1]);
      x.scale(fx, fy);
      if (face && calm && face.eyeTilt) x.rotate(sd * face.eyeTilt);
      if (sk && calm) drawAlmondEye(x, sk, sd, ew, eh, this.open);
      else {
        // Mood glyphs are cut dark into the glowing body, so they read at any size.
        if (sk) { x.fillStyle = INK; x.strokeStyle = INK; }
        this.drawGlyph(x, shape, ew, eh, sd, sk ? INK : ink);
        x.fillStyle = ink; x.strokeStyle = ink;
      }
      x.restore();
    }
    x.restore();
  }

  /** Eye shapes for moods (Groovy's own set): crescents, flat lids, sparks. */
  private drawGlyph(x: CanvasRenderingContext2D, shape: EyeShape, w: number, h: number, sd: number, ink: string) {
    const t = now();
    switch (shape) {
      case "pill":
      case "wide":
        capsule(x, w, Math.max(h * this.open, w * 0.28));
        x.fill();
        break;
      case "wink":
        if (sd < 0) { capsule(x, w, Math.max(h * this.open, w * 0.28)); x.fill(); }
        else this.crescent(x, w, h, true);
        break;
      case "dot":
        x.beginPath();
        x.ellipse(0, 0, w * 0.4, w * 0.46, 0, 0, Math.PI * 2);
        x.fill();
        break;
      case "line":
      case "flat": {
        // Lids flatten to a straight top edge (sloped in when annoyed).
        const slope = shape === "line" ? sd * h * 0.12 : 0;
        x.beginPath();
        x.moveTo(-w * 0.68, -h * 0.08 - slope);
        x.lineTo(w * 0.68, -h * 0.08 + slope);
        x.quadraticCurveTo(w * 0.5, h * 0.34, 0, h * 0.34);
        x.quadraticCurveTo(-w * 0.5, h * 0.34, -w * 0.68, -h * 0.08 - slope);
        x.fill();
        break;
      }
      case "happy":
        this.crescent(x, w, h, true);
        break;
      case "closed":
        this.crescent(x, w, h, false);
        break;
      case "tired":
        // Half-lidded: the lower half of the eye under a straight lid.
        x.beginPath();
        x.moveTo(-w * 0.55, 0);
        x.lineTo(w * 0.55, 0);
        x.quadraticCurveTo(w * 0.45, h * 0.42, 0, h * 0.42);
        x.quadraticCurveTo(-w * 0.45, h * 0.42, -w * 0.55, 0);
        x.fill();
        break;
      case "spiral": {
        x.lineWidth = w * 0.18;
        x.lineCap = "round";
        x.beginPath();
        for (let i = 0; i <= 40; i++) {
          const a = (i / 40) * Math.PI * 5 + t * 7 * sd;
          const r = (i / 40) * w * 0.55;
          const px = Math.cos(a) * r;
          const py = Math.sin(a) * r;
          if (i === 0) x.moveTo(px, py); else x.lineTo(px, py);
        }
        x.stroke();
        break;
      }
      case "heart":
      case "star": {
        // A four-point glint that turns slowly; warm for "heart".
        x.fillStyle = shape === "heart" ? "rgb(255,120,150)" : "rgb(255,214,102)";
        x.rotate(t * 1.2 * sd);
        const s = w * 0.9;
        x.beginPath();
        for (let i = 0; i < 8; i++) {
          const a = (i * Math.PI) / 4;
          const r = i % 2 ? s * 0.32 : s;
          x.lineTo(Math.cos(a) * r, Math.sin(a) * r);
        }
        x.closePath();
        x.fill();
        x.fillStyle = ink;
        break;
      }
      case "cup": {
        const hh = Math.max(h * this.open, w * 0.3);
        x.beginPath();
        x.moveTo(-w / 2, -hh / 2);
        x.lineTo(w / 2, -hh / 2);
        x.bezierCurveTo(w / 2, hh * 0.7, -w / 2, hh * 0.7, -w / 2, -hh / 2);
        x.fill();
        break;
      }
    }
  }

  /** A filled crescent: smiling (up) for happy, hanging (down) for asleep. */
  private crescent(x: CanvasRenderingContext2D, w: number, h: number, up: boolean) {
    const s = up ? 1 : -1;
    x.beginPath();
    x.moveTo(-w * 0.62, h * 0.1 * s);
    x.quadraticCurveTo(0, -h * 0.62 * s, w * 0.62, h * 0.1 * s);
    x.quadraticCurveTo(0, -h * 0.22 * s, -w * 0.62, h * 0.1 * s);
    x.fill();
  }

  /** Groovy's mouth, below the eyes, following the head; its mood follows the eyes. */
  private drawSkinMouth(x: CanvasRenderingContext2D, R: number, rx: number, ry: number) {
    const shape: EyeShape = this.eyeOverride ?? this.cfg.eye;
    const mood: MouthMood =
      shape === "happy" || shape === "heart" || shape === "star" || shape === "wink" ? "happy"
      : shape === "wide" || shape === "spiral" ? "open"
      : this.state === "error" || shape === "tired" || shape === "line" ? "sad"
      : shape === "closed" ? "none"
      : "calm";
    const p = EYE_P - 0.42 + this.pitch;
    const yawM = this.yaw;
    const cp = Math.cos(p);
    if (Math.cos(yawM) * cp <= 0.2) return;
    const mx = Math.sin(yawM) * cp * rx;
    const my = -Math.sin(p) * ry;
    drawSkinMouth(x, this.style, R, mx, my, Math.max(0.3, Math.cos(yawM)), mood);
  }

  /** The drop slot cut into the morphed box: a dark opening with a lit lip. */
  private drawSlot(x: CanvasRenderingContext2D, body: Path2D, R: number) {
    const m = this.morph;
    const w = R * 1.7 * m;
    const h = this.slotH * R * m;
    const top = -R * 0.8 * m;
    x.save();
    x.clip(body);
    if (h > 0.8) {
      const g = x.createLinearGradient(0, top, 0, top + h);
      g.addColorStop(0, "rgb(4,8,6)");
      g.addColorStop(1, "rgb(14,24,20)");
      rrect(x, -w / 2, top, w, h, h / 2);
      x.fillStyle = g;
      x.fill();
      x.strokeStyle = `rgba(200,255,220,${0.3 * m})`;
      x.lineWidth = 1;
      x.beginPath();
      x.moveTo(-w / 2 + h / 2, top + h - 0.5);
      x.lineTo(w / 2 - h / 2, top + h - 0.5);
      x.stroke();
    }
    x.restore();
  }

  /** The halo: follows the breath, beats twice every 2 s in approval, and breaks into orbiting dots while thinking. */
  private drawHalo(x: CanvasRenderingContext2D, R: number, rx: number, ry: number, cx: number, cy: number, b: number, t: number) {
    if (this.morph > 0.3 || R <= 14) return;
    const sk = this.style;
    let k = 0.1 * b;
    if (this.state === "approval") {
      const ph = t % 2;
      k += 0.35 * (envelope(ph, 0.08, 0, 0.25) + envelope(ph - 0.32, 0.08, 0, 0.3));
    }
    k *= 1 - this.morph;
    if (this.state === "thinking") {
      const a0 = (t / 4.2) * Math.PI * 2;
      x.save();
      x.fillStyle = sk.rimGlow;
      for (let i = 0; i < 3; i++) {
        const a = a0 + (i * Math.PI * 2) / 3;
        x.globalAlpha = 0.55 + 0.35 * Math.sin(a);
        x.beginPath();
        x.arc(cx + Math.cos(a) * rx * 0.75, cy - ry * 1.18 + Math.sin(a) * ry * 0.16, R * 0.05, 0, Math.PI * 2);
        x.fill();
      }
      x.restore();
    }
    if (k <= 0.01) return;
    x.save();
    x.globalAlpha = Math.min(1, k);
    const g = x.createRadialGradient(cx, cy, Math.min(rx, ry) * 0.8, cx, cy, Math.max(rx, ry) * 1.35);
    g.addColorStop(0, sk.rimGlow);
    g.addColorStop(1, "rgba(0,0,0,0)");
    x.fillStyle = g;
    x.fillRect(cx - rx * 1.4, cy - ry * 1.4, rx * 2.8, ry * 2.8);
    x.restore();
  }

  /** Where the wisps want to be (x in rx, y in ry, + is down; r radians). */
  private wispTargets(t: number, o: Offsets): Array<{ x: number; y: number; r: number }> {
    const flare = o.flare;
    const drop = clamp(o.drop, -0.5, 1);
    const pose = (sd: number) => ({ x: sd * (1.08 + 0.14 * flare), y: 0.7 - 0.35 * flare + 0.12 * drop, r: -sd * 0.5 * flare + sd * 0.35 * drop });
    const w = [pose(-1), pose(1)];
    switch (this.state) {
      case "working": {
        // Knitting: two small alternating circles, body still.
        for (const [i, sd] of [[0, -1], [1, 1]] as const) {
          const a = t * 7 + (sd > 0 ? Math.PI : 0);
          w[i].x = sd * 0.98 + Math.cos(a) * 0.06;
          w[i].y = 0.55 + Math.sin(a) * 0.06;
          w[i].r = sd * 0.25;
        }
        break;
      }
      case "searching": {
        // The element leans and scans in an arc; the eyes follow it.
        const s = Math.sin((t / 2.4) * Math.PI * 2);
        w[1].x = 1.15; w[1].y = 0.15 - Math.abs(s) * 0.15; w[1].r = -0.6 + s * 0.5;
        break;
      }
      case "question":
        // The wisp curls into a hook.
        w[1].x = 1.12; w[1].y = -0.2; w[1].r = -1.1;
        break;
      case "approval":
        w[0].x = -1.0; w[1].x = 1.0; w[0].y = w[1].y = 0.5;
        break;
      case "error":
        w[0].y = w[1].y = 0.85; w[0].r = -0.3; w[1].r = 0.3;
        break;
      case "sleeping":
        w[0].y = w[1].y = 0.82; w[0].r = -0.5; w[1].r = 0.5;
        break;
      case "finished":
        w[0].y = w[1].y = 0.25; w[0].r = 0.5; w[1].r = -0.5;
        break;
    }
    if (o.flickL) w[0].r += 0.7 * o.flickL;
    if (o.flickR) w[1].r -= 0.7 * o.flickR;
    if (o.wave) {
      // Two beats: up and out, then a curl back.
      w[1].x = lerp(w[1].x, 1.25, o.wave);
      w[1].y = lerp(w[1].y, -0.35, o.wave);
    }
    return w;
  }

  /** The two element wisps, behind the body, each with its element's idle life. */
  private drawWisps(x: CanvasRenderingContext2D, R: number, rx: number, ry: number, cx: number, cy: number, t: number) {
    if (this.isMini || R <= 14) return;
    const shown = Math.max(this.hands, 0.85) * (1 - this.morph);
    if (shown <= 0.01) return;
    const kind = this.element;
    const size = 0.3 * ry * shown * 1.25 * WISP_SCALE;
    const n = now();
    const waving = this.waveStart > 0 && n >= this.waveStart && n < this.waveUntil;
    // While he tumbles, the wisps trail a quarter-turn behind.
    const prog = this.rollTotal > 0 ? clamp(this.roll / this.rollTotal, 0, 1) : 0;
    const trail = (this.rigidRoll ? 0 : this.roll) - (this.roll !== 0 ? (Math.PI / 2) * Math.sin(Math.PI * prog) : 0);
    const beat = this.dancingLevel > 0 ? Math.abs(Math.sin((n * DANCE_BPM / 60) * Math.PI)) * this.dancingLevel : 0;
    for (let i = 0; i < 2; i++) {
      const sd = i ? 1 : -1;
      const w = this.wisp[i];
      let lx = w.x.v * rx * this.sx;
      let ly = w.y.v * ry * this.sy - beat * ry * 0.12;
      let rot = w.r.v + sd * 0.35;
      // The element idles while the body rests.
      let sy = 1;
      if (kind === "leaf") rot += Math.sin((t / 5.5) * Math.PI * 2 + sd) * 0.105;
      else if (kind === "flame") { sy = 1 + flicker(t / 0.9, sd) * 0.08; rot += flicker(t * 0.7, sd + 3) * 0.05; }
      else ly += Math.sin((t / 2.4) * Math.PI * 2 + sd) * ry * 0.03;
      if (waving && sd > 0) {
        const wt = n - this.waveStart;
        rot += Math.sin(wt * Math.PI * 2 * 1.6) * 0.45 - 0.4;
      }
      const a = this.tilt + trail;
      const ca = Math.cos(a), sa = Math.sin(a);
      const wx = cx + ca * lx - sa * ly;
      const wy = cy + sa * lx + ca * ly;
      x.save();
      x.translate(wx, wy);
      x.rotate(rot);
      x.scale(1, sy);
      drawWisp(x, kind, size, R, this.bodyColor);
      x.restore();
    }
  }

  /** The state tag: hangs from his outline on a short string and swings. */
  private drawTag(x: CanvasRenderingContext2D, badge: Badge, R: number, rx: number, ry: number, cx: number, cy: number) {
    const s = this.badgeS;
    const ax = cx + rx * 0.62 * this.sx;
    const ay = cy - ry * 0.86 * this.sy;
    const len = R * 0.3;
    const a = this.badgeSwing.v + 0.35;
    x.save();
    x.translate(ax, ay);
    x.rotate(-a);
    x.strokeStyle = rgba(badge.color, 0.7 * s);
    x.lineWidth = Math.max(1, R * 0.025);
    x.beginPath();
    x.moveTo(0, 0);
    x.lineTo(0, -len * s);
    x.stroke();
    x.translate(0, -len * s);
    x.rotate(a);
    x.scale(s, s);
    const w = R * 0.34, h = R * 0.3;
    x.fillStyle = rgba(badge.color, 1);
    rrect(x, -w / 2, -h, w, h, R * 0.09);
    x.fill();
    x.fillStyle = "rgba(255,255,255,0.95)";
    const gy = -h / 2;
    const tt = now();
    switch (badge.kind) {
      case "dots":
        for (let i = -1; i <= 1; i++) {
          const k = 0.5 + 0.5 * Math.sin(tt * 6 - i * 0.9);
          x.globalAlpha = 0.45 + 0.55 * k;
          x.beginPath();
          x.arc(i * w * 0.24, gy - k * h * 0.06, R * 0.03, 0, Math.PI * 2);
          x.fill();
        }
        x.globalAlpha = 1;
        break;
      case "bang":
      case "question":
        x.font = `800 ${h * 0.78}px ${FONT}`;
        x.textAlign = "center";
        x.textBaseline = "middle";
        x.fillText(badge.kind === "bang" ? "!" : "?", 0, gy + h * 0.04);
        break;
      case "dot":
        x.beginPath();
        x.arc(0, gy, R * 0.055, 0, Math.PI * 2);
        x.fill();
        break;
    }
    x.restore();
  }

  /** Particles drawn from the look's own elements: leaf flecks, embers, bubbles. */
  private drawParticles(x: CanvasRenderingContext2D, R: number, cx: number, cy: number) {
    if (!this.particles.length) return;
    const u = R * 1.3;
    for (const p of this.particles) {
      if (p.age <= 0) continue;
      const k = p.age / p.life;
      const fade = k < 0.15 ? k / 0.15 : 1 - easeOut((k - 0.15) / 0.85);
      let px = p.x;
      if (p.spiral) px += Math.sin(p.age * 4 + p.seed) * 0.32 * (1 - k * 0.6);
      const X = cx + px * u;
      const Y = cy + p.y * u;
      const sz = R * p.size;
      x.save();
      x.translate(X, Y);
      x.globalAlpha = clamp(fade, 0, 1);
      switch (p.kind) {
        case "fleck":
          x.rotate(Math.sin(p.age * 3 + p.seed) * 0.9);
          x.fillStyle = "rgb(110,220,140)";
          x.fill(wispPath("leaf", sz));
          break;
        case "ember": {
          const g = x.createRadialGradient(0, 0, 0, 0, 0, sz * 1.6);
          g.addColorStop(0, "rgba(255,240,180,1)");
          g.addColorStop(0.4, "rgba(255,150,50,0.9)");
          g.addColorStop(1, "rgba(255,90,20,0)");
          x.fillStyle = g;
          x.fillRect(-sz * 1.6, -sz * 1.6, sz * 3.2, sz * 3.2);
          break;
        }
        case "bubble":
          x.strokeStyle = "rgba(170,225,255,0.95)";
          x.lineWidth = Math.max(1, sz * 0.14);
          x.beginPath();
          x.arc(0, 0, sz * 0.6, 0, Math.PI * 2);
          x.stroke();
          x.fillStyle = "rgba(255,255,255,0.8)";
          x.beginPath();
          x.arc(-sz * 0.22, -sz * 0.22, sz * 0.14, 0, Math.PI * 2);
          x.fill();
          break;
        case "drop":
          x.fillStyle = "rgb(140,205,255)";
          x.fill(wispPath("water", sz * 0.8));
          break;
        case "z":
          x.rotate(-0.2 + Math.sin(p.age * 2) * 0.15);
          x.fillStyle = "rgb(200,225,215)";
          x.font = `700 ${sz * (1.6 + k)}px ${FONT}`;
          x.textAlign = "center";
          x.textBaseline = "middle";
          x.fillText("z", 0, 0);
          break;
        case "ring": {
          x.strokeStyle = this.isMini ? "rgba(255,255,255,0.8)" : this.style.rimGlow;
          x.lineWidth = Math.max(1, R * 0.04 * (1 - k));
          x.beginPath();
          x.ellipse(0, 0, R * (1.05 + 0.6 * easeOut(k)), R * (0.95 + 0.5 * easeOut(k)), 0, 0, Math.PI * 2);
          x.stroke();
          break;
        }
        case "glint":
          x.fillStyle = "#fff";
          x.beginPath();
          x.arc(0, 0, sz * 0.3, 0, Math.PI * 2);
          x.fill();
          break;
      }
      x.restore();
    }
  }
}

/** Ray → rounded-rect boundary intersection, for the mailbox morph. */
function rrPoint(ca: number, sa: number, W: number, H: number, cr: number): { x: number; y: number } {
  const eps = 1e-6;
  const kx = ca >= 0 ? 1 : -1;
  const ky = sa >= 0 ? 1 : -1;
  const cx = kx * (W - cr);
  const cy = ky * (H - cr);

  const dot = ca * cx + sa * cy;
  const disc = dot * dot - (cx * cx + cy * cy - cr * cr);
  if (disc >= 0) {
    const t = dot + Math.sqrt(disc);
    if (t > eps) {
      const px = ca * t;
      const py = sa * t;
      if (Math.abs(px) >= W - cr - eps && Math.abs(py) >= H - cr - eps) return { x: px, y: py };
    }
  }
  if (Math.abs(sa) > eps) {
    const t = (ky * H) / sa;
    if (t > eps) {
      const px = ca * t;
      if (Math.abs(px) <= W - cr + eps) return { x: px, y: ky * H };
    }
  }
  if (Math.abs(ca) > eps) {
    const t = (kx * W) / ca;
    if (t > eps) {
      const py = sa * t;
      if (Math.abs(py) <= H - cr + eps) return { x: kx * W, y: py };
    }
  }
  return { x: kx * W, y: ky * H };
}
