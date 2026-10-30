// The launch intro, "Seed" (FIXES_ROUND3 item 7; mockup
// docs/design/island-round3/IntroSeed.dc.html), in place of Coucou's
// drop-and-wave greeting. A glowing seed in the look's colour falls into a
// small island, bounces, cracks into two halves with a soft flash, and Groovy
// pops out; then the island grows around him.
//
//   first    (first launch, ~3 s): out at stage 0, his first sprout, spark or
//            droplet appears, the island grows, "Hi, I'm Groovy." /
//            "I'll grow the longer we work together."
//   grew     (~2 s): out at the stage he had, then the new one grows in:
//            "I grew while you were away!"
//   back     (~1 s): out at his stage; the island then opens on Up next.
//   still    (reduced motion): no fall, no crack: he fades in, 0.5 s.
//
// Drawn with BotEngine on the island's greeting canvas (640×150); a click
// skips it; it is made for one launch and let go when it ends.

import { BotEngine } from "./engine";
import { SKIN_STYLES, type Skin } from "./skins";
import type { Appearance } from "./appearance";
import type { Outfit } from "./wardrobe";
import { Sound } from "../core/sound";

export type SeedKind = "first" | "grew" | "back" | "still";

export const SEED_W = 640;
export const SEED_H = 150;

/** The small island the seed falls into, before it grows (logical px). */
export const SEED_ISLAND = { w: 132, h: 118 };

/** Beat times in seconds, per kind. */
export interface SeedPlan {
  land: number;
  crack: number;
  pop: number;
  touch: number;
  grow: number;
  text: number;
  end: number;
}

export function seedPlan(kind: SeedKind): SeedPlan {
  switch (kind) {
    case "first":
      return { land: 0.6, crack: 1.2, pop: 1.25, touch: 1.62, grow: 1.86, text: 2.28, end: 3.0 };
    case "grew":
      return { land: 0.3, crack: 0.36, pop: 0.4, touch: 0.95, grow: 0.7, text: 1.0, end: 2.1 };
    case "back":
      return { land: 0.3, crack: 0.36, pop: 0.4, touch: Infinity, grow: 0.75, text: Infinity, end: 1.0 };
    case "still":
      return { land: 0, crack: 0, pop: 0, touch: Infinity, grow: 0, text: Infinity, end: 0.5 };
  }
}

/** Which intro: the first one ever, a stage reached since the last, or a plain hello. */
export function seedKind(seen: number, reached: number, reducedMotion: boolean): SeedKind {
  if (reducedMotion) return "still";
  if (seen < 0) return "first";
  return reached > seen ? "grew" : "back";
}

/** What is on screen at `t` seconds. */
export interface SeedPose {
  /** The seed: centre y, scale and opacity. */
  seedY: number;
  seedScale: number;
  seedAlpha: number;
  /** 0 → 1 as the two halves fly apart and fade. */
  halves: number;
  /** The flash: 0 → 1 → gone. */
  flash: number;
  /** Groovy's scale (1.15 overshoot) and opacity. */
  bot: number;
  botAlpha: number;
  /** Groovy's centre x in the 640 canvas: the middle, then aside for the text. */
  botX: number;
  /** The new touch (first) or the new stage (grew): 0 → 1. */
  touch: number;
  /** The text's opacity. */
  text: number;
  /** The island is full size from here on. */
  grown: boolean;
}

const clamp = (v: number, a = 0, b = 1) => Math.max(a, Math.min(b, v));
const seg = (t: number, a: number, b: number) => clamp((t - a) / Math.max(1e-6, b - a));
const lerp = (a: number, b: number, k: number) => a + (b - a) * k;
/** 0 → `peak` → 1: a pop with an overshoot. */
const overshoot = (k: number, peak = 1.15) => (k < 0.6 ? lerp(0, peak, k / 0.6) : lerp(peak, 1, (k - 0.6) / 0.4));

export const BOT_Y = 72;
const BOT_TEXT_X = 236;

