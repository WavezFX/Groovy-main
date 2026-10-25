// The seasonal wardrobe (round 5 item 4): four seasons by the local date, each
// with two head and two neck items. Running Groovy during a season earns its
// four items for good (`seasonalEarned` in settings, merged as a union in
// Rust), and on the season's first run he wears one of its hats if the
// wardrobe's "Dress for the season" is on.

import type { Accessory, SeasonalItem } from "./wardrobe";
import { SEASONAL_ITEMS, STAGE_LOCK, available, isSeasonal } from "./wardrobe";
import { language, t } from "../i18n/i18n";
import { parseWardrobe, wear } from "./appearance";

export const SEASONS = ["halloween", "winter", "easter", "summer"] as const;
export type Season = (typeof SEASONS)[number];

/** Each season's four items: two for the head, two for the neck (wardrobe.ts order). */
export const SEASON_ITEMS: Record<Season, readonly SeasonalItem[]> = {
  halloween: SEASONAL_ITEMS.slice(0, 4),
  winter: SEASONAL_ITEMS.slice(4, 8),
  easter: SEASONAL_ITEMS.slice(8, 12),
  summer: SEASONAL_ITEMS.slice(12, 16),
};

export function seasonOf(item: SeasonalItem): Season {
  return SEASONS[Math.floor(SEASONAL_ITEMS.indexOf(item) / 4)];
}

/** Easter Sunday (Gregorian, the anonymous algorithm), as a local date. */
export function easterSunday(year: number): Date {
  const a = year % 19;
  const b = Math.floor(year / 100);
  const c = year % 100;
  const d = Math.floor(b / 4);
  const e = b % 4;
  const f = Math.floor((b + 8) / 25);
  const g = Math.floor((b - f + 1) / 3);
  const h = (19 * a + b - d - g + 15) % 30;
  const i = Math.floor(c / 4);
  const k = c % 4;
  const l = (32 + 2 * e + 2 * i - h - k) % 7;
  const m = Math.floor((a + 11 * h + 22 * l) / 451);
  const month = Math.floor((h + l - 7 * m + 114) / 31);
  const day = ((h + l - 7 * m + 114) % 31) + 1;
  return new Date(year, month - 1, day);
}

const DAY = 86_400_000;

/** A season's window around `year`: [first day, last day], local dates. */
function windowOf(season: Season, year: number): [Date, Date] {
  switch (season) {
    case "halloween": return [new Date(year, 9, 1), new Date(year, 10, 7)];
    case "winter": return [new Date(year, 11, 1), new Date(year + 1, 0, 6)];
    case "summer": return [new Date(year, 5, 21), new Date(year, 7, 31)];
    case "easter": {
      const e = easterSunday(year).getTime();
      return [new Date(e - 7 * DAY), new Date(e + 7 * DAY)];
    }
  }
}

function dayOf(d: Date): number {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}

/** The season `date` falls in, if any, and the year its window started. */
export function seasonOn(date: Date): { season: Season; year: number } | null {
  const day = dayOf(date);
  for (const season of SEASONS) {
    // Winter's window runs into January: it started the year before.
    for (const year of [date.getFullYear(), date.getFullYear() - 1]) {
      const [from, to] = windowOf(season, year);
      if (day >= from.getTime() && day <= to.getTime()) return { season, year };
    }
  }
  return null;
}

/** When a season next begins (today, if it is on now). */
export function nextStart(season: Season, now: Date): Date {
  const day = dayOf(now);
  for (const year of [now.getFullYear() - 1, now.getFullYear(), now.getFullYear() + 1]) {
    const [from, to] = windowOf(season, year);
    if (to.getTime() >= day) return from.getTime() > day ? from : new Date(day);
  }
  return windowOf(season, now.getFullYear() + 1)[0];
}

/** The earned items, cleaned: known seasonal ids only, once each. */
export function parseEarned(v: unknown): SeasonalItem[] {
  return Array.isArray(v) ? [...new Set(v.filter(isSeasonal))] : [];
}

/** The earned items after running on `now`: the season's four are added. */
export function earn(earned: unknown, now: Date): SeasonalItem[] {
  const have = parseEarned(earned);
  const on = seasonOn(now);
  if (!on) return have;
  return [...new Set([...have, ...SEASON_ITEMS[on.season]])];
}

/**
 * At launch: the season's items are earned and, on its first run with
 * "Dress for the season" on, he puts on one of its hats (once a season; the
 * person can change it at once). True when something changed, to save.
 */
export function seasonalLaunch(settings: { seasonalEarned: unknown; wardrobe: unknown }, now: Date): boolean {
  const before = parseEarned(settings.seasonalEarned);
  const after = earn(before, now);
  let changed = after.length !== before.length || !Array.isArray(settings.seasonalEarned) || settings.seasonalEarned.length !== before.length;
  settings.seasonalEarned = after;
  const on = seasonOn(now);
  const prefs = parseWardrobe(settings.wardrobe);
  const key = on ? `${on.season}:${on.year}` : "";
  if (on && prefs.autoSeason && prefs.dressedFor !== key) {
    const hat = SEASON_ITEMS[on.season][on.year % 2];
    settings.wardrobe = { ...prefs, dressedFor: key, appearance: { ...prefs.appearance, layers: wear(prefs.appearance.layers.filter((l) => l !== hat), hat).layers } };
    changed = true;
  }
  return changed;
}

/** Month names, one formatter per language (making one is slow: the wardrobe asks for 16). */
const months = new Map<string, Intl.DateTimeFormat>();
function monthName(d: Date): string {
  const lang = language();
  let f = months.get(lang);
  if (!f) months.set(lang, (f = new Intl.DateTimeFormat(lang, { month: "long" })));
  return f.format(d);
}

/** Why an item can't be worn yet ("Stage 3", "Comes back in December"), or null. */
export function lockLabel(item: Accessory, reached: number, earned: readonly string[], now = new Date()): string | null {
  if (available(item, reached, earned)) return null;
  if (isSeasonal(item)) return t("Comes back in {month}", { month: monthName(nextStart(seasonOf(item), now)) });
  return t("Stage {n}", { n: STAGE_LOCK[item] ?? 0 });
}
