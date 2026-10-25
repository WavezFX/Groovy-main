// The wardrobe's items (src/mochi/wardrobe.ts, round 5 items 3 and 4, round
// 3E item 7): the 28 Wardrobe v2 items and the Diamond crown, the 16 seasonal
// ones, stage locks, and every Mochi outfit gone.

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  ACCESSORIES, DEFAULT_OUTFIT, FACE_ITEMS, HEAD_ITEMS, NECK_ITEMS, OUTFIT_KEYS, SEASONAL_ITEMS, STAGE_LOCK,
  available, isAccessory, parseOutfit,
} from "../src/mochi/wardrobe.ts";
import { itemArt } from "../src/mochi/outfits.ts";
import { DEFAULT_SETTINGS } from "../src/core/state.ts";

test("12 head, 8 face and 8 neck items, the Diamond crown, and 16 seasonal ones", () => {
  assert.equal(HEAD_ITEMS.length, 13);
  assert.equal(HEAD_ITEMS.at(-1), "diamondCrown");
  assert.equal(FACE_ITEMS.length, 8);
  assert.equal(NECK_ITEMS.length, 8);
  assert.equal(SEASONAL_ITEMS.length, 16);
  assert.equal(new Set(ACCESSORIES).size, 45);
  for (const id of ACCESSORIES) {
    assert.ok(OUTFIT_KEYS[id], `${id} has a name`);
    const art = itemArt(id);
    assert.ok(art && (art.front?.length || art.body?.length), `${id} is drawn`);
  }
});

test("no Mochi outfit is left", () => {
  for (const id of ["partyHat", "beanie", "crown", "sunglasses", "roundGlasses", "bow", "scarf", "witchHat", "santaHat", "bunnyEars", "pumpkin", "auto"]) {
    assert.ok(!isAccessory(id), id);
  }
  for (const name of Object.values(OUTFIT_KEYS)) assert.doesNotMatch(name, /^(Party hat|Beanie|Crown|Witch hat|Santa hat|Bunny ears|Bow|Sunglasses|Round glasses|Scarf)$/);
});

test("a stored outfit is an item, its nearest new one, or nothing", () => {
  assert.equal(parseOutfit("acornCap"), "acornCap");
  assert.equal(parseOutfit("beanie"), "bucketHat");
  assert.equal(parseOutfit("bow"), "bowTie");
  for (const v of ["auto", "pumpkin", "witchHat", "crown", "toString", "", null, 3, undefined]) assert.equal(parseOutfit(v), "none", String(v));
});

test("stage items wait for their stage and never lock again", () => {
  assert.deepEqual(STAGE_LOCK, { seedPendant: 1, flowerCrown: 2, miniTopHat: 3, cloudBuddy: 4, goldChain: 5, diamondCrown: 6 });
  assert.ok(!available("flowerCrown", 1, []));
  assert.ok(available("flowerCrown", 2, []));
  assert.ok(available("flowerCrown", 6, []));
  assert.ok(!available("diamondCrown", 5, []));
  assert.ok(available("diamondCrown", 6, []));
  assert.ok(available("leafCrown", 0, []), "the rest are there from the start");
  // Seasonal items only once earned.
  assert.ok(!available("pumpkinCap", 6, []));
  assert.ok(available("pumpkinCap", 0, ["pumpkinCap"]));
});

test("new installs wear nothing", () => {
  assert.equal(DEFAULT_OUTFIT, "none");
  assert.equal(DEFAULT_SETTINGS.mochiOutfit, "none");
  assert.deepEqual(DEFAULT_SETTINGS.seasonalEarned, []);
});

test("no Mochi outfit is left anywhere in the code (round 3E item 8)", async () => {
  const { readdirSync, readFileSync, statSync } = await import("node:fs");
  const { join } = await import("node:path");
  const files = [];
  const walk = (dir) => {
    for (const name of readdirSync(dir)) {
      const p = join(dir, name);
      if (statSync(p).isDirectory()) walk(p);
      else if (/\.(ts|css|html)$/.test(name)) files.push(p);
    }
  };
  walk("src");
  walk("src-tauri/src");
  const mochi = /\b(partyHat|witchHat|santaHat|bunnyEars|roundGlasses)\b|"(Party hat|Witch hat|Santa hat|Bunny ears|Beanie|Sunglasses|Round glasses)"/;
  const found = [];
  for (const f of files) {
    readFileSync(f, "utf8").split("\n").forEach((line, i) => {
      // The one place old ids are read, to move them to their new item.
      if (f.endsWith("wardrobe.ts") && /OLD_IDS|^\s+(beanie|bow|sunglasses|roundGlasses|scarf): "/.test(line)) return;
      if (mochi.test(line)) found.push(`${f}:${i + 1}: ${line.trim()}`);
    });
  }
  assert.deepEqual(found, []);
});
