// Round 4F item 7: the callback counter CI uses to prove a quiet page.

import { test } from "node:test";
import assert from "node:assert/strict";
import { countCallbacks, untilNext } from "../src/core/activity.ts";

test("timers and frames run in the window are counted, by where they were set, and the originals come back", async () => {
  let frame = null;
  const w = {
    requestAnimationFrame: (cb) => { frame = cb; return 1; },
    setTimeout: (cb, ms) => globalThis.setTimeout(cb, ms),
    setInterval: (cb, ms) => globalThis.setInterval(cb, ms),
  };
  const before = w.setTimeout;
  const stop = countCallbacks(w);
  w.setTimeout(() => {}, 1);
  w.requestAnimationFrame(() => {});
  frame(0);
  const id = w.setInterval(() => {}, 1);
  await new Promise((r) => globalThis.setTimeout(r, 30));
  globalThis.clearInterval(id);
  const c = stop();
  assert.equal(c.timeouts, 1);
  assert.equal(c.frames, 1);
  assert.ok(c.intervals >= 1);
  assert.ok(c.sources.length >= 1 && c.sources[0].at.includes("activity.test"));
  assert.equal(w.setTimeout, before);
});

test("untilNext waits for the earliest moment, at least the minimum", () => {
  assert.equal(untilNext([null, undefined], 0), null);
  assert.equal(untilNext([5_000, 3_000], 0), 3_000);
  assert.equal(untilNext([10], 0), 1_000);
});
