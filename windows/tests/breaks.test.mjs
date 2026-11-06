// Break reminders: each timer on its own, Done and snooze.

import { test } from "node:test";
import assert from "node:assert/strict";
import { State } from "../src/core/state.ts";
import { BreakTimer } from "../src/island/breaks.ts";

const MIN = 60_000;

test("a break comes due after its interval, Done restarts it, snooze asks in 10 minutes", () => {
  State.settings = { ...State.settings, breakWater: 45, breakStretch: 0, breakEyes: 0 };
  const timer = new BreakTimer();
  assert.equal(timer.check(0), null); // starts counting
  assert.equal(timer.check(44 * MIN), null);
  assert.equal(timer.check(45 * MIN), "water");
  timer.snooze("water", 45 * MIN);
  assert.equal(timer.check(50 * MIN), null);
  assert.equal(timer.check(55 * MIN), "water");
  timer.done("water", 55 * MIN);
  assert.equal(timer.check(99 * MIN), null);
  assert.equal(timer.check(100 * MIN), "water");
});

test("turned off, a break never comes; shortened, it comes sooner", () => {
  State.settings = { ...State.settings, breakWater: 0, breakStretch: 60, breakEyes: 0 };
  const timer = new BreakTimer();
  timer.check(0);
  assert.equal(timer.check(10 * MIN), null);
  State.settings = { ...State.settings, breakStretch: 30 };
  assert.equal(timer.check(10 * MIN), null); // restarted at the new interval
  assert.equal(timer.check(39 * MIN), null);
  assert.equal(timer.check(40 * MIN), "stretch");
});
