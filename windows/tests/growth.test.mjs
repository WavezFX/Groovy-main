// Looks that grow (FIXES_ROUND3 item 6): Night → Grove, Ember → Blaze,
// Frost → Tide over five stages of time Groovy has run on the PC.

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  MAX_STAGE, SAVE_EVERY_SECONDS, STAGE_AFTER, TICK_SECONDS, grow, mergeGrowth, migrateGrownLook, parseGrowth,
  progressLine, showStage, shownStage, stageFor, startGrowthClock, untilNext,
} from "../src/mochi/growth.ts";
import { GROWN_STYLES, SKINS, SKIN_STYLES, parseSkin, styleAt } from "../src/mochi/skins.ts";

const DAY = 86400;

test("the stage follows the time he has run: 1 day, 3 days, 1 week, 2 weeks, 1 month", () => {
  assert.deepEqual([...STAGE_AFTER], [DAY, 3 * DAY, 7 * DAY, 14 * DAY, 30 * DAY]);
  assert.equal(MAX_STAGE, 5);
  assert.equal(stageFor(0), 0);
  assert.equal(stageFor(DAY - 1), 0);
  assert.equal(stageFor(DAY), 1);
  assert.equal(stageFor(3 * DAY + 5), 2);
  assert.equal(stageFor(7 * DAY), 3);
  assert.equal(stageFor(14 * DAY), 4);
  assert.equal(stageFor(400 * DAY), 5);
});

test("growing counts running time only, never more than two ticks at once, and never goes back", () => {
  let g = parseGrowth({});
  assert.deepEqual(g, { activeSeconds: 0, reached: 0, shown: null });
  g = grow(g, 60);
  assert.equal(g.activeSeconds, 60);
  // A night asleep (the timer was suspended) is one gap: at most two ticks.
  g = grow(g, 9 * 3600);
  assert.equal(g.activeSeconds, 60 + 2 * TICK_SECONDS);
  for (const bad of [-500, Number.NaN, -Infinity]) assert.equal(grow(g, bad).activeSeconds, g.activeSeconds);
  // A stage once reached stays, even if the seconds read back lower.
  assert.equal(parseGrowth({ activeSeconds: 10, reached: 4 }).reached, 4);
  assert.equal(parseGrowth({ activeSeconds: 8 * DAY, reached: 1 }).reached, 3);
});

test("the furthest of two copies wins, as Rust keeps them", () => {
  const island = { activeSeconds: 8 * DAY, reached: 3, shown: 1 };
  const stale = { activeSeconds: 2 * DAY, reached: 1, shown: null };
  assert.deepEqual(mergeGrowth(island, stale), { activeSeconds: 8 * DAY, reached: 3, shown: null });
  assert.deepEqual(mergeGrowth(stale, island), { activeSeconds: 8 * DAY, reached: 3, shown: 1 });
  assert.deepEqual(mergeGrowth("junk", null), { activeSeconds: 0, reached: 0, shown: null });
});

test("only a stage already reached can be shown", () => {
  const g = parseGrowth({ activeSeconds: 8 * DAY });
  assert.equal(shownStage(g), 3);
  assert.equal(shownStage(showStage(g, 1)), 1);
  assert.equal(shownStage(showStage(g, 0)), 0);
  assert.equal(shownStage(showStage(g, 5)), 3, "not reached yet");
  assert.equal(showStage(g, 3).shown, null, "the stage reached is the default, not a choice");
  assert.equal(shownStage(showStage(g, 2.5)), 3);
  assert.equal(parseGrowth({ activeSeconds: 8 * DAY, shown: 4 }).shown, null);
});

test("the progress line is rough and says when he is fully grown", () => {
  assert.equal(progressLine(parseGrowth({ activeSeconds: 4 * DAY })), "Stage 2 of 5 · next in about 3 days");
  assert.equal(progressLine(parseGrowth({ activeSeconds: DAY - 1800 })), "Stage 0 of 5 · next in less than an hour");
  assert.equal(progressLine(parseGrowth({ activeSeconds: DAY - 3700 })), "Stage 0 of 5 · next in about an hour");
  assert.equal(progressLine(parseGrowth({ activeSeconds: DAY - 5 * 3600 })), "Stage 0 of 5 · next in about 5 hours");
  assert.equal(progressLine(parseGrowth({ activeSeconds: 31 * DAY })), "Stage 5 of 5 · fully grown");
  assert.equal(untilNext(parseGrowth({ activeSeconds: 31 * DAY })), null);
  // A smaller stage shown still says how far he has really got.
  assert.equal(progressLine(showStage(parseGrowth({ activeSeconds: 4 * DAY }), 0)), "Stage 0 of 5 · next in about 3 days");
});

test("Grove, Blaze and Tide become their base look, fully grown", () => {
  for (const [old, base] of [["grove", "night"], ["blaze", "ember"], ["tide", "frost"]]) {
    const s = { groovyLook: old, growth: { activeSeconds: 120, reached: 0, shown: null } };
    assert.equal(migrateGrownLook(s), true);
    assert.equal(s.groovyLook, base);
    assert.deepEqual(s.growth, { activeSeconds: 120, reached: 5, shown: null });
    assert.equal(migrateGrownLook(s), false, "once");
    assert.equal(parseSkin(old), base);
  }
  const s = { groovyLook: "frost", growth: {} };
  assert.equal(migrateGrownLook(s), false);
  assert.deepEqual(SKINS, ["night", "ember", "frost"], "three looks to choose from");
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
