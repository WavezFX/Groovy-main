// Looks that grow (FIXES_ROUND3 item 6): Night → Grove, Ember → Blaze,
// Frost → Tide over five stages of time Groovy has run on the PC.

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  GROWTH_CORRECTION, GROWTH_RULE, MAX_STAGE, STAGE_AFTER, clockLines, hoursMinutes, localDay, lookGrowth, lookStarted, mergeGrowth, migrateGrownLook, parseGrowth,
  parseRule, SIX_MONTHS, progressLine, shownStage, stageAt, stageFor, untilNext,
} from "../src/mochi/growth.ts";
import { GROWN_STYLES, SKINS, SKIN_STYLES, parseSkin, styleAt } from "../src/mochi/skins.ts";

const DAY = 86400;
const NOW = 1_790_000_000;

test("the stage follows the time he has run: 1 day, 3 days, 1 week, 2 weeks, 1 month, then six months on the calendar", () => {
  assert.deepEqual([...STAGE_AFTER], [DAY, 3 * DAY, 7 * DAY, 14 * DAY, 30 * DAY]);
  assert.equal(MAX_STAGE, 6);
  assert.equal(stageFor(0), 0);
  assert.equal(stageFor(DAY - 1), 0);
  assert.equal(stageFor(DAY), 1);
  assert.equal(stageFor(3 * DAY + 5), 2);
  assert.equal(stageFor(7 * DAY), 3);
  assert.equal(stageFor(14 * DAY), 4);
  assert.equal(stageFor(400 * DAY), 5, "running time alone stops at 5");
  // Stage 6: at stage 5 and six months since he hatched.
  assert.equal(stageAt(31 * DAY, NOW - 181 * DAY, NOW), 5);
  assert.equal(stageAt(31 * DAY, NOW - SIX_MONTHS, NOW), 6);
  assert.equal(stageAt(20 * DAY, NOW - 400 * DAY, NOW), 4, "the calendar alone isn't enough");
});

test("a record reads back as saved, and never goes back", () => {
  const g = parseGrowth({}, NOW);
  assert.deepEqual(g, { activeSeconds: 0, daysWorn: 0, today: null, counted: 0, reached: 0, shown: null, seen: -1, hatchedAt: NOW, correction: 0 });
  // A stage once reached stays, even if the seconds read back lower.
  assert.equal(parseGrowth({ activeSeconds: 10, reached: 4 }, NOW).reached, 4);
  assert.equal(parseGrowth({ activeSeconds: 8 * DAY, reached: 1 }, NOW).reached, 3);
  // Without a hatching time, it counts back from his running time.
  assert.equal(parseGrowth({ activeSeconds: 8 * DAY }, NOW).hatchedAt, NOW - 8 * DAY);
  // The clock's own numbers (growth_clock.rs), checked.
  assert.deepEqual(parseGrowth({ daysWorn: 2, today: { day: "2026-10-30", seconds: 600 } }, NOW).today, { day: "2026-10-30", seconds: 600 });
  assert.equal(parseGrowth({ daysWorn: -1, today: { day: "yesterday", seconds: 5 } }, NOW).today, null);
  assert.equal(parseGrowth({ daysWorn: 2.5 }, NOW).daysWorn, 0);
});

test("round 3H item 3: the rule is one constant, running time by default; days worn counts a day each", () => {
  assert.equal(GROWTH_RULE, "running");
  assert.equal(parseRule(undefined), "running");
  assert.equal(parseRule("days"), "days");
  assert.equal(parseRule("weeks"), "running");
  const record = { activeSeconds: 6 * 3600, daysWorn: 3, hatchedAt: NOW - 3 * DAY, correction: 2 };
  assert.equal(parseGrowth(record, NOW, "running").reached, 0, "6 h of running isn't a day");
  assert.equal(parseGrowth(record, NOW, "days").reached, 2, "three days worn reach the 3-day stage");
  const settings = { groovyLook: "night", growths: { night: record }, growthRule: "days" };
  assert.equal(lookGrowth(settings).reached, 2);
  assert.equal(lookGrowth({ ...settings, growthRule: undefined }).reached, 0);
});

