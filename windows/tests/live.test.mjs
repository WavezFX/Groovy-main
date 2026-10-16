// The island's live state, Groovy's notices and the voice he answers with
// (src/island/live.ts) — what the bottom pill used to do (round 3 item 1).

import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { Notices, liveBusy, noticeIcon, systemVoice } from "../src/island/live.ts";

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
