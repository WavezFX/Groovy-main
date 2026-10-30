// Preview the stage looks and locked items for 5 seconds (round 3E item 11).

import { test } from "node:test";
import assert from "node:assert/strict";
import { stageStrip, untilStage, parseGrowth } from "../src/mochi/growth.ts";
import { PREVIEW_MS, Previewer } from "../src/settings/preview.ts";

const DAY = 86400;
const NOW = 1_790_000_000;

test("a profile at stage 2 has stages 1 and 2 unlocked, 3 to 6 locked, each saying when", () => {
  const g = parseGrowth({ activeSeconds: 4 * DAY, reached: 2, seen: 2, hatchedAt: NOW - 5 * DAY, correction: 1 }, NOW);
  const strip = stageStrip(g, NOW);
  assert.deepEqual(strip.map((s) => s.unlocked), [true, true, false, false, false, false]);
  assert.equal(strip[2].left, 3 * DAY, "stage 3 after a week of running");
  assert.equal(strip[5].left, NOW - 5 * DAY + 182 * DAY - NOW, "stage 6 on the calendar");
  assert.equal(untilStage(g, 2, NOW), null);
});

test("a preview shows for 5 seconds, then he is back as he is; a new one starts the 5 seconds again", () => {
  const timers = [];
  const after = (fn, ms) => (timers.push({ fn, ms, live: true }), timers.length - 1);
  const cancel = (i) => (timers[i].live = false);
  let changes = 0;
  const p = new Previewer(() => changes++, after, cancel);
  const settings = { growth: { reached: 2 }, groovyLook: "night" };
  const before = JSON.stringify(settings);
  p.show({ look: "frost", stage: 6, locked: true });
  assert.equal(p.current.stage, 6);
  assert.equal(timers[0].ms, PREVIEW_MS);
  assert.equal(PREVIEW_MS, 5000);
  p.show({ item: "diamondCrown", locked: true });
  assert.equal(timers[0].live, false, "the first one's timer is cancelled");
  timers[1].fn();
  assert.equal(p.current, null, "reverted after 5 seconds");
  assert.equal(changes, 3);
  assert.equal(JSON.stringify(settings), before, "nothing is saved or changed");
  p.stop();
  assert.equal(changes, 3, "stopping when nothing shows changes nothing");
});