test("round 3H item 3: a look worn 2 h on three evenings shows 6 h counted; the other two stay at 0 and Not started", () => {
  // What growth_clock.rs leaves after three evenings of two hours.
  const settings = { groovyLook: "night", growths: { night: { activeSeconds: 6 * 3600, daysWorn: 3, today: { day: "2026-10-29", seconds: 7200 }, reached: 0, seen: 0, hatchedAt: NOW - 3 * DAY, correction: 2 } } };
  const night = clockLines(lookGrowth(settings, "night", NOW), "Night", "running", "2026-10-29");
  assert.equal(night.main, "Time with Night: 6 h of 24 h for Stage 1 · counts only while Groovy is running");
  assert.equal(night.today, "today: 2 h");
  assert.equal(clockLines(lookGrowth(settings, "night", NOW), "Night", "running", "2026-10-30").today, "today: 0 m", "a new day starts at 0");
  for (const look of ["ember", "frost"]) {
    assert.equal(lookStarted(settings, look), false);
    const g = lookGrowth(settings, look, NOW);
    assert.equal(g.activeSeconds, 0);
    assert.equal(g.reached, 0);
  }
  const days = clockLines(lookGrowth({ ...settings, growthRule: "days" }, "night", NOW), "Night", "days", "2026-10-29");
  assert.equal(days.main, "Days with Night: 3 of 7 for Stage 3 · a day counts once he wears it 30 minutes");
  assert.equal(days.today, "today: 2 h (the day counts at 30 m)");
  // Stage 6 is the calendar's, and then he is fully grown.
  assert.equal(clockLines(parseGrowth({ activeSeconds: 31 * DAY, hatchedAt: NOW - 40 * DAY }, NOW), "Night", "running").main, "Stage 6 comes six months after Night hatched.");
  assert.equal(clockLines(parseGrowth({ activeSeconds: 31 * DAY, hatchedAt: NOW - 200 * DAY }, NOW), "Night", "running").main, "Night is fully grown.");
});

test("hours and minutes, and the local day", () => {
  assert.equal(hoursMinutes(0), "0 m");
  assert.equal(hoursMinutes(25 * 60 + 59), "25 m");
  assert.equal(hoursMinutes(2 * 3600 + 10 * 60), "2 h 10 m");
  assert.equal(hoursMinutes(14 * 3600), "14 h");
  assert.equal(localDay(new Date(2026, 0, 5, 23, 59)), "2026-01-05");
});

test("the furthest of two copies wins, as Rust keeps them, and a corrected copy wins the stage", () => {
  const island = { activeSeconds: 8 * DAY, reached: 3, shown: 1, seen: 3, hatchedAt: NOW - 20 * DAY };
  const stale = { activeSeconds: 2 * DAY, reached: 1, shown: null, hatchedAt: NOW - 10 * DAY };
  const want = { activeSeconds: 8 * DAY, daysWorn: 0, today: null, counted: 8 * DAY, reached: 3, shown: null, seen: 3, hatchedAt: NOW - 20 * DAY, correction: 0 };
  assert.deepEqual(mergeGrowth(island, stale, NOW), want);
  assert.deepEqual(mergeGrowth(stale, island, NOW), want);
  assert.deepEqual(mergeGrowth("junk", null, NOW), { activeSeconds: 0, daysWorn: 0, today: null, counted: 0, reached: 0, shown: null, seen: -1, hatchedAt: NOW, correction: 0 });
  // Round 3E item 4: the granted 5 loses to the corrected record, whichever way round.
  const granted = { activeSeconds: 2 * DAY, reached: 5, seen: 5 };
  const corrected = { activeSeconds: 2 * DAY, reached: 1, seen: 1, correction: 1, hatchedAt: NOW - 2 * DAY };
  assert.equal(mergeGrowth(granted, corrected, NOW).reached, 1);
  assert.equal(mergeGrowth(corrected, granted, NOW).reached, 1);
});

test("no stage picking: he always shows the stage he has grown to, and an old choice is dropped", () => {
  const g = parseGrowth({ activeSeconds: 8 * DAY, shown: 1 });
  assert.equal(g.shown, null);
  assert.equal(shownStage(g), 3);
  assert.equal(shownStage(parseGrowth({ activeSeconds: 8 * DAY, reached: 4, shown: 0 })), 4);
});

test("the progress line is rough and says when he is fully grown", () => {
  const at = (activeSeconds, hatchedAt = NOW - activeSeconds) => parseGrowth({ activeSeconds, hatchedAt }, NOW);
  assert.equal(progressLine(at(4 * DAY), NOW), "Stage 2 of 6 · next in about 3 days");
  assert.equal(progressLine(at(DAY - 1800), NOW), "Stage 0 of 6 · next in less than an hour");
  assert.equal(progressLine(at(DAY - 3700), NOW), "Stage 0 of 6 · next in about an hour");
  assert.equal(progressLine(at(DAY - 5 * 3600), NOW), "Stage 0 of 6 · next in about 5 hours");
  // Stage 5: the rest of his six months, in months.
  assert.equal(progressLine(at(31 * DAY, NOW - 60 * DAY), NOW), "Stage 5 of 6 · next in about 4 months");
  assert.equal(progressLine(at(31 * DAY, NOW - 200 * DAY), NOW), "Stage 6 of 6 · fully grown");
  assert.equal(untilNext(at(31 * DAY, NOW - 200 * DAY), NOW), null);
});

test("Grove, Blaze and Tide become their base look at the stage he really reached", () => {
  for (const [old, base] of [["grove", "night"], ["blaze", "ember"], ["tide", "frost"]]) {
    const s = { groovyLook: old, growth: { activeSeconds: 120, reached: 0, shown: null } };
    assert.equal(migrateGrownLook(s, NOW), true);
    assert.equal(s.groovyLook, base);
    assert.deepEqual(s.growth, { activeSeconds: 120, daysWorn: 0, today: null, counted: 120, reached: 0, shown: null, seen: 0, hatchedAt: NOW - 120, correction: GROWTH_CORRECTION });
    assert.equal(migrateGrownLook(s, NOW), false, "once");
    assert.equal(parseSkin(old), base);
  }
  assert.deepEqual(SKINS, ["night", "ember", "frost"], "three looks to choose from");
});

