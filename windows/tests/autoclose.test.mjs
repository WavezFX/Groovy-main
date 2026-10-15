// Auto-close choices (src/core/autoclose.ts).

import { test } from "node:test";
import assert from "node:assert/strict";
import { AUTO_CLOSE_CHOICES, nearestChoice } from "../src/core/autoclose.ts";
import { DEFAULT_SETTINGS } from "../src/core/state.ts";

test("'When I leave' is the default and the first choice", () => {
  assert.equal(DEFAULT_SETTINGS.autoCloseInterval, 0);
  assert.deepEqual([...AUTO_CLOSE_CHOICES], [0, 1, 3, 5, 10, 30]);
});

test("older delays show the nearest choice", () => {
  assert.equal(nearestChoice(0), 0);
  assert.equal(nearestChoice(-2), 0);
  assert.equal(nearestChoice(Number.NaN), 0);
  assert.equal(nearestChoice(15), 10);
  assert.equal(nearestChoice(120), 30);
  assert.equal(nearestChoice(4), 3);
  assert.equal(nearestChoice(0.5), 1);
});
