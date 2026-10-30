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

test("the sea and the heat glow reach past his body at any turn (round 3E item 9)", async () => {
  const { SEA_L, SEA_R, SEA_FLOOR, drawHeat } = await import("../src/mochi/finish.ts");
  const { BOARD } = await import("../src/mochi/art/draw.ts");
  // The board slides by up to 0.8 of his half-width each way as he turns.
  const slide = BOARD.hw * 0.8;
  assert.ok(SEA_L <= BOARD.cx - BOARD.hw - slide - 10, "the left edge");
  assert.ok(SEA_R >= BOARD.cx + BOARD.hw + slide + 10, "the right edge");
  assert.ok(SEA_FLOOR >= BOARD.cy + BOARD.hh + 20, "under his body");
  const rects = [];
  const gradient = { addColorStop() {} };
  const ctx = new Proxy({ fillRect: (...a) => rects.push(a) }, { get: (t, k) => (k in t ? t[k] : k === "createRadialGradient" ? () => gradient : () => {}), set: () => true });
  drawHeat(ctx, 4);
  const [x, y, w, h] = rects[0];
  assert.ok(x <= SEA_L && x + w >= SEA_R && y <= BOARD.cy - BOARD.hh && y + h >= SEA_FLOOR, "the glow's rectangle covers the same reach");
});
