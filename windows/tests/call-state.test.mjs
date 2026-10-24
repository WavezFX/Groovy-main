// The voice call's state machine (src/voice/call-state.ts), barge-in, the
// level meter, and the microphone constraints.

import { test } from "node:test";
import assert from "node:assert/strict";
import { BargeIn, LevelMeter, nextCallState } from "../src/voice/call-state.ts";
import { micConstraints } from "../src/voice/mic.ts";

const run = (events, from = "idle") => events.reduce((s, e) => nextCallState(s, e), from);

test("a call goes listening → processing → speaking → listening", () => {
  assert.equal(run(["start"]), "listening");
  assert.equal(run(["start", "heard"]), "processing");
  assert.equal(run(["start", "heard", "answered"]), "speaking");
  assert.equal(run(["start", "heard", "answered", "spoke"]), "listening");
  // An answer that isn't read aloud goes straight back to listening.
  assert.equal(run(["start", "heard", "silent"]), "listening");
});

test("speaking can be interrupted, and mute holds until unmuted", () => {
  assert.equal(run(["start", "heard", "answered", "interrupt"]), "listening");
  assert.equal(run(["start", "mute"]), "muted");
  // Answers still arrive while muted, and it stays muted.
  assert.equal(run(["start", "heard", "mute", "answered", "spoke"]), "muted");
  assert.equal(run(["start", "mute", "heard"]), "muted", "nothing is heard while muted");
  assert.equal(run(["start", "mute", "unmute"]), "listening");
});

test("failures and hanging up", () => {
  assert.equal(run(["start", "fail"]), "error");
  assert.equal(run(["start", "fail", "start"]), "listening", "trying again");
  assert.equal(run(["fail"]), "idle", "no call, nothing to fail");
  for (const s of ["listening", "processing", "speaking", "muted", "error"]) assert.equal(nextCallState(s, "end"), "idle");
  // Events that don't apply change nothing.
  assert.equal(nextCallState("idle", "heard"), "idle");
  assert.equal(nextCallState("listening", "spoke"), "listening");
});

test("barge-in needs speech held for a moment, well above the noise", () => {
  const b = new BargeIn(4, 24, 1e-4);
  const noise = 1e-6;
  // Quiet room, then a click: nothing.
  assert.ok(![1e-6, 1e-3, 1e-6].some((p) => b.feed(p, noise)));
  // Speech for four frames in a row: yes, on the fourth.
  assert.deepEqual([1e-3, 1e-3, 1e-3, 1e-3].map((p) => b.feed(p, noise)), [false, false, false, true]);
  // In a loud room the threshold rises with the noise floor.
  assert.ok(![1e-3, 1e-3, 1e-3, 1e-3, 1e-3].some((p) => b.feed(p, 1e-4)));
});

test("the level meter rises fast and falls slowly, within 0…1", () => {
  const m = new LevelMeter();
  assert.equal(m.feed(1e-12), 0);
  const loud = m.feed(0.1);
  assert.ok(loud > 0.9 && loud <= 1);
  const after = m.feed(1e-12);
  assert.ok(after > 0.7 && after < loud, "slow release");
});

test("the picked microphone is asked for exactly, with noise suppression", () => {
  assert.deepEqual(micConstraints().audio, { channelCount: 1, echoCancellation: true, noiseSuppression: true, autoGainControl: true });
  assert.deepEqual(micConstraints("abc").audio.deviceId, { exact: "abc" });
});
