// Voice: the wake words, the voice-activity detector and the listening flow,
// fed synthetic audio and a fake speech-to-text.

import { test } from "node:test";
import assert from "node:assert/strict";
import { splitWake, isEndPhrase } from "../src/voice/wake.ts";
import { EnergyVAD } from "../src/voice/vad.ts";
import { VoiceController, SAMPLE_RATE, toInt16 } from "../src/voice/controller.ts";

test("the three wake phrases match, in the spellings speech-to-text uses", () => {
  for (const said of ["Groovy", "Hey Groovy", "yo groovy", "Hey, Groovie!", "OK groovey", "Yo, gruvy."]) {
    assert.equal(splitWake(said).matched, true, said);
  }
});

test("what follows the wake phrase is the command, as it was said", () => {
  assert.deepEqual(splitWake("Hey Groovy, what's on my calendar today?"), {
    matched: true,
    command: "what's on my calendar today?",
  });
  assert.deepEqual(splitWake("Groovy open Spotify"), { matched: true, command: "open Spotify" });
  assert.deepEqual(splitWake("um, Groovy"), { matched: true, command: "" });
  // The last wake phrase wins.
  assert.equal(splitWake("hey groovy hey groovy play music").command, "play music");
});

test("a sentence that only mentions the name doesn't wake him", () => {
  assert.equal(splitWake("that song was groovy").matched, false);
  assert.equal(splitWake("I think Groovy is cool").matched, false);
  assert.equal(splitWake("").matched, false);
  assert.equal(splitWake("good morning").matched, false);
});

test("end phrases are recognised", () => {
  assert.equal(isEndPhrase("Never mind."), true);
  assert.equal(isEndPhrase("stop"), true);
  assert.equal(isEndPhrase("stop the music"), false);
});

// ── Synthetic audio ───────────────────────────────────────────────────────────

const FRAME = 1024;
const quiet = () => Float32Array.from({ length: FRAME }, () => (Math.random() - 0.5) * 0.002);
const loud = () => Float32Array.from({ length: FRAME }, (_, i) => Math.sin(i / 3) * 0.4);
const frames = (seconds) => Math.round((seconds * SAMPLE_RATE) / FRAME);

function feed(c, kind, seconds) {
  for (let i = 0; i < frames(seconds); i++) c.feed(kind === "speech" ? loud() : quiet());
}

const tick = () => new Promise((r) => setTimeout(r, 0));

test("the VAD calibrates, then cuts speech into one segment", () => {
  const fps = SAMPLE_RATE / FRAME;
  const vad = new EnergyVAD(fps, 0.7);
  const events = [];
  const run = (make, seconds) => {
    for (let i = 0; i < frames(seconds); i++) {
      const f = make();
      let p = 0;
      for (const v of f) p += v * v;
      const e = vad.feed(p / f.length);
      if (e !== "none") events.push(e);
    }
  };
  run(quiet, 1);
  run(loud, 1);
  run(quiet, 1.5);
  assert.deepEqual(events, ["start", "end"]);
});

function controller(answers) {
  const heard = [];
  const log = [];
  const c = new VoiceController(
    async (pcm, purpose) => {
      heard.push([purpose, pcm.length / SAMPLE_RATE]);
      return answers.shift() ?? "";
    },
    {
      onWake: () => log.push("wake"),
      onCommand: (t) => log.push(`command:${t}`),
      onCancel: () => log.push("cancel"),
    },
  );
  c.start();
  return { c, log, heard };
}

test("“Hey Groovy, …” in one breath goes straight through as a command", async () => {
  const { c, log } = controller(["hey groovy what time is it", "Hey Groovy, what time is it?"]);
  feed(c, "quiet", 1);
  feed(c, "speech", 1.5);
  feed(c, "quiet", 1.2);
  await tick();
  await tick();
  assert.deepEqual(log, ["command:what time is it?"]);
  assert.equal(c.phase, "wake");
});

test("a bare “Groovy” opens listening, and the next thing said is the command", async () => {
  const { c, log } = controller(["Groovy.", "turn the music down"]);
  feed(c, "quiet", 1);
  feed(c, "speech", 0.6);
  feed(c, "quiet", 1);
  await tick();
  assert.deepEqual(log, ["wake"]);
  assert.equal(c.phase, "command");
  feed(c, "quiet", 1);
  feed(c, "speech", 1.5);
  feed(c, "quiet", 2);
  await tick();
  assert.deepEqual(log, ["wake", "command:turn the music down"]);
  assert.equal(c.phase, "wake");
});

test("silence after “Groovy” gives up and goes back to waiting", async () => {
  const { c, log } = controller(["groovy"]);
  feed(c, "quiet", 1);
  feed(c, "speech", 0.6);
  feed(c, "quiet", 1);
  await tick();
  feed(c, "quiet", 7);
  assert.deepEqual(log, ["wake", "cancel"]);
  assert.equal(c.phase, "wake");
});

test("other talk is ignored, and “never mind” cancels", async () => {
  const { c, log } = controller(["what a nice day", "groovy", "never mind"]);
  feed(c, "quiet", 1);
  feed(c, "speech", 1);
  feed(c, "quiet", 1);
  await tick();
  assert.deepEqual(log, []);
  feed(c, "speech", 0.6);
  feed(c, "quiet", 1);
  await tick();
  feed(c, "speech", 1);
  feed(c, "quiet", 2);
  await tick();
  assert.deepEqual(log, ["wake", "cancel"]);
});

test("paused, the controller hears nothing until it resumes", async () => {
  const { c, log, heard } = controller(["hey groovy hello"]);
  feed(c, "quiet", 1);
  c.pause();
  feed(c, "speech", 1);
  feed(c, "quiet", 1);
  await tick();
  assert.equal(heard.length, 0);
  c.resume();
  feed(c, "quiet", 0.5);
  feed(c, "speech", 1);
  feed(c, "quiet", 1);
  await tick();
  await tick();
  assert.deepEqual(log, ["command:hello"]);
});

test("samples go to Rust as 16-bit integers", () => {
  assert.deepEqual(toInt16(Float32Array.from([0, 1, -1, 2, 0.5])), [0, 32767, -32767, 32767, 16384]);
});

test("in a call he listens for the next turn without the wake word, and gives up after a long quiet", async () => {
  const { c, log } = controller(["what's the weather"]);
  feed(c, "quiet", 1);
  c.listen(12);
  assert.equal(c.phase, "command");
  feed(c, "speech", 1.2);
  feed(c, "quiet", 2);
  await tick();
  assert.deepEqual(log, ["command:what's the weather"]);
  c.listen(12);
  feed(c, "quiet", 11);
  assert.deepEqual(log, ["command:what's the weather"]);
  feed(c, "quiet", 2);
  assert.deepEqual(log, ["command:what's the weather", "cancel"]);
});
