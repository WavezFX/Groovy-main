// The wardrobe's items (src/mochi/wardrobe.ts, FIXES_ROUND3 item 5): no Auto,
// no costume, and the old setting read only to move it into its slot.

import { test } from "node:test";
import assert from "node:assert/strict";
import { ACCESSORIES, DEFAULT_OUTFIT, OUTFIT_KEYS, isAccessory, parseOutfit } from "../src/mochi/wardrobe.ts";
import { DEFAULT_SETTINGS } from "../src/core/state.ts";

test("the items keep the Mac's raw values; Auto and the pumpkin are gone", () => {
  for (const id of ["partyHat", "beanie", "crown", "sunglasses", "roundGlasses", "bow", "scarf", "witchHat", "santaHat", "bunnyEars"]) {
    assert.ok(isAccessory(id), id);
    assert.ok(OUTFIT_KEYS[id], id);
  }
  assert.ok(!isAccessory("auto"));
  assert.ok(!isAccessory("pumpkin"));
});

test("a stored outfit is an accessory or nothing", () => {
  assert.equal(parseOutfit("witchHat"), "witchHat");
  for (const v of ["auto", "pumpkin", "topHat", "", null, 3, undefined]) assert.equal(parseOutfit(v), "none");
});

test("new installs wear nothing", () => {
  assert.equal(DEFAULT_OUTFIT, "none");
  assert.equal(DEFAULT_SETTINGS.mochiOutfit, "none");
});
