// Looks that grow (FIXES_ROUND3 item 6): Night → Grove, Ember → Blaze,
// Frost → Tide over five stages of time Groovy has run on the PC.

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  MAX_STAGE, SAVE_EVERY_SECONDS, STAGE_AFTER, TICK_SECONDS, grow, mergeGrowth, migrateGrownLook, parseGrowth,
  SIX_MONTHS, progressLine, shownStage, stageAt, stageFor, startGrowthClock, untilNext,
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

test("growing counts running time only, never more than two ticks at once, and never goes back", () => {
  let g = parseGrowth({}, NOW);
  assert.deepEqual(g, { activeSeconds: 0, reached: 0, shown: null, seen: -1, hatchedAt: NOW, correction: 0 });
  g = grow(g, 60, NOW);
  assert.equal(g.activeSeconds, 60);
  // A night asleep (the timer was suspended) is one gap: at most two ticks.
  g = grow(g, 9 * 3600, NOW);
  assert.equal(g.activeSeconds, 60 + 2 * TICK_SECONDS);
  for (const bad of [-500, Number.NaN, -Infinity]) assert.equal(grow(g, bad, NOW).activeSeconds, g.activeSeconds);
  // A stage once reached stays, even if the seconds read back lower.
  assert.equal(parseGrowth({ activeSeconds: 10, reached: 4 }, NOW).reached, 4);
  assert.equal(parseGrowth({ activeSeconds: 8 * DAY, reached: 1 }, NOW).reached, 3);
  // Without a hatching time, it counts back from his running time.
  assert.equal(parseGrowth({ activeSeconds: 8 * DAY }, NOW).hatchedAt, NOW - 8 * DAY);
});

test("the furthest of two copies wins, as Rust keeps them, and a corrected copy wins the stage", () => {
  const island = { activeSeconds: 8 * DAY, reached: 3, shown: 1, seen: 3, hatchedAt: NOW - 20 * DAY };
  const stale = { activeSeconds: 2 * DAY, reached: 1, shown: null, hatchedAt: NOW - 10 * DAY };
  const want = { activeSeconds: 8 * DAY, reached: 3, shown: null, seen: 3, hatchedAt: NOW - 20 * DAY, correction: 0 };
  assert.deepEqual(mergeGrowth(island, stale, NOW), want);
  assert.deepEqual(mergeGrowth(stale, island, NOW), want);
  assert.deepEqual(mergeGrowth("junk", null, NOW), { activeSeconds: 0, reached: 0, shown: null, seen: -1, hatchedAt: NOW, correction: 0 });
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
    assert.deepEqual(s.growth, { activeSeconds: 120, reached: 0, shown: null, seen: 0, hatchedAt: NOW - 120, correction: 1 });
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

test("the clock saves every five minutes, and at once when a stage is reached", async () => {
  const realSet = globalThis.setInterval;
  const realClear = globalThis.clearInterval;
  let tick = null;
  globalThis.setInterval = (fn) => ((tick = fn), 1);
  globalThis.clearInterval = () => (tick = null);
  try {
    let now = 0;
    let stored = { activeSeconds: DAY - 90, reached: 0, shown: null };
    const saves = [];
    const grew = [];
    const stop = startGrowthClock(() => stored, (g, did) => { stored = g; grew.push(did); }, (g) => saves.push(g), () => now);
    now += 60_000; tick();
    assert.equal(saves.length, 0);
    now += 60_000; tick();
    assert.equal(stored.reached, 1);
    assert.deepEqual(grew, [false, true]);
    assert.equal(saves.length, 1, "a new stage is saved at once");
    for (let i = 0; i < SAVE_EVERY_SECONDS / 60; i++) { now += 60_000; tick(); }
    assert.equal(saves.length, 2);
    stop();
    assert.equal(tick, null);
  } finally {
    globalThis.setInterval = realSet;
    globalThis.clearInterval = realClear;
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
