// How far Groovy's look has grown (FIXES_ROUND3 item 6): Night → Grove,
// Ember → Blaze, Frost → Tide over six stages, as Groovy runs on the PC.
// Stages 1–5 count the time he runs; stage 6 counts the calendar: six months
// since he hatched, once he is at stage 5 (round 5 item 2).
//
// The clock counts the time the island is actually running (a sleeping PC
// suspends the timer, and a long gap counts as one tick at most), is saved in
// settings (`growth`), is shared by the three looks, and never goes back:
// Rust keeps the furthest of two copies (settings::merge_growth).

import { GROWN_FROM, parseSkin, type Skin } from "./skins";
import { t } from "../i18n/i18n";

const DAY = 24 * 60 * 60;

/** Active seconds to reach stages 1–5: 1 day, 3 days, 1 week, 2 weeks, 1 month. Tune here. */
export const STAGE_AFTER = [DAY, 3 * DAY, 7 * DAY, 14 * DAY, 30 * DAY] as const;
/** Stage 6: this long on the calendar since he hatched, at stage 5. */
export const SIX_MONTHS = 182 * DAY;
export const MAX_STAGE = STAGE_AFTER.length + 1;
/**
 * The one-time correction of round 3E item 4: before it, the old Grove,
 * Blaze and Tide looks were granted "fully grown". A record below this
 * version has its stage worked out again from what really happened.
 */
export const GROWTH_CORRECTION = 1;

const nowSeconds = () => Math.floor(Date.now() / 1000);

/** How often the island counts, and how often it saves. */
export const TICK_SECONDS = 60;
export const SAVE_EVERY_SECONDS = 5 * 60;

export interface Growth {
  activeSeconds: number;
  /** The highest stage ever reached (0–5). */
  reached: number;
  /**
   * Always null: Groovy always shows the stage he has grown to (round 3B
   * item 5). Kept in the record so files that had a chosen stage read back
   * the same shape; whatever was saved there is dropped.
   */
  shown: null;
  /** The stage the last launch intro greeted him at; -1 before the first intro (mochi/seed.ts). */
  seen: number;
  /** When he hatched (unix seconds): the Seed intro sets it; an older record counts back from its active time. */
  hatchedAt: number;
  /** Which one-time correction the record has had (GROWTH_CORRECTION); 0 none. */
  correction: number;
}

const stageOf = (v: unknown): number | null =>
  typeof v === "number" && Number.isInteger(v) && v >= 0 && v <= MAX_STAGE ? v : null;

/** The saved record, checked: anything unreadable counts as nothing. */
export function parseGrowth(v: unknown, now: number = nowSeconds()): Growth {
  const o = v && typeof v === "object" ? (v as Record<string, unknown>) : {};
  const secs = typeof o.activeSeconds === "number" && Number.isFinite(o.activeSeconds) && o.activeSeconds > 0 ? o.activeSeconds : 0;
  const hatched = typeof o.hatchedAt === "number" && Number.isFinite(o.hatchedAt) && o.hatchedAt > 0 && o.hatchedAt <= now ? Math.floor(o.hatchedAt) : Math.floor(now - secs);
  const correction = typeof o.correction === "number" && Number.isInteger(o.correction) && o.correction >= 0 ? o.correction : 0;
  const reached = Math.max(stageOf(o.reached) ?? 0, stageAt(secs, hatched, now));
  const seen = stageOf(o.seen);
  return { activeSeconds: secs, reached, shown: null, seen: seen != null ? Math.min(seen, reached) : -1, hatchedAt: hatched, correction };
}

/** The stage that many active seconds reach (1–5; stage 6 is the calendar's, stageAt). */
export function stageFor(activeSeconds: number): number {
  let stage = 0;
  for (const after of STAGE_AFTER) if (activeSeconds >= after) stage++;
  return stage;
}

/** The stage he has really grown to: his running time, then six months since he hatched. */
export function stageAt(activeSeconds: number, hatchedAt: number, now: number = nowSeconds()): number {
  const stage = stageFor(activeSeconds);
  return stage >= STAGE_AFTER.length && now - hatchedAt >= SIX_MONTHS ? MAX_STAGE : stage;
}

/**
 * Round 3E item 4, once: the stage worked out again from the real record
 * (his running time and when he hatched), even if that is lower than what was
 * saved, which a granted "fully grown" made too high. Never runs twice.
 */
export function correctGrowth(g: Growth, now: number = nowSeconds()): { growth: Growth; changed: boolean } {
  if (g.correction >= GROWTH_CORRECTION) return { growth: g, changed: false };
  const reached = stageAt(g.activeSeconds, g.hatchedAt, now);
  return { growth: { ...g, reached, seen: Math.min(g.seen, reached), correction: GROWTH_CORRECTION }, changed: true };
}

/** Counts `seconds` more of running (never fewer, never more than two ticks at once). */
export function grow(g: Growth, seconds: number, now: number = nowSeconds()): Growth {
  const add = Number.isFinite(seconds) ? Math.min(Math.max(0, seconds), 2 * TICK_SECONDS) : 0;
  const activeSeconds = g.activeSeconds + add;
  return { ...g, activeSeconds, reached: Math.max(g.reached, stageAt(activeSeconds, g.hatchedAt, now)) };
}

/** The stage drawn: always the one he has grown to (no picking, round 3B item 5). */
export function shownStage(g: Growth): number {
  return g.reached;
}

/** How far he is from the stage he is at to the next, 0–1 (1 at the last stage). */
export function stageProgress(g: Growth, now: number = nowSeconds()): number {
  const stage = shownStage(g);
  if (stage >= MAX_STAGE) return 1;
  if (stage === STAGE_AFTER.length) return Math.max(0, Math.min(1, (now - g.hatchedAt) / SIX_MONTHS));
  const from = stage === 0 ? 0 : STAGE_AFTER[stage - 1];
  const to = STAGE_AFTER[stage];
  return Math.max(0, Math.min(1, (g.activeSeconds - from) / (to - from)));
}