export function seedPose(t: number, kind: SeedKind): SeedPose {
  const p = seedPlan(kind);
  if (kind === "still") {
    const a = seg(t, 0, p.end);
    return { seedY: BOT_Y, seedScale: 0, seedAlpha: 0, halves: 1, flash: 0, bot: 1, botAlpha: a, botX: SEED_W / 2, touch: 0, text: 0, grown: true };
  }
  // The fall, one bounce, and the squash before the crack.
  const fall = seg(t, 0, p.land);
  const bounce = seg(t, p.land, p.land + (p.crack - p.land) * 0.5);
  let seedY = lerp(-20, BOT_Y, fall * fall);
  if (t > p.land) seedY = BOT_Y - 14 * Math.sin(Math.PI * bounce);
  const squash = seg(t, p.land + (p.crack - p.land) * 0.5, p.crack);
  const seedScale = t >= p.crack ? 0 : 1 + 0.08 * squash;
  const seedAlpha = t >= p.crack ? 0 : seg(t, 0, Math.min(0.1, p.land));
  const halves = t < p.crack ? 0 : seg(t, p.crack, p.crack + 0.25);
  const flashK = seg(t, p.crack - 0.02, p.crack + 0.3);
  const flash = flashK <= 0 || flashK >= 1 ? 0 : flashK < 0.25 ? flashK / 0.25 : 1 - (flashK - 0.25) / 0.75;
  const popK = seg(t, p.pop, p.pop + 0.3);
  const bot = t < p.pop ? 0 : overshoot(popK);
  const aside = kind === "back" ? 0 : seg(t, p.grow, p.grow + 0.35);
  return {
    seedY, seedScale, seedAlpha, halves, flash,
    bot, botAlpha: t < p.pop ? 0 : clamp(popK * 3),
    botX: lerp(SEED_W / 2, BOT_TEXT_X, aside * aside * (3 - 2 * aside)),
    touch: seg(t, p.touch, p.touch + 0.35),
    text: seg(t, p.text, p.text + 0.18),
    grown: t >= p.grow,
  };
}

export interface SeedLook {
  skin: Skin;
  /** The stage he had at the last launch (grew), or has now. */
  stage: number;
  /** The stage reached now (grew: shown at the touch). */
  next: number;
  appearance: Appearance | null;
  outfit: Outfit;
  layers: Outfit[];
  lite: boolean;
}

/**
 * Runs the intro. `onGrow` fires when the island should grow to full size,
 * `onComplete` once at the end (or at once on a skip).
 */
export class SeedIntro {
  readonly kind: SeedKind;
  readonly plan: SeedPlan;
  private engine: BotEngine | null = new BotEngine();
  private startMs = 0;
  private grownFired = false;
  private fired = false;
  private hold = false;
  private touched = false;
  private lastMs = 0;
  onGrow: (() => void) | null = null;
  onComplete: (() => void) | null = null;

  private look: SeedLook;
  private lines: { title: string; sub: string };

  constructor(kind: SeedKind, look: SeedLook, lines: { title: string; sub: string }) {
    this.kind = kind;
    this.look = look;
    this.lines = lines;
    this.plan = seedPlan(kind);
    const e = this.engine!;
    e.setSkin(look.skin, false);
    e.setStage(kind === "first" ? 0 : look.stage, false);
    e.appearance = look.appearance;
    e.setOutfit(look.outfit, false);
    e.layers = look.layers;
    e.lite = look.lite;
  }

  start(now = performance.now()) {
    this.startMs = now;
    this.lastMs = now;
    if (this.kind !== "still") Sound.play("greeting");
  }

  get elapsed(): number {
    return (performance.now() - this.startMs) / 1000;
  }

  get done(): boolean {
    return this.fired;
  }

  /** The island is full size from now (and stays so). */
  get grown(): boolean {
    return this.kind === "still" || this.grownFired;
  }

  /** The pointer is on the island: the end waits for it to leave (or a click). */
  hover() {
    this.hold = true;
  }

  /** The pointer left. */
  interrupt() {
    this.hold = false;
  }

  /** A click: straight to the end. */
  skip() {
    if (!this.grownFired) {
      this.grownFired = true;
      this.onGrow?.();
    }
    this.finish();
  }

  /** The greeting view went away: let its sound fade. */
  leave() {
    Sound.fadeOut("greeting", 0.2);
    this.finish();
  }

  private finish() {
    if (this.fired) return;
    this.fired = true;
    // Made for one launch: nothing of it is kept.
    this.engine = null;
    this.onComplete?.();
  }

