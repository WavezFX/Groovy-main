import { test } from "node:test";
import assert from "node:assert/strict";
import { noise, WATER_LEVEL, isTongue, isWaterPart } from "../src/mochi/finish.ts";
import { GROWTH_ART } from "../src/mochi/art/growth.ts";

test("the flicker's noise is smooth and stays in range", () => {
  let last = noise(0);
  for (let t = 0.01; t < 70; t += 0.01) {
    const n = noise(t);
    assert.ok(n >= 0 && n < 1);
    assert.ok(Math.abs(n - last) < 0.05, `no jumps at ${t}`);
    last = n;
  }
});

test("Tide's water rises with the stage", () => {
  for (let s = 2; s <= 5; s++) assert.ok(WATER_LEVEL[s] < WATER_LEVEL[s - 1], `stage ${s} is fuller`);
  assert.equal(WATER_LEVEL[0], null);
});

test("the board's flat flames and water are the parts drawn softly", () => {
  for (let s = 2; s <= 5; s++) assert.ok(GROWTH_ART.ember[s].front.some(isTongue), `Blaze ${s} has a tongue`);
  for (let s = 1; s <= 5; s++) assert.ok(GROWTH_ART.frost[s].body.some(isWaterPart), `Tide ${s} has water`);
  // Seaweed stays as the board draws it.
  assert.ok(!GROWTH_ART.frost[4].body.filter((sh) => sh.s === "#2FBF8F").some(isWaterPart));
});