test("round 3E item 4: a profile granted 'fully grown' goes back to its real stage, once", () => {
  // Migrated from Grove with 2 days of running time: stage 1, not 5.
  const migrated = { groovyLook: "night", growth: { activeSeconds: 2 * DAY, reached: 5, shown: null, seen: 5 } };
  assert.equal(migrateGrownLook(migrated, NOW), true);
  assert.equal(parseGrowth(migrated.growth, NOW).reached, 1);
  assert.equal(parseGrowth(migrated.growth, NOW).seen, 1);
  // It runs once: a later stage 5 earned for real is never taken away again.
  const later = { ...migrated.growth, activeSeconds: 31 * DAY, reached: 5 };
  migrated.growth = later;
  assert.equal(migrateGrownLook(migrated, NOW), false);
  assert.equal(parseGrowth(migrated.growth, NOW).reached, 5);
  // A real 35-day profile shows stage 5.
  const real = { groovyLook: "frost", growth: { activeSeconds: 35 * DAY, reached: 5 } };
  migrateGrownLook(real, NOW);
  assert.equal(parseGrowth(real.growth, NOW).reached, 5);
  // A fresh profile stays a seed.
  const fresh = { groovyLook: "ember", growth: {} };
  migrateGrownLook(fresh, NOW);
  assert.equal(parseGrowth(fresh.growth, NOW).reached, 0);
});

test("the colours move from the look to its grown one", () => {
  for (const skin of SKINS) {
    assert.deepEqual(styleAt(skin, 0).body, SKIN_STYLES[skin].body);
    assert.deepEqual(styleAt(skin, 5).body, GROWN_STYLES[skin].body);
    assert.equal(styleAt(skin, 3).stage, 3);
    assert.equal(styleAt(skin, 3), styleAt(skin, 3), "kept, not rebuilt every frame");
  }
});

// ── Drawing every stage ──────────────────────────────────────────────────────

/** A 2D context that checks every number it is given and counts the calls. */
function fakeContext() {
  const calls = { n: 0 };
  const gradient = { addColorStop() {} };
  const target = { canvas: { width: 120, height: 120 } };
  const ctx = new Proxy(target, {
    get(t, k) {
      if (k in t) return t[k];
      if (k === "createRadialGradient" || k === "createLinearGradient" || k === "createConicGradient") return () => gradient;
      if (k === "measureText") return () => ({ width: 0 });
      if (k === "getTransform") return () => ({ a: 1, b: 0, c: 0, d: 1, e: 0, f: 0 });
      if (k === "isPointInPath") return () => false;
      return (...args) => {
        calls.n++;
        for (const a of args) if (typeof a === "number") assert.ok(Number.isFinite(a), `${String(k)} got ${a}`);
      };
    },
    set(t, k, v) {
      t[k] = v;
      return true;
    },
  });
  return { ctx, calls };
}

test("every look draws every stage, lite or not, and each stage adds something", async () => {
  globalThis.Path2D ??= class { constructor() { return new Proxy(this, { get: (t, k) => (k in t ? t[k] : () => {}) }); } };
  const { BotEngine } = await import("../src/mochi/engine.ts");
  for (const skin of SKINS) {
    const counts = [];
    for (let stage = 0; stage <= MAX_STAGE; stage++) {
      for (const lite of [false, true]) {
        const e = new BotEngine();
        e.setSkin(skin, false);
        e.setStage(stage, false);
        e.lite = lite;
        const { ctx, calls } = fakeContext();
        e.draw(ctx, 110, 110);
        if (!lite) counts.push(calls.n);
      }
    }
    assert.ok(counts[MAX_STAGE] > counts[0], `${skin}: stage 5 draws more than stage 0`);
  }
});

test("round 5 item 5: stage 6 comes exactly at 182 days, and two copies keep the earliest hatching", () => {
  assert.equal(SIX_MONTHS, 182 * DAY);
  assert.equal(stageAt(31 * DAY, NOW - 182 * DAY + 1, NOW), 5, "a second short of 182 days");
  assert.equal(stageAt(31 * DAY, NOW - 182 * DAY, NOW), 6, "182 days to the second");
  const a = { activeSeconds: 31 * DAY, reached: 5, seen: 5, hatchedAt: NOW - 100 * DAY, correction: 2 };
  const b = { activeSeconds: 31 * DAY, reached: 5, seen: 5, hatchedAt: NOW - 190 * DAY, correction: 2 };
  const m = mergeGrowth(a, b, NOW);
  assert.equal(m.hatchedAt, NOW - 190 * DAY, "the earliest hatching is kept");
  assert.equal(m.reached, 6, "and with it, six months: stage 6");
  // A record from before hatchedAt counts back from its running time.
  assert.equal(parseGrowth({ activeSeconds: 31 * DAY, reached: 5 }, NOW).hatchedAt, NOW - 31 * DAY);
});
