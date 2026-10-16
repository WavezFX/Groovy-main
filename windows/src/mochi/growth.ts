// How far Groovy's look has grown (FIXES_ROUND3 item 6): Night → Grove,
// Ember → Blaze, Frost → Tide over five stages, as Groovy runs on the PC.
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
export const MAX_STAGE = STAGE_AFTER.length;

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
}

const stageOf = (v: unknown): number | null =>
  typeof v === "number" && Number.isInteger(v) && v >= 0 && v <= MAX_STAGE ? v : null;

/** The saved record, checked: anything unreadable counts as nothing. */
export function parseGrowth(v: unknown): Growth {
  const o = v && typeof v === "object" ? (v as Record<string, unknown>) : {};
  const secs = typeof o.activeSeconds === "number" && Number.isFinite(o.activeSeconds) && o.activeSeconds > 0 ? o.activeSeconds : 0;
  const reached = Math.max(stageOf(o.reached) ?? 0, stageFor(secs));
  const seen = stageOf(o.seen);
  return { activeSeconds: secs, reached, shown: null, seen: seen != null ? Math.min(seen, reached) : -1 };
}

/** The stage that many active seconds reach. */
export function stageFor(activeSeconds: number): number {
  let stage = 0;
  for (const after of STAGE_AFTER) if (activeSeconds >= after) stage++;
  return stage;
}

/** Counts `seconds` more of running (never fewer, never more than two ticks at once). */
export function grow(g: Growth, seconds: number): Growth {
  const add = Number.isFinite(seconds) ? Math.min(Math.max(0, seconds), 2 * TICK_SECONDS) : 0;
  const activeSeconds = g.activeSeconds + add;
  return { ...g, activeSeconds, reached: Math.max(g.reached, stageFor(activeSeconds)) };
}

/** The stage drawn: always the one he has grown to (no picking, round 3B item 5). */
export function shownStage(g: Growth): number {
  return g.reached;
}

/** Active seconds left to the next stage; null at the last one. */
export function untilNext(g: Growth): number | null {
  if (g.reached >= MAX_STAGE) return null;
  return Math.max(0, STAGE_AFTER[g.reached] - g.activeSeconds);
}

/** "Stage 2 of 5 · next in about 3 days": rough, never a countdown. */
export function progressLine(g: Growth): string {
  const stage = t("Stage {n} of {max}", { n: shownStage(g), max: MAX_STAGE });
  const left = untilNext(g);
  if (left == null) return `${stage} · ${t("fully grown")}`;
  let next: string;
  if (left < 3600) next = t("next in less than an hour");
  else if (left < 1.5 * 3600) next = t("next in about an hour");
  else if (left < 1.5 * DAY) next = t("next in about {n} hours", { n: Math.round(left / 3600) });
  else next = t("next in about {n} days", { n: Math.round(left / DAY) });
  return `${stage} · ${next}`;
}

/**
 * Grove, Blaze and Tide are no longer looks of their own (round 3 item 6):
 * someone who had one gets its base look, fully grown. True when it changed.
 */
export function migrateGrownLook(settings: { groovyLook: string; growth: unknown }): boolean {
  const base: Skin | undefined = GROWN_FROM[settings.groovyLook];
  if (!base) return false;
  const g = parseGrowth(settings.growth);
  settings.groovyLook = parseSkin(base);
  // Not a first meeting: the next intro is a plain hello at his full stage.
  settings.growth = { ...g, reached: MAX_STAGE, shown: null, seen: MAX_STAGE };
  return true;
}

/** The furthest of two records, as Rust keeps them (settings::merge_growth). */
export function mergeGrowth(older: unknown, newer: unknown): Growth {
  const a = parseGrowth(older);
  const b = parseGrowth(newer);
  const reached = Math.max(a.reached, b.reached);
  return {
    activeSeconds: Math.max(a.activeSeconds, b.activeSeconds),
    reached,
    shown: null,
    seen: Math.max(a.seen, b.seen),
  };
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
