// Choosing a look sticks, and the wardrobe draws each picture once
// (FIXES_ROUND3B item 4).

import { test, mock } from "node:test";
import assert from "node:assert/strict";
import { installFakeDom } from "./fakedom.mjs";
import { DebouncedSave, chooseLook, shownLook } from "../src/core/lookchoice.ts";

installFakeDom();

test("Night → Ember → Frost → Night twenty times quickly ends on Night, saved once", () => {
  mock.timers.enable({ apis: ["setTimeout"] });
  try {
    const settings = { groovyLook: "night", lookAt: 0 };
    const saved = [];
    const saver = new DebouncedSave(() => saved.push({ ...settings }), 300);
    for (let i = 0; i < 20; i++) {
      for (const look of ["ember", "frost", "night"]) {
        chooseLook(settings, look, 5000);
        saver.schedule();
        mock.timers.tick(40);
      }
    }
    assert.equal(saved.length, 0, "nothing saved while still clicking");
    mock.timers.tick(300);
    assert.equal(saved.length, 1);
    assert.equal(saved[0].groovyLook, "night");
    // Every choice is stamped later than the one before, even within one millisecond.
    assert.equal(saved[0].lookAt, 5000 + 59);
  } finally {
    mock.timers.reset();
  }
});

test("a try-on shows only while it lasts; the saved look otherwise", () => {
  assert.equal(shownLook("ember", "night"), "ember");
  assert.equal(shownLook(null, "night"), "night");
  assert.equal(shownLook(null, "grove"), "night");
});

test("the wardrobe's pictures are drawn once and reused", async () => {
  globalThis.Path2D ??= class { constructor() { return new Proxy(this, { get: (t, k) => (k in t ? t[k] : () => {}) }); } };
  const make = document.createElement.bind(document);
  let drawn = 0;
  document.createElement = (tag) => {
    const el = make(tag);
    if (tag === "canvas") el.getContext = () => { drawn++; return null; };
    return el;
  };
  const { itemSprite, lookSprite, spriteCount } = await import("../src/mochi/sprites.ts");
  const before = spriteCount();
  const a = lookSprite("night", 2, 40, 2);
  const b = lookSprite("night", 2, 40, 2);
  assert.equal(a, b);
  assert.notEqual(lookSprite("night", 3, 40, 2), a, "another stage is another picture");
  itemSprite("acornCap", 26, 2);
  itemSprite("acornCap", 26, 2);
  assert.equal(spriteCount(), before + 3);
  assert.equal(drawn, 3, "three pictures drawn, however often they were asked for");
  document.createElement = make;
});
