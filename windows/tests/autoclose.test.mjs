// Auto-close choices (src/core/autoclose.ts).

import { test } from "node:test";
import assert from "node:assert/strict";
import { AUTO_CLOSE_CHOICES, CLICK_AWAY, nearestChoice } from "../src/core/autoclose.ts";
import { DEFAULT_SETTINGS } from "../src/core/state.ts";

test("'When I leave' is the default and the first choice", () => {
  assert.equal(DEFAULT_SETTINGS.autoCloseInterval, 0);
  assert.deepEqual([...AUTO_CLOSE_CHOICES], [0, 1, 2, 3, 5, CLICK_AWAY]);
});

test("older delays show the nearest choice", () => {
  assert.equal(nearestChoice(0), 0);
  assert.equal(nearestChoice(-2), 0);
  assert.equal(nearestChoice(Number.NaN), 0);
  assert.equal(nearestChoice(15), 5);
  assert.equal(nearestChoice(120), 5);
  assert.equal(nearestChoice(2.2), 2);
  assert.equal(nearestChoice(4.5), 5);
  assert.equal(nearestChoice(0.5), 1);
});

test("Click away is kept as it is (round 4 item 2)", () => {
  assert.equal(nearestChoice(CLICK_AWAY), CLICK_AWAY);
});