  /** Advances and draws one frame; false once it is over. */
  draw(x: CanvasRenderingContext2D, now = performance.now()): boolean {
    if (this.fired) return false;
    const t = (now - this.startMs) / 1000;
    const dt = Math.min(0.05, Math.max(0, (now - this.lastMs) / 1000));
    this.lastMs = now;
    const pose = seedPose(t, this.kind);
    if (pose.grown && !this.grownFired) {
      this.grownFired = true;
      this.onGrow?.();
    }
    const e = this.engine!;
    if (this.kind === "grew" && pose.touch > 0 && !this.touched) {
      this.touched = true;
      e.setStage(this.look.next, true);
    }
    e.update(dt);

    x.clearRect(0, 0, SEED_W, SEED_H);
    const color = SKIN_STYLES[this.look.skin].iris;
    if (pose.seedAlpha > 0) drawSeed(x, SEED_W / 2, pose.seedY, pose.seedScale, pose.seedAlpha, color);
    if (pose.halves > 0 && pose.halves < 1) drawHalves(x, SEED_W / 2, BOT_Y, pose.halves, color);
    if (pose.flash > 0 && !this.look.lite) {
      const g = x.createRadialGradient(SEED_W / 2, BOT_Y, 0, SEED_W / 2, BOT_Y, 70);
      g.addColorStop(0, `rgba(255,255,255,${0.75 * pose.flash})`);
      g.addColorStop(1, "rgba(255,255,255,0)");
      x.fillStyle = g;
      x.fillRect(SEED_W / 2 - 70, BOT_Y - 70, 140, 140);
    }
    if (pose.botAlpha > 0) {
      const size = 96;
      x.save();
      x.globalAlpha = pose.botAlpha;
      x.translate(pose.botX, BOT_Y);
      x.scale(pose.bot, pose.bot);
      x.translate(-size / 2, -size / 2);
      e.draw(x, size, size);
      x.restore();
      if (this.kind === "first" && pose.touch > 0) drawFirstTouch(x, this.look.skin, pose.botX, BOT_Y - 30, pose.touch);
    }
    if (pose.text > 0) {
      x.save();
      x.globalAlpha = pose.text;
      x.fillStyle = "#fff";
      x.font = "600 17px system-ui, 'Segoe UI', sans-serif";
      x.fillText(this.lines.title, BOT_TEXT_X + 64, BOT_Y - 4);
      x.fillStyle = "rgba(255,255,255,0.7)";
      x.font = "13px system-ui, 'Segoe UI', sans-serif";
      x.fillText(this.lines.sub, BOT_TEXT_X + 64, BOT_Y + 18);
      x.restore();
    }
    if (t >= this.plan.end && !this.hold) this.finish();
    return !this.fired;
  }
}

function drawSeed(x: CanvasRenderingContext2D, cx: number, cy: number, s: number, a: number, color: string) {
  x.save();
  x.globalAlpha = a;
  x.translate(cx, cy);
  x.scale(s, s);
  const glow = x.createRadialGradient(0, 0, 0, 0, 0, 22);
  glow.addColorStop(0, color);
  glow.addColorStop(1, "rgba(0,0,0,0)");
  x.globalAlpha = a * 0.55;
  x.fillStyle = glow;
  x.fillRect(-22, -22, 44, 44);
  x.globalAlpha = a;
  x.beginPath();
  x.ellipse(0, 0, 7, 9.5, 0, 0, Math.PI * 2);
  x.fillStyle = "#f6f1e7";
  x.fill();
  x.beginPath();
  x.ellipse(-2, -3, 2, 3, -0.4, 0, Math.PI * 2);
  x.fillStyle = "rgba(255,255,255,0.8)";
  x.fill();
  x.restore();
}

function drawHalves(x: CanvasRenderingContext2D, cx: number, cy: number, k: number, color: string) {
  for (const side of [-1, 1]) {
    x.save();
    x.globalAlpha = 1 - k;
    x.translate(cx + side * 40 * k, cy + 30 * k * k);
    x.rotate(side * k * (Math.PI / 3));
    x.beginPath();
    x.ellipse(0, 0, 7, 9.5, 0, side < 0 ? Math.PI / 2 : -Math.PI / 2, side < 0 ? Math.PI * 1.5 : Math.PI / 2);
    x.closePath();
    x.fillStyle = "#f6f1e7";
    x.fill();
    x.strokeStyle = color;
    x.lineWidth = 1;
    x.stroke();
    x.restore();
  }
}

/** Stage 0's first touch: one tiny sprout, spark or droplet on his head. */
function drawFirstTouch(x: CanvasRenderingContext2D, skin: Skin, cx: number, top: number, k: number) {
  const s = k < 0.6 ? (k / 0.6) * 1.25 : 1.25 - 0.25 * ((k - 0.6) / 0.4);
  x.save();
  x.translate(cx, top);
  x.scale(s, s);
  if (skin === "night") {
    x.strokeStyle = "#5FAE5A";
    x.lineWidth = 1.6;
    x.lineCap = "round";
    x.beginPath();
    x.moveTo(0, 0);
    x.quadraticCurveTo(1, -5, 0, -9);
    x.stroke();
    x.beginPath();
    x.ellipse(3.2, -9.5, 3.6, 1.8, -0.5, 0, Math.PI * 2);
    x.fillStyle = "#7CCB6B";
    x.fill();
  } else if (skin === "ember") {
    const g = x.createRadialGradient(0, -6, 0, 0, -6, 7);
    g.addColorStop(0, "#FFF2B0");
    g.addColorStop(0.5, "#FF9A3C");
    g.addColorStop(1, "rgba(255,90,30,0)");
    x.fillStyle = g;
    x.fillRect(-7, -13, 14, 14);
  } else {
    x.beginPath();
    x.moveTo(0, -12);
    x.quadraticCurveTo(5, -4, 0, -1);
    x.quadraticCurveTo(-5, -4, 0, -12);
    x.fillStyle = "#8CC8FF";
    x.fill();
  }
  x.restore();
}
