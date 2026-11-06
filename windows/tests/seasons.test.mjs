// The seasonal wardrobe (src/mochi/seasons.ts, round 5 item 4).

import { test } from "node:test";
import assert from "node:assert/strict";
import { SEASON_ITEMS, earn, easterSunday, lockLabel, nextStart, parseEarned, seasonOn, seasonalLaunch } from "../src/mochi/seasons.ts";
import { parseWardrobe } from "../src/mochi/appearance.ts";

const day = (y, m, d) => new Date(y, m - 1, d, 12);
const ymd = (d) => [d.getFullYear(), d.getMonth() + 1, d.getDate()];

test("Easter Sunday, by the Gregorian formula", () => {
  assert.deepEqual(ymd(easterSunday(2026)), [2026, 4, 5]);
  assert.deepEqual(ymd(easterSunday(2027)), [2027, 3, 28]);
  assert.deepEqual(ymd(easterSunday(2028)), [2028, 4, 16]);
});

test("the season windows, by the local date", () => {
  const at = (y, m, d) => seasonOn(day(y, m, d))?.season ?? null;
  assert.equal(at(2026, 9, 30), null);
  assert.equal(at(2026, 10, 1), "halloween");
  assert.equal(at(2026, 10, 25), "halloween", "today is in the Halloween window");
  assert.equal(at(2026, 11, 7), "halloween");
  assert.equal(at(2026, 11, 8), null);
  assert.equal(at(2026, 12, 1), "winter");
  assert.equal(at(2027, 1, 6), "winter");
  assert.deepEqual(seasonOn(day(2027, 1, 6)), { season: "winter", year: 2026 }, "January belongs to the winter that began in December");
  assert.equal(at(2027, 1, 7), null);
  assert.equal(at(2027, 3, 21), "easter");
  assert.equal(at(2027, 4, 4), "easter");
  assert.equal(at(2027, 4, 5), null);
  assert.equal(at(2026, 6, 20), null);
  assert.equal(at(2026, 6, 21), "summer");
  assert.equal(at(2026, 8, 31), "summer");
  assert.equal(at(2026, 9, 1), null);
});

test("each season has two head and two neck items", () => {
  assert.deepEqual(SEASON_ITEMS.halloween, ["pumpkinCap", "batWingBand", "candyCornScarf", "littleGhostPal"]);
  assert.deepEqual(SEASON_ITEMS.summer, ["strawSunHat", "watermelonCap", "shellNecklace", "poolRing"]);
});

test("running in a season earns its four items for good", () => {
  const now = earn([], day(2026, 10, 25));
  assert.deepEqual(now, SEASON_ITEMS.halloween);
  // Later, out of season: still there.
  assert.deepEqual(earn(now, day(2027, 2, 1)), now);
  // Winter adds its own.
  assert.deepEqual(earn(now, day(2026, 12, 24)), [...SEASON_ITEMS.halloween, ...SEASON_ITEMS.winter]);
  assert.deepEqual(parseEarned(["pumpkinCap", "pumpkinCap", "witchHat", 3]), ["pumpkinCap"]);
});

test("not earned yet: it says when it comes back", () => {
  const now = day(2026, 10, 25);
  assert.deepEqual(ymd(nextStart("winter", now)), [2026, 12, 1]);
  assert.deepEqual(ymd(nextStart("easter", now)), [2027, 3, 21]);
  assert.deepEqual(ymd(nextStart("summer", now)), [2027, 6, 21]);
  assert.match(lockLabel("bobbleBeanie", 0, [], now), /^Comes back in December$/);
  assert.equal(lockLabel("pumpkinCap", 0, ["pumpkinCap"], now), null);
  assert.equal(lockLabel("diamondCrown", 3, [], now), "Stage 6");
});

test("at launch the season is earned, and he dresses for it once", () => {
  const settings = { seasonalEarned: [], wardrobe: { appearance: { layers: ["bucketHat", "shades"] } } };
  assert.equal(seasonalLaunch(settings, day(2026, 10, 25)), true);
  assert.deepEqual(settings.seasonalEarned, SEASON_ITEMS.halloween);
  const w = parseWardrobe(settings.wardrobe);
  assert.deepEqual(w.appearance.layers, ["pumpkinCap", "shades"], "a seasonal hat in the head slot, the rest kept");
  assert.equal(w.dressedFor, "halloween:2026");
  // He changes it back: the next launch in the same season leaves it alone.
  settings.wardrobe = { ...w, appearance: { ...w.appearance, layers: ["acornCap"] } };
  assert.equal(seasonalLaunch(settings, day(2026, 10, 30)), false);
  assert.deepEqual(parseWardrobe(settings.wardrobe).appearance.layers, ["acornCap"]);
  // Turned off: earned, never dressed.
  const off = { seasonalEarned: [], wardrobe: { autoSeason: false, appearance: { layers: [] } } };
  seasonalLaunch(off, day(2026, 12, 3));
  assert.deepEqual(off.seasonalEarned, SEASON_ITEMS.winter);
  assert.deepEqual(parseWardrobe(off.wardrobe).appearance.layers, []);
});
