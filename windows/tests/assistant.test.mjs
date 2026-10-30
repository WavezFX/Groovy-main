// The assistant's pieces on the page side: the defaults match the Rust side,
// the model manager's sizes and fit, reminder times, and snoozing a reminder.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { DEFAULT_ASSISTANT, DEFAULT_SETTINGS } from "../src/core/state.ts";
import { catalogRam, fit, formatBytes } from "../src/settings/models.ts";
import { useLight, wordsPerSecond } from "../src/settings/lightlocal.ts";
import { dueLabel, parseLocal } from "../src/settings/tasks.ts";
import { snoozeReminder } from "../src/island/breaks.ts";

test("the assistant defaults are the ones assistant.rs has", () => {
  const rs = readFileSync(new URL("../src-tauri/src/assistant.rs", import.meta.url), "utf8");
  const block = rs.slice(rs.indexOf("impl Default for AssistantPrefs"), rs.indexOf("pub const PERSONALITIES"));
  const camel = (s) => s.replace(/_([a-z])/g, (_, c) => c.toUpperCase());
  let checked = 0;
  for (const m of block.matchAll(/^\s+(\w+): (.+),$/gm)) {
    const key = camel(m[1]);
    const raw = m[2];
    const value = raw.startsWith('"') ? raw.slice(1, raw.indexOf('"', 1))
      : raw === "String::new()" ? ""
      : raw === "Vec::new()" ? []
      : /BTreeMap::new\(\)$/.test(raw) ? {}
      : raw === "true" || raw === "false" ? raw === "true"
      : Number(raw);
    assert.deepEqual(DEFAULT_ASSISTANT[key], value, key);
    checked++;
  }
  assert.equal(checked, Object.keys(DEFAULT_ASSISTANT).length, "every field is compared");
  assert.equal(DEFAULT_SETTINGS.assistant, DEFAULT_ASSISTANT);
});

test("sizes read like a person would write them", () => {
  assert.equal(formatBytes(491_400_032), "469 MB");
  assert.equal(formatBytes(2_497_280_256), "2.3 GB");
  assert.equal(formatBytes(9_216), "9 KB");
});

test("a model is judged by the PC, not by the memory free this second", () => {
  const GB = 1024 ** 3;
  const MB = 1024 ** 2;
  // The owner's 8 GB PC at 91 %: the 0.5B model still suits it, memory is just short now.
  assert.equal(fit(600 * MB, 8 * GB, 700 * MB), "fits");
  assert.equal(fit(970 * MB, 8 * GB, 700 * MB), "short");
  assert.equal(fit(3 * GB, 8 * GB, 6 * GB), "fits");
  assert.equal(fit(4 * GB, 8 * GB, 6 * GB), "too-big");
});

test("catalogue models use their measured memory, else an estimate", () => {
  const MB = 1024 ** 2;
  assert.equal(catalogRam({ ramMb: 971, size: 806_058_240 }), 971 * MB);
  assert.equal(catalogRam({ ramMb: 0, size: 1000 * MB }), 1250 * MB + 350 * MB);
});

test("Light local uses Groovy's own engine with the picked model", () => {
  const next = useLight({ ...DEFAULT_ASSISTANT, aiSetup: "", localProvider: "ollama" }, "gemma-3-1b-it-Q4_K_M.gguf");
  assert.equal(next.chatProvider, "llamacpp");
  assert.equal(next.assistant.aiSetup, "local");
  assert.equal(next.assistant.localProvider, "llamacpp");
  assert.equal(next.assistant.localModel, "gemma-3-1b-it-Q4_K_M.gguf");
  assert.equal(wordsPerSecond(18), 14);
  assert.equal(wordsPerSecond(0), 1);
});

test("reminder times come from the date field in local time", () => {
  assert.equal(parseLocal(""), null);
  assert.equal(parseLocal("not a date"), null);
  const secs = parseLocal("2026-10-15T09:30");
  assert.equal(secs, Math.floor(new Date(2026, 9, 15, 9, 30).getTime() / 1000));
  const task = { id: 1, text: "x", dueAt: 100, done: false, notified: false, createdAt: 0 };
  assert.match(dueLabel(task, 200), /^overdue · /);
  assert.doesNotMatch(dueLabel({ ...task, done: true }, 200), /overdue/);
  assert.equal(dueLabel({ ...task, dueAt: null }), "");
});

test("snoozing a reminder closes it and adds the same one ten minutes later", async () => {
  const calls = [];
  const bridge = {
    taskSetDone: async (id, done) => calls.push(["done", id, done]),
    taskAdd: async (text, dueAt) => calls.push(["add", text, dueAt]),
  };
  await snoozeReminder(bridge, { id: 7, text: "Stand-up" }, 1_000_000);
  assert.deepEqual(calls, [["done", 7, true], ["add", "Stand-up", 1000 + 600]]);
});

// ── The AI setup: cloud first, local last (FIXES_ROUND1 item 2) ──────────────
import { applyChoice, localAdvice } from "../src/settings/aisetup.ts";

test("nothing is chosen at first, and the brain follows the setup", () => {
  assert.equal(DEFAULT_ASSISTANT.aiSetup, "");
  assert.equal(DEFAULT_ASSISTANT.brain, "auto");
});

test("each setup choice leads to the right settings", () => {
  const g = applyChoice(DEFAULT_ASSISTANT, { kind: "cloud", provider: "google" });
  assert.equal(g.assistant.aiSetup, "cloud");
  assert.equal(g.chatProvider, "google");
  const s = applyChoice({ ...DEFAULT_ASSISTANT, aiMode: "local" }, { kind: "subscription", tool: "codex" });
  assert.deepEqual([s.assistant.aiSetup, s.assistant.brain, s.assistant.aiMode, s.chatProvider], ["subscription", "codex", "hybrid", null]);
  const l = applyChoice(DEFAULT_ASSISTANT, { kind: "local" });
  assert.deepEqual([l.assistant.aiSetup, l.chatProvider], ["local", "llamacpp"]);
});

test("local AI is only recommended from 16 GB, and 8 GB gets a warning", () => {
  const GB = 1024 ** 3;
  assert.deepEqual(localAdvice(8 * GB), { recommended: false, warning: "slow" });
  assert.deepEqual(localAdvice(12 * GB), { recommended: false, warning: "tight" });
  assert.deepEqual(localAdvice(32 * GB), { recommended: true, warning: null });
});
