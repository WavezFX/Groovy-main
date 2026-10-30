// Round 3E item 5 (round 5 item 2): stage 6, the final forms.

import { test } from "node:test";
import assert from "node:assert/strict";
import { PASS_SECONDS, creatureAt, creatureFor, passStart } from "../src/mochi/stage6.ts";
import { SKINS, artFor, styleAt } from "../src/mochi/skins.ts";

test("the Diamond sea's swimmer: fish 70 %, shark 20 %, submarine 10 %, one at a time, every 2–4 minutes", () => {
  const n = 4000;
  const count = { fish: 0, shark: 0, submarine: 0 };
  for (let i = 0; i < n; i++) count[creatureFor(i)]++;
  assert.ok(Math.abs(count.fish / n - 0.7) < 0.03, JSON.stringify(count));
  assert.ok(Math.abs(count.shark / n - 0.2) < 0.03, JSON.stringify(count));
  assert.ok(Math.abs(count.submarine / n - 0.1) < 0.03, JSON.stringify(count));
  for (let i = 1; i < 50; i++) {
    const gap = passStart(i) - passStart(i - 1);
    assert.ok(gap >= 120 && gap <= 240, `gap ${gap}`);
  }
  // Nothing between passes; during one, a single creature moving across.
  const t0 = passStart(3);
  assert.equal(creatureAt(t0 - 1), null);
  assert.equal(creatureAt(t0 + PASS_SECONDS + 1), null);
  const mid = creatureAt(t0 + PASS_SECONDS / 2);
  assert.ok(mid && mid.progress > 0.4 && mid.progress < 0.6);
  assert.equal(mid.kind, creatureFor(3));
});

test("stage 6 gives every look its final form and gem eyes; below it, none", () => {
  for (const look of SKINS) {
    const six = styleAt(look, 6);
    assert.equal(six.stage, 6);
    assert.equal(six.gem, look);
    assert.ok(artFor(six), `${look} has stage 6 art`);
    assert.equal(styleAt(look, 5).gem, undefined);
    assert.equal(styleAt(look, 9).stage, 6, "never past 6");
  }
  // Grove's eyes are emeralds, Tide's diamonds, Blaze's gold.
  assert.equal(styleAt("night", 6).iris, "rgb(15,169,88)");
  assert.equal(styleAt("frost", 6).irisCore, "rgb(207,243,255)");
  assert.equal(styleAt("ember", 6).iris, "rgb(224,161,0)");
  // The board's three golden swimmers are not drawn standing still in his sea.
  const body = artFor(styleAt("frost", 6)).body ?? [];
  assert.ok(!body.some((s) => (s.f ?? s.s ?? "").toUpperCase() === "#E0B84A"));
});
