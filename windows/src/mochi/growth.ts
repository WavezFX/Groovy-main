// How far Groovy's look has grown (FIXES_ROUND3 item 6): Night → Grove,
// Ember → Blaze, Frost → Tide over six stages, as Groovy runs on the PC.
// Stages 1–5 count the time he runs; stage 6 counts the calendar: six months
// since he hatched, once he is at stage 5 (round 5 item 2).
//
// The clock is Rust's (growth_clock.rs, round 3H item 3): it counts while
// Groovy runs and the PC is awake, whatever the windows do (a long gap counts
// as two ticks at most), and never goes back: Rust keeps the furthest of two
// copies (settings::merge_growth). Each record holds the running time, today's
// time and the days the look was worn for 30 minutes; GROWTH_RULE (or the
// owner's choice, settings.growthRule) says which of them grows him.
//
// Each look grows on its own (round 3E item 12): `settings.growths` holds one
// record per look (Night, Ember, Frost), and only the look he wears grows. A
// look without a record is "Not started": stage 0, its clock and its six
// months start the first time it is worn. The single `growth` record of
// earlier versions moves, once, to the look he wears (migrateToPerLook).

import { GROWN_FROM, parseSkin, type Skin } from "./skins";
import { t } from "../i18n/i18n";

const DAY = 24 * 60 * 60;

/** Active seconds to reach stages 1–5: 1 day, 3 days, 1 week, 2 weeks, 1 month. Tune here. */
export const STAGE_AFTER = [DAY, 3 * DAY, 7 * DAY, 14 * DAY, 30 * DAY] as const;
/** Stage 6: this long on the calendar since he hatched, at stage 5. */
export const SIX_MONTHS = 182 * DAY;
export const MAX_STAGE = STAGE_AFTER.length + 1;
/**
 * The one-time corrections: 1, round 3E item 4 (the old Grove, Blaze and
 * Tide looks were granted "fully grown"); 2, round 3E item 12 (one record per
 * look). A record below this version has its stage worked out again from
 * what really happened, which may be lower, once.
 */
export const GROWTH_CORRECTION = 2;

const nowSeconds = () => Math.floor(Date.now() / 1000);

/**
 * What grows him (round 3H item 3, the owner's decision, asked on the Groovy
 * page): "running", the time Groovy runs (the default), or "days", calendar
 * days the look was worn for at least 30 minutes, each counting as a day of
 * the stage times above. Change the default here.
 */
export type GrowthRule = "running" | "days";
export const GROWTH_RULE: GrowthRule = "running";

export function parseRule(v: unknown): GrowthRule {
  return v === "days" || v === "running" ? v : GROWTH_RULE;
}

