// Voice calls, round 2 item 4: answers spoken sentence by sentence as they
// stream in, the call's stages as the island shows them, and the voice test.

import { test } from "node:test";
import assert from "node:assert/strict";
import { SentenceSpeaker, finishedSentences } from "../src/voice/sentences.ts";
import { brainName, stageText } from "../src/voice/stages.ts";
import { runVoiceTest } from "../src/settings/voicetest.ts";
import { DEFAULT_ASSISTANT, DEFAULT_SETTINGS } from "../src/core/state.ts";

test("finished sentences come out as the reply streams in", () => {
  let r = finishedSentences("Hello there! I'm Gro", 0);
  assert.deepEqual(r.sentences, ["Hello there!"]);
  r = finishedSentences("Hello there! I'm Groovy. How are", r.next);
  assert.deepEqual(r.sentences, ["I'm Groovy."]);
  // Not a sentence on its own: "e.g." waits for the rest.
  assert.deepEqual(finishedSentences("Try e.g. a walk. Or tea", 0).sentences, ["Try e.g. a walk."]);
  assert.deepEqual(finishedSentences("Line one\nLine two\n", 0).sentences, ["Line one", "Line two"]);
});

test("the first sentence is spoken before the answer is finished, the rest after", async () => {
  const said = [];
  let first = 0;
  const sp = new SentenceSpeaker(async (s) => { said.push(s); return null; }, () => first++);
  sp.feed("Sure! The capital");
  await Promise.resolve();
  assert.equal(first, 1);
  sp.feed("Sure! The capital of Australia is Canberra. Anything");
  await sp.finish("Sure! The capital of Australia is Canberra. Anything else?");
  assert.deepEqual(said, ["Sure!", "The capital of Australia is Canberra.", "Anything else?"]);
  assert.equal(first, 1);
  assert.equal(sp.failure, null);
});

test("a whole answer at once (cloud) is spoken in full; a failing voice stops with its reason", async () => {
  const said = [];
  const sp = new SentenceSpeaker(async (s) => { said.push(s); return said.length === 2 ? "No voice is installed" : null; }, () => {});
  await sp.finish("One. Two. Three.");
  assert.deepEqual(said, ["One.", "Two."]);
  assert.equal(sp.failure, "No voice is installed");
});

test("stopping (an interruption) drops what was still to say", async () => {
  const said = [];
  const sp = new SentenceSpeaker(async (s) => { said.push(s); return null; }, () => {});
  sp.feed("First. Second. ");
  sp.stop();
  await sp.finish("First. Second. Third.");
  assert.ok(said.length <= 1, String(said));
});

test("each stage of a call reads as what is happening", () => {
  assert.equal(stageText("listening", null, "Gemini", null), "Listening…");
  assert.equal(stageText("processing", "what's the time", "Gemini", null), "Heard “what's the time” · Thinking (Gemini)…");
  assert.equal(stageText("speaking", "x", "Gemini", null), "Speaking…");
  assert.equal(stageText("error", null, "", "Calls need the speech model."), "Calls need the speech model.");
  const s = { ...DEFAULT_SETTINGS, chatProvider: "llamacpp", assistant: { ...DEFAULT_ASSISTANT, localModel: "gemma-3-1b-it-Q4_K_M.gguf" } };
  assert.equal(brainName(s), "Local gemma 3 1b it");
  assert.equal(brainName({ ...DEFAULT_SETTINGS, chatProvider: "google" }), "Gemini");
});

test("the voice test reports every stage, skipping only what needs a failed one", async () => {
  const ok = await runVoiceTest({
    micLevel: async () => 0.6,
    record: async () => "hello Groovy",
    probe: async () => ({ answer: "Hello!", brain: "google" }),
    say: async () => null,
  });
  assert.deepEqual(ok.map((r) => [r.stage, r.ok]), [["mic", true], ["speech", true], ["brain", true], ["voice", true]]);

  const bad = await runVoiceTest({
    micLevel: async () => 0.02,
    record: async () => { throw new Error("should not record"); },
    probe: async () => { throw new Error("Local AI is off."); },
    say: async () => "No voice is installed",
  });
  assert.deepEqual(bad.map((r) => [r.stage, r.ok]), [["mic", false], ["speech", null], ["brain", false], ["voice", false]]);
  assert.equal(bad[2].detail, "Local AI is off.");
  assert.equal(bad[3].detail, "No voice is installed");
});
