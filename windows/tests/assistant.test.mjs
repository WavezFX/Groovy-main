// The assistant's pieces on the page side: the defaults match the Rust side,
// the model manager's sizes and fit, reminder times, and snoozing a reminder.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { DEFAULT_ASSISTANT, DEFAULT_SETTINGS } from "../src/core/state.ts";
import { fit, formatBytes } from "../src/settings/models.ts";
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

test("a model fits, is tight, or does not fit in the memory free now", () => {
  const GB = 1024 ** 3;
  assert.equal(fit(1 * GB, 4 * GB, 0.6), "ok");
  assert.equal(fit(2.2 * GB, 4 * GB, 0.6), "tight");
  assert.equal(fit(3 * GB, 4 * GB, 0.6), "no");
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