export interface Growth {
  /** Running time (seconds) while this look was worn. */
  activeSeconds: number;
  /** Days this look was worn for at least 30 minutes. */
  daysWorn: number;
  /** Today's running time with this look (the local day, "2026-10-30"). */
  today: { day: string; seconds: number } | null;
  /** The seconds that count for the stages, by the rule: activeSeconds, or daysWorn days. */
  counted: number;
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
export function parseGrowth(v: unknown, now: number = nowSeconds(), rule: GrowthRule = GROWTH_RULE): Growth {
  const o = v && typeof v === "object" ? (v as Record<string, unknown>) : {};
  const secs = typeof o.activeSeconds === "number" && Number.isFinite(o.activeSeconds) && o.activeSeconds > 0 ? o.activeSeconds : 0;
  const daysWorn = typeof o.daysWorn === "number" && Number.isInteger(o.daysWorn) && o.daysWorn > 0 ? o.daysWorn : 0;
  const td = o.today && typeof o.today === "object" ? (o.today as Record<string, unknown>) : null;
  const today = td && typeof td.day === "string" && /^\d{4}-\d{2}-\d{2}$/.test(td.day)
    ? { day: td.day, seconds: typeof td.seconds === "number" && Number.isFinite(td.seconds) && td.seconds > 0 ? td.seconds : 0 }
    : null;
  const counted = rule === "days" ? daysWorn * DAY : secs;
  const hatched = typeof o.hatchedAt === "number" && Number.isFinite(o.hatchedAt) && o.hatchedAt > 0 && o.hatchedAt <= now ? Math.floor(o.hatchedAt) : Math.floor(now - secs);
  const correction = typeof o.correction === "number" && Number.isInteger(o.correction) && o.correction >= 0 ? o.correction : 0;
  const reached = Math.max(stageOf(o.reached) ?? 0, stageAt(counted, hatched, now));
  const seen = stageOf(o.seen);
  return { activeSeconds: secs, daysWorn, today, counted, reached, shown: null, seen: seen != null ? Math.min(seen, reached) : -1, hatchedAt: hatched, correction };
}

/** The stage that many counted seconds reach (1–5; stage 6 is the calendar's, stageAt). */
export function stageFor(counted: number): number {
  let stage = 0;
  for (const after of STAGE_AFTER) if (counted >= after) stage++;
  return stage;
}

/** The stage he has really grown to: his counted time, then six months since he hatched. */
export function stageAt(counted: number, hatchedAt: number, now: number = nowSeconds()): number {
  const stage = stageFor(counted);
  return stage >= STAGE_AFTER.length && now - hatchedAt >= SIX_MONTHS ? MAX_STAGE : stage;
}

/**
 * Round 3E item 4, once: the stage worked out again from the real record
 * (his running time and when he hatched), even if that is lower than what was
 * saved, which a granted "fully grown" made too high. Never runs twice.
 */
export function correctGrowth(g: Growth, now: number = nowSeconds()): { growth: Growth; changed: boolean } {
  if (g.correction >= GROWTH_CORRECTION) return { growth: g, changed: false };
  const reached = stageAt(g.counted, g.hatchedAt, now);
  return { growth: { ...g, reached, seen: Math.min(g.seen, reached), correction: GROWTH_CORRECTION }, changed: true };
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
  return Math.max(0, Math.min(1, (g.counted - from) / (to - from)));
}

/** Seconds left to the next stage (running time, or the calendar for stage 6); null at the last one. */
export function untilNext(g: Growth, now: number = nowSeconds()): number | null {
  if (g.reached >= MAX_STAGE) return null;
  if (g.reached === STAGE_AFTER.length) return Math.max(0, g.hatchedAt + SIX_MONTHS - now);
  return Math.max(0, STAGE_AFTER[g.reached] - g.counted);
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
  const daysWorn = Math.max(a.daysWorn, b.daysWorn);
  const counted = Math.max(a.counted, b.counted);
  const today = !a.today || !b.today ? a.today ?? b.today
    : a.today.day === b.today.day ? { day: a.today.day, seconds: Math.max(a.today.seconds, b.today.seconds) }
    : a.today.day > b.today.day ? a.today : b.today;
  const hatchedAt = Math.min(a.hatchedAt, b.hatchedAt);
  const fair = stageAt(counted, hatchedAt, now);
  const reached = a.correction === b.correction ? Math.max(a.reached, b.reached) : Math.max(fair, (a.correction > b.correction ? a : b).reached);
  return { activeSeconds, daysWorn, today, counted, reached, shown: null, seen: Math.min(Math.max(a.seen, b.seen), reached), hatchedAt, correction };
}

/** Seconds until stage `n` is reached (running time, or the calendar for stage 6); null once reached. */
export function untilStage(g: Growth, n: number, now: number = nowSeconds()): number | null {
  if (g.reached >= n) return null;
  if (n <= STAGE_AFTER.length) return Math.max(0, STAGE_AFTER[n - 1] - g.counted);
  const toFive = Math.max(0, STAGE_AFTER[STAGE_AFTER.length - 1] - g.counted);
  const calendar = g.hatchedAt > 0 ? Math.max(0, g.hatchedAt + SIX_MONTHS - now) : SIX_MONTHS;
  return Math.max(toFive, calendar);
}

/** Today's date where the user is, as the clock writes it ("2026-10-30"). */
export function localDay(d: Date = new Date()): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

/** "14 h", "2 h 10 m", "25 m". */
export function hoursMinutes(seconds: number): string {
  const m = Math.floor(Math.max(0, seconds) / 60);
  const h = Math.floor(m / 60);
  if (h === 0) return t("{m} m", { m });
  if (m % 60 === 0) return t("{h} h", { h });
  return t("{h} h {m} m", { h, m: m % 60 });
}

/**
 * The Groovy page's clock (round 3H item 3), so it can be seen counting:
 * "Time with Night: 14 h of 24 h for Stage 1 · counts only while Groovy is
 * running", and "today: 2 h 10 m".
 */
export function clockLines(g: Growth, look: string, rule: GrowthRule, today: string = localDay()): { main: string; today: string } {
  const time = hoursMinutes(g.today?.day === today ? g.today.seconds : 0);
  const todayLine = rule === "days" ? t("today: {time} (the day counts at 30 m)", { time }) : t("today: {time}", { time });
  let main: string;
  if (g.reached >= MAX_STAGE) main = t("{look} is fully grown.", { look });
  else if (g.reached === STAGE_AFTER.length) main = t("Stage 6 comes six months after {look} hatched.", { look });
  else {
    const need = STAGE_AFTER[g.reached];
    const stage = g.reached + 1;
    main = rule === "days"
      ? t("Days with {look}: {n} of {of} for Stage {stage} · a day counts once he wears it 30 minutes", { look, n: Math.min(g.daysWorn, need / DAY), of: need / DAY, stage })
      : t("Time with {look}: {n} h of {of} h for Stage {stage} · counts only while Groovy is running", { look, n: Math.min(Math.floor(g.activeSeconds / 3600), need / 3600), of: need / 3600, stage });
  }
  return { main, today: todayLine };
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

// ── One record per look (round 3E item 12) ─────────────────────────────────

const LOOKS: readonly Skin[] = ["night", "ember", "frost"];

type WithGrowths = { growths?: unknown; groovyLook: string; growthRule?: unknown };

/** The stored records, by look; anything else is dropped. */
export function growthsOf(settings: { growths?: unknown }): Partial<Record<Skin, unknown>> {
  const o = settings.growths && typeof settings.growths === "object" ? (settings.growths as Record<string, unknown>) : {};
  const out: Partial<Record<Skin, unknown>> = {};
  for (const look of LOOKS) if (o[look] && typeof o[look] === "object") out[look] = o[look];
  return out;
}

/** Whether this look's clock has started (it has been worn). */
export function lookStarted(settings: { growths?: unknown }, look: Skin): boolean {
  return growthsOf(settings)[look] !== undefined;
}

/** A look's own growth (the look he wears, by default); a look not started is stage 0. */
export function lookGrowth(settings: WithGrowths, look: Skin = parseSkin(settings.groovyLook), now: number = nowSeconds()): Growth {
  const g = parseGrowth(growthsOf(settings)[look], now, parseRule(settings.growthRule));
  return lookStarted(settings, look) ? g : { ...g, correction: GROWTH_CORRECTION };
}

/** Puts a look's record back into the settings. */
export function setLookGrowth(settings: { growths?: unknown }, look: Skin, g: Growth) {
  settings.growths = { ...growthsOf(settings), [look]: g };
}

/** A new look's record: its clock and its six months start now. */
export function freshGrowth(now: number = nowSeconds(), seen = 0): Growth {
  return { activeSeconds: 0, daysWorn: 0, today: null, counted: 0, reached: 0, shown: null, seen, hatchedAt: now, correction: GROWTH_CORRECTION };
}

/** Starts a look's clock the first time it is chosen (not before). True when it started now. */
export function startLook(settings: { growths?: unknown }, look: Skin, now: number = nowSeconds()): boolean {
  if (lookStarted(settings, look)) return false;
  setLookGrowth(settings, look, freshGrowth(now));
  return true;
}

/** The highest stage any look has reached: the wardrobe's keepsakes unlock at it. */
export function highestReached(settings: { growths?: unknown; growthRule?: unknown }, now: number = nowSeconds()): number {
  const all = growthsOf(settings);
  return Math.max(0, ...LOOKS.filter((l) => all[l] !== undefined).map((l) => parseGrowth(all[l], now, parseRule(settings.growthRule)).reached));
}

/**
 * Once: the single record of earlier versions moves to the look he wears;
 * the other two are "Not started". Runs only while no look has a record of
 * its own, so never twice. True when it moved something (to save).
 */
export function migrateToPerLook(settings: { growth?: unknown; growths?: unknown; groovyLook: string }, now: number = nowSeconds()): boolean {
  if (Object.keys(growthsOf(settings)).length) return false;
  const raw = settings.growth && typeof settings.growth === "object" ? (settings.growth as Record<string, unknown>) : null;
  if (!raw || !Object.keys(raw).length) return false;
  const { growth } = correctGrowth(parseGrowth(raw, now), now);
  setLookGrowth(settings, parseSkin(settings.groovyLook), { ...growth, correction: GROWTH_CORRECTION });
  return true;
}

/** Two copies of every look's record as one (as Rust keeps them): each look on its own. */
export function mergeGrowths(older: unknown, newer: unknown, now: number = nowSeconds()): Partial<Record<Skin, Growth>> {
  const a = growthsOf({ growths: older });
  const b = growthsOf({ growths: newer });
  const out: Partial<Record<Skin, Growth>> = {};
  for (const look of LOOKS) {
    if (a[look] === undefined && b[look] === undefined) continue;
    out[look] = a[look] === undefined ? parseGrowth(b[look], now) : b[look] === undefined ? parseGrowth(a[look], now) : mergeGrowth(a[look], b[look], now);
  }
  return out;
}