/** Seconds left to the next stage (running time, or the calendar for stage 6); null at the last one. */
export function untilNext(g: Growth, now: number = nowSeconds()): number | null {
  if (g.reached >= MAX_STAGE) return null;
  if (g.reached === STAGE_AFTER.length) return Math.max(0, g.hatchedAt + SIX_MONTHS - now);
  return Math.max(0, STAGE_AFTER[g.reached] - g.activeSeconds);
}

/** "Stage 2 of 6 · next in about 3 days": rough, never a countdown. */
export function progressLine(g: Growth, now: number = nowSeconds()): string {
  const stage = t("Stage {n} of {max}", { n: shownStage(g), max: MAX_STAGE });
  const left = untilNext(g, now);
  if (left == null) return `${stage} · ${t("fully grown")}`;
  let next: string;
  if (left < 3600) next = t("next in less than an hour");
  else if (left < 1.5 * 3600) next = t("next in about an hour");
  else if (left < 1.5 * DAY) next = t("next in about {n} hours", { n: Math.round(left / 3600) });
  else if (left < 45 * DAY) next = t("next in about {n} days", { n: Math.round(left / DAY) });
  else next = t("next in about {n} months", { n: Math.round(left / (30.4 * DAY)) });
  return `${stage} · ${next}`;
}

/**
 * Grove, Blaze and Tide are no longer looks of their own (round 3 item 6):
 * someone who had one gets its base look, at the stage he has really grown
 * to (round 3E item 4: no "fully grown" grant), and the one-time correction
 * puts right a record that was granted it. True when anything changed.
 */
export function migrateGrownLook(settings: { groovyLook: string; growth: unknown }, now: number = nowSeconds()): boolean {
  const base: Skin | undefined = GROWN_FROM[settings.groovyLook];
  const fixed = correctGrowth(parseGrowth(settings.growth, now), now);
  if (base) {
    settings.groovyLook = parseSkin(base);
    // Not a first meeting: the next intro is a plain hello at his stage.
    fixed.growth.seen = fixed.growth.reached;
  }
  if (!base && !fixed.changed) return false;
  settings.growth = fixed.growth;
  return true;
}

/**
 * Two records as one, as Rust keeps them (settings::merge_growth): the most
 * running time and the earliest hatching; the stage never goes back, except
 * that a record with a newer correction wins over one without it.
 */
export function mergeGrowth(older: unknown, newer: unknown, now: number = nowSeconds()): Growth {
  const a = parseGrowth(older, now);
  const b = parseGrowth(newer, now);
  const correction = Math.max(a.correction, b.correction);
  const activeSeconds = Math.max(a.activeSeconds, b.activeSeconds);
  const hatchedAt = Math.min(a.hatchedAt, b.hatchedAt);
  const fair = stageAt(activeSeconds, hatchedAt, now);
  const reached = a.correction === b.correction ? Math.max(a.reached, b.reached) : Math.max(fair, (a.correction > b.correction ? a : b).reached);
  return { activeSeconds, reached, shown: null, seen: Math.min(Math.max(a.seen, b.seen), reached), hatchedAt, correction };
}

/**
 * Counts the island's running time once a minute and saves it every five
 * minutes, or at once when a stage is reached. Returns a stop function.
 */
export function startGrowthClock(
  get: () => unknown,
  set: (g: Growth, grew: boolean) => void,
  save: (g: Growth) => void,
  now: () => number = () => performance.now(),
): () => void {
  let last = now();
  let unsaved = 0;
  const id = setInterval(() => {
    const t = now();
    const dt = (t - last) / 1000;
    last = t;
    const before = parseGrowth(get());
    const after = grow(before, dt);
    const grew = after.reached > before.reached;
    set(after, grew);
    unsaved += dt;
    if (grew || unsaved >= SAVE_EVERY_SECONDS) {
      unsaved = 0;
      save(after);
    }
  }, TICK_SECONDS * 1000);
  return () => clearInterval(id);
}

/** Seconds until stage `n` is reached (running time, or the calendar for stage 6); null once reached. */
export function untilStage(g: Growth, n: number, now: number = nowSeconds()): number | null {
  if (g.reached >= n) return null;
  if (n <= STAGE_AFTER.length) return Math.max(0, STAGE_AFTER[n - 1] - g.activeSeconds);
  const toFive = Math.max(0, STAGE_AFTER[STAGE_AFTER.length - 1] - g.activeSeconds);
  const calendar = g.hatchedAt > 0 ? Math.max(0, g.hatchedAt + SIX_MONTHS - now) : SIX_MONTHS;
  return Math.max(toFive, calendar);
}

/** "unlocks in about 3 days": rough, like progressLine. */
export function unlocksIn(left: number): string {
  if (left < 3600) return t("unlocks in under an hour");
  if (left < 1.5 * DAY) return t("unlocks in about {n} hours", { n: Math.max(1, Math.round(left / 3600)) });
  if (left < 45 * DAY) return t("unlocks in about {n} days", { n: Math.round(left / DAY) });
  return t("unlocks in about {n} months", { n: Math.round(left / (30.4 * DAY)) });
}

/** The six stage looks for the preview strip (round 3E item 11): reached, or when each comes. */
export function stageStrip(g: Growth, now: number = nowSeconds()): { stage: number; unlocked: boolean; left: number | null }[] {
  return Array.from({ length: MAX_STAGE }, (_, i) => {
    const stage = i + 1;
    const left = untilStage(g, stage, now);
    return { stage, unlocked: left === null, left };
  });
}
