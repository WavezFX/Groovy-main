// The island's live state, Groovy's notices and the voice he answers with
// (src/island/live.ts) — what the bottom pill used to do (round 3 item 1).

import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { LiveClock, Notices, liveBusy, liveClock, liveView, noticeIcon, systemVoice } from "../src/island/live.ts";

const idle = { phase: "idle", key: "right_ctrl", noteStartedAt: null, enabled: true, voice: null };

test("the island is busy while dictating, recording a call or talking with OK Groovy", () => {
  assert.equal(liveBusy(idle), false);
  assert.equal(liveBusy({ ...idle, phase: "listening" }), true);
  assert.equal(liveBusy({ ...idle, noteStartedAt: 1 }), true);
  assert.equal(liveBusy({ ...idle, voice: { phase: "listening", heard: "", answer: "" } }), true);
});

test("notices wait in order, once each, and go when answered", () => {
  Notices.list = [];
  const n = (id) => ({ id, icon: "error", tint: "#F05252", text: `n${id}`, seconds: 8 });
  Notices.push(n(1));
  Notices.push(n(2));
  Notices.push(n(1));
  assert.deepEqual(Notices.list.map((x) => x.id), [1, 2]);
  Notices.dismiss(1);
  assert.equal(Notices.current?.id, 2);
  Notices.act(2);
  assert.equal(Notices.current, null);
});

test("every icon Rust sends has one in the island", () => {
  for (const i of ["mic", "lock", "clipboard", "download", "check", "error", "bolt"]) assert.ok(noticeIcon(i));
  assert.equal(noticeIcon("unknown"), "check");
});

test("OK Groovy speaks in the answer's language, with the voice asked for", () => {
  const v = (name, lang) => ({ name, lang });
  const voices = [v("Microsoft David", "en-US"), v("Microsoft Zira", "en-US"), v("Microsoft Hortense", "fr-FR")];
  assert.equal(systemVoice(voices, "fr-FR", "female").name, "Microsoft Hortense");
  assert.equal(systemVoice(voices, "en-US", "male").name, "Microsoft David");
  assert.equal(systemVoice(voices, "en-GB", "female").name, "Microsoft Zira");
  assert.equal(systemVoice(voices, "de-DE", "female"), null);
});

test("there is no second window at the bottom of the screen any more", () => {
  assert.equal(existsSync(new URL("../pill.html", import.meta.url)), false);
  const caps = JSON.parse(readFileSync(new URL("../src-tauri/capabilities/default.json", import.meta.url), "utf8"));
  assert.ok(!caps.windows.includes("pill"));
  assert.ok(!readFileSync(new URL("../vite.config.ts", import.meta.url), "utf8").includes("pill.html"));
});

test("the badge shows the most important listener first", () => {
  assert.equal(liveView(idle, false), null);
  const rec = liveView({ ...idle, noteStartedAt: 1, phase: "listening" }, true);
  assert.deepEqual([rec.kind, rec.tone, rec.stop], ["recording", "red", "stopNote"]);
  const dict = liveView({ ...idle, phase: "locked" }, true);
  assert.deepEqual([dict.kind, dict.tone, dict.label, dict.stop], ["dictating", "green", "Dictating {0}", "finish"]);
  assert.equal(liveView({ ...idle, phase: "thinking" }, false).stop, null);
  assert.equal(liveView(idle, true).stop, "endCall");
  assert.equal(liveView({ ...idle, voice: { phase: "speaking", heard: "", answer: "" } }, false).label, "Speaking…");
  assert.equal(liveView({ ...idle, phase: "pastedAgain" }, false).kind, "message");
  assert.equal(liveView({ ...idle, phase: "failed" }, false).tone, "red");
});

test("the clock counts from when it began, the recording's from Rust's start", () => {
  assert.equal(liveClock(4_000), "0:04");
  assert.equal(liveClock(760_000), "12:40");
  assert.equal(liveClock(3_725_000), "1:02:05");
  const clocks = new LiveClock();
  const dict = { ...idle, phase: "listening" };
  const view = liveView(dict, false);
  assert.equal(clocks.start(view, dict, 1000), 1000);
  assert.equal(clocks.start(view, dict, 5000), 1000, "the same dictation keeps its start");
  assert.equal(clocks.start(null, idle, 6000), null);
  assert.equal(clocks.start(view, dict, 7000), 7000, "a new one starts again");
  const rec = { ...idle, noteStartedAt: 42 };
  assert.equal(clocks.start(liveView(rec, false), rec, 9000), 42);
});
