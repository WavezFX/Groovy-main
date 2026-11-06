import { test } from "node:test";
import assert from "node:assert/strict";
import { frameStats, frameTour, frameVerdict } from "../src/core/frametest.ts";

test("smooth frames pass; a stall or too many slow frames fail", () => {
  const smooth = frameStats(Array(100).fill(16.7));
  assert.equal(frameVerdict(smooth), null);
  assert.equal(smooth.p95, 16.7);
  const fewSlow = frameStats([...Array(96).fill(16.7), 25, 25, 25, 25]);
  assert.equal(frameVerdict(fewSlow), null, "4% over 20 ms is fine");
  const manySlow = frameStats([...Array(90).fill(16.7), ...Array(10).fill(25)]);
  assert.match(frameVerdict(manySlow), /10\.0% of frames/);
  const stall = frameStats([...Array(200).fill(16.7), 60]);
  assert.match(frameVerdict(stall), /over 50 ms/);
  assert.equal(frameVerdict(frameStats([])), "no frames were recorded");
});

test("the tour records each step's frames", async () => {
  let t = 0;
  const queue = [];
  const raf = (cb) => queue.push(cb);
  const wait = async () => {
    for (let i = 0; i < 5; i++) {
      t += 16;
      const cbs = queue.splice(0);
      for (const cb of cbs) cb(t);
    }
  };
  const seen = [];
  const out = await frameTour([{ name: "F", run: () => seen.push("F") }, { name: "E", run: () => seen.push("E") }], 0, raf, wait);
  assert.deepEqual(seen, ["F", "E"]);
  assert.ok(out.F.length >= 4 && out.E.length === 5);
  assert.ok(out.E.every((d) => d === 16));
});
