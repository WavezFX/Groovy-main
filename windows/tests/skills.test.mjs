import assert from "node:assert/strict";
import { test } from "node:test";
import { installId, pending, removeId, risks } from "../src/core/skills.ts";

const recs = (over = {}) => ({
  install: [
    { source: "o/r", skill: "react-best-practices", reason: "React", confidence: 0.9, installs: 10 },
    { source: "o/r", skill: "vitest-testing", reason: "Vitest", confidence: 0.6, installs: 3 },
    { source: "o/r", skill: "already-here", reason: "x", confidence: 0.8, installs: 1 },
  ],
  remove: [{ skill: "swiftui-tips", reason: "no Swift" }, { skill: "gone-already", reason: "x" }],
  keep: [],
  by: "rules",
  at: 1,
  problems: [],
  dismissed: [],
  canUndo: false,
  ai: false,
  ...over,
});
const installed = (names) => names.map((name) => ({ name, path: `/p/${name}`, scope: "project", agents: [], source: null, sourceUrl: null, sourceType: null, description: "", skillMd: null, edited: false, allowedTools: [] }));

test("recommendations hide what is installed, dismissed or already gone", () => {
  const p = pending(recs({ dismissed: [installId({ source: "o/r", skill: "vitest-testing" })] }), installed(["already-here", "swiftui-tips"]));
  assert.deepEqual(p.install.map((r) => r.skill), ["react-best-practices"]);
  assert.deepEqual(p.remove.map((r) => r.skill), ["swiftui-tips"]);
  const kept = pending(recs({ dismissed: [removeId("swiftui-tips")] }), installed(["swiftui-tips"]));
  assert.deepEqual(kept.remove, []);
});

test("ids match the backend's", () => {
  assert.equal(installId({ source: "anthropics/skills", skill: "pdf" }), "install:anthropics/skills@pdf");
  assert.equal(removeId("pdf"), "remove:pdf");
});

test("a preview's risks are its allowed tools and scripts", () => {
  const r = risks({ frontmatter: { name: "x", description: "y", allowedTools: ["Bash"] }, scripts: ["scripts/run.sh"] });
  assert.deepEqual(r, { tools: ["Bash"], scripts: ["scripts/run.sh"] });
  assert.deepEqual(risks({ frontmatter: { name: null, description: null, allowedTools: [] }, scripts: [] }), { tools: [], scripts: [] });
});
