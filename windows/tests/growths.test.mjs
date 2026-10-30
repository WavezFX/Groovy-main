// Night, Ember and Frost each grow on their own (round 3E item 12).

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  GROWTH_CORRECTION, SIX_MONTHS, highestReached, lookGrowth, lookStarted, mergeGrowths, migrateToPerLook,
  setLookGrowth, startLook,
} from "../src/mochi/growth.ts";
import { chooseLook } from "../src/core/lookchoice.ts";

const DAY = 86400;
const NOW = 1_790_000_000;

/** Runs the look he wears for `days`, as the clock (growth_clock.rs) counts it. */
function run(settings, days, now) {
  const g = lookGrowth(settings, undefined, now);
  setLookGrowth(settings, settings.groovyLook, { ...g, activeSeconds: g.activeSeconds + days * DAY });
}

test("30 days as Night: Night at stage 5, Ember and Frost not started at stage 0", () => {
  const s = { groovyLook: "night", lookAt: 0, growths: {} };
  startLook(s, "night", NOW - 30 * DAY);
  run(s, 30, NOW);
  assert.equal(lookGrowth(s, "night", NOW).reached, 5);
  for (const look of ["ember", "frost"]) {
    assert.equal(lookStarted(s, look), false, look);
    assert.equal(lookGrowth(s, look, NOW).reached, 0, look);
  }
});

test("switching to Ember and running 2 days: Ember stage 2, Night unchanged", () => {
  const s = { groovyLook: "night", lookAt: 0, growths: {} };
  startLook(s, "night", NOW - 40 * DAY);
  run(s, 30, NOW);
  const night = JSON.stringify(lookGrowth(s, "night", NOW));
  chooseLook(s, "ember", NOW * 1000);
  assert.equal(lookStarted(s, "ember"), true, "choosing a look starts its clock");
  assert.equal(lookGrowth(s, "ember", NOW).hatchedAt, NOW, "from its own start");
  run(s, 3.1, NOW);
  assert.equal(lookGrowth(s, "ember", NOW).reached, 2);
  assert.equal(JSON.stringify(lookGrowth(s, "night", NOW)), night, "Night didn't move");
  assert.equal(highestReached(s, NOW), 5, "the wardrobe's keepsakes follow the furthest look");
});

test("the old single record moves to the chosen look once, and never again", () => {
  const s = { groovyLook: "frost", growth: { activeSeconds: 8 * DAY, reached: 3, seen: 3, hatchedAt: NOW - 20 * DAY, correction: 1 } };
  assert.equal(migrateToPerLook(s, NOW), true);
  assert.equal(lookGrowth(s, "frost", NOW).reached, 3);
  assert.equal(lookGrowth(s, "frost", NOW).correction, GROWTH_CORRECTION);
  assert.equal(lookStarted(s, "night"), false);
  assert.equal(lookStarted(s, "ember"), false);
  s.groovyLook = "night";
  assert.equal(migrateToPerLook(s, NOW), false, "once only");
  assert.equal(lookStarted(s, "night"), false);
  // A new install has nothing to move.
  assert.equal(migrateToPerLook({ groovyLook: "night", growth: {} }, NOW), false);
});

test("copies merge look by look, and none goes back", () => {
  const a = { night: { activeSeconds: 31 * DAY, reached: 5, seen: 5, hatchedAt: NOW - 40 * DAY, correction: 2 } };
  const b = { night: { activeSeconds: 2 * DAY, reached: 1, seen: 1, hatchedAt: NOW - 40 * DAY, correction: 2 }, ember: { activeSeconds: 4 * DAY, reached: 2, seen: 2, hatchedAt: NOW - 5 * DAY, correction: 2 } };
  const m = mergeGrowths(a, b, NOW);
  assert.equal(m.night.reached, 5);
  assert.equal(m.ember.reached, 2);
  assert.equal(m.frost, undefined);
});

test("the six months for stage 6 count from each look's own start", () => {
  const s = { groovyLook: "night", lookAt: 0, growths: {} };
  startLook(s, "night", NOW - SIX_MONTHS - DAY);
  run(s, 31, NOW);
  assert.equal(lookGrowth(s, "night", NOW).reached, 6, "Night: 31 days of running, six months since it started");
  chooseLook(s, "ember", (NOW - 60 * DAY) * 1000);
  s.growths.ember.hatchedAt = NOW - 60 * DAY;
  run(s, 31, NOW);
  assert.equal(lookGrowth(s, "ember", NOW).reached, 5, "Ember: the same running, but only two months since it started");
});
