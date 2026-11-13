// Round 3M item 4: worn parts lag, swing and settle to exactly still (no CPU at rest).
import assert from "node:assert/strict";
import { test } from "node:test";

test("a turn and a hop swing the dangling parts, which settle to exactly 0 and stop the redraw loop", async () => {
  const { BotEngine } = await import("../src/mochi/engine.ts");
  const e = new BotEngine();
  e.setOutfit("necktie", false);
  e.outfitPresence = 1;
  let peak = 0;
  e.locks.add("yaw");
  e.squash();
  for (let i = 0; i < 40; i++) {
    e.yaw = Math.min(0.9, i * 0.06);
    e.update(0.016);
    peak = Math.max(peak, Math.abs(e.swing));
  }
  assert.ok(peak > 0.005, `the tie swings while he turns and hops (peak ${peak})`);
  e.locks.delete("yaw");
  e.tgYaw = e.yaw;
  for (let i = 0; i < 900; i++) e.update(0.016);
  assert.equal(e.swing, 0, "at rest the swing is exactly 0");
  assert.deepEqual([e.physDx, e.physVx, e.physDy, e.physVy, e.swingV], [0, 0, 0, 0, 0], "every spring is exactly still, so they ask for no redraw");
});

test("with no outfit, or at the tiny size, nothing swings", async () => {
  const { BotEngine } = await import("../src/mochi/engine.ts");
  const e = new BotEngine();
  e.tgYaw = 0.9;
  for (let i = 0; i < 30; i++) e.update(0.016);
  assert.equal(e.swing, 0);
  const f = new BotEngine();
  f.lite = true;
  f.setOutfit("necktie", false);
  f.tgYaw = 0.9;
  for (let i = 0; i < 30; i++) f.update(0.016);
  assert.equal(f.swing, 0);
});
