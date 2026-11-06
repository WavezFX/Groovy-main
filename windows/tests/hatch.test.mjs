// Choose your Groovy and name him at hatching (round 5 item 6).

import { test, mock } from "node:test";
import assert from "node:assert/strict";
import { installFakeDom } from "./fakedom.mjs";
import { checkName, nameOf, syllables, wakeName } from "../src/mochi/naming.ts";

installFakeDom();

test("a name is 1-20 characters of letters, numbers, spaces and simple punctuation", () => {
  assert.deepEqual(checkName("  Pip  "), { ok: true, name: "Pip" });
  assert.deepEqual(checkName("Sir   Pips-a-lot!"), { ok: true, name: "Sir Pips-a-lot!" });
  assert.deepEqual(checkName("Zoë"), { ok: true, name: "Zoë" });
  assert.deepEqual(checkName("小明"), { ok: true, name: "小明" });
  assert.deepEqual(checkName("R2"), { ok: true, name: "R2" });
  assert.equal(checkName("").ok, false, "empty");
  assert.equal(checkName("   ").ok, false, "only spaces");
  assert.equal(checkName("Pip 🐣").ok, false, "emoji");
  assert.equal(checkName("<b>Pip</b>").ok, false, "markup");
  assert.equal(checkName("...").ok, false, "punctuation only");
  assert.equal(checkName("a".repeat(20)).ok, true);
  assert.equal(checkName("a".repeat(21)).ok, false, "21 characters");
});

test("his name, or Groovy", () => {
  assert.equal(nameOf({ assistant: { name: "Pip" } }), "Pip");
  assert.equal(nameOf({ assistant: { name: "  " } }), "Groovy");
  assert.equal(nameOf({}), "Groovy");
});

test("waking to his name needs two syllables or more, in English letters", () => {
  assert.equal(syllables("Pip"), 1);
  assert.equal(syllables("Pippa"), 2);
  assert.equal(syllables("Mochi"), 2);
  assert.equal(syllables("Jake"), 1, "a silent e");
  assert.equal(syllables("Captain Pip"), 3);
  assert.equal(wakeName("Pip"), "short");
  assert.equal(wakeName("Pippa"), "ok");
  assert.equal(wakeName("Zoë"), "letters");
  assert.equal(wakeName("Groovy"), "groovy");
});

test("the picker: tap a hatchling, the others go, then name him", async () => {
  globalThis.Path2D ??= class { constructor() { return new Proxy(this, { get: (t, k) => (k in t ? t[k] : () => {}) }); } };
  const { buildHatch } = await import("../src/views/hatch.ts");
  const calls = [];
  const view = buildHatch({ hatch: (look, name) => calls.push([look, name]), previewLook: () => {} });
  // sync() draws the previews (canvas): left out here.
  const picks = view.el.find(".hatch-pick");
  assert.equal(picks.length, 3, "Night, Ember and Frost");
  const naming = view.el.querySelector(".hatch-naming");
  assert.equal(naming.hidden, true, "the name comes after the choice");
  mock.timers.enable({ apis: ["setTimeout"] });
  try {
    picks[1].fire("click");
    mock.timers.tick(400);
  } finally {
    mock.timers.reset();
  }
  assert.ok(picks[1].classList.contains("chosen"));
  assert.ok(picks[0].classList.contains("gone") && picks[2].classList.contains("gone"));
  assert.equal(naming.hidden, false);
  const input = view.el.querySelector(".hatch-name");
  assert.equal(input.value, "Groovy", "pre-filled");
  const done = view.el.querySelector(".hatch-done");
  input.value = "  ";
  done.fire("click");
  assert.equal(calls.length, 0, "an empty name is not taken");
  assert.notEqual(view.el.querySelector(".hatch-why").textContent, "");
  input.value = "Pip 🐣";
  done.fire("click");
  assert.equal(calls.length, 0, "nor one with an emoji");
  input.value = " Pippa ";
  done.fire("click");
  assert.deepEqual(calls, [["ember", "Pippa"]]);
});

test("new installs are picked and named; the hatched aren't asked again", async () => {
  const { DEFAULT_SETTINGS } = await import("../src/core/state.ts");
  assert.equal(DEFAULT_SETTINGS.hatchDone, false);
  assert.equal(DEFAULT_SETTINGS.wakeToName, false);
  const { readFileSync } = await import("node:fs");
  const island = readFileSync("src/island/island.ts", "utf8");
  assert.match(island, /const choose = g\.seen < 0 && !State\.settings\.hatchDone;/, "only a first hatching shows the picker");
});
