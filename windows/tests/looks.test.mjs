// The three looks keep their own colours (FIXES_ROUND3B item 3).

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { SKIN_STYLES } from "../src/mochi/skins.ts";

test("Night is plainly green, as Ember is warm and Frost is cool", () => {
  const [r, g, b] = SKIN_STYLES.night.bodyBottom;
  assert.ok(g - r >= 0.15 && g - b >= 0.1, `night bottom ${SKIN_STYLES.night.bodyBottom}`);
  const [er, , eb] = SKIN_STYLES.ember.bodyBottom;
  assert.ok(er > eb, "ember is warm");
  const [fr, , fb] = SKIN_STYLES.frost.bodyBottom;
  assert.ok(fb > fr, "frost is cool");
});

test("nothing recolours the island's Groovy: no tile, plan, theme or hour", () => {
  const island = readFileSync(new URL("../src/island/island.ts", import.meta.url), "utf8");
  for (const line of island.split("\n").filter((l) => l.includes("engine.bodyColor"))) {
    assert.match(line, /engine\.bodyColor = null;/, line);
  }
  const engine = readFileSync(new URL("../src/mochi/engine.ts", import.meta.url), "utf8");
  assert.ok(!/getHours\(|prefers-color-scheme|accentColor/.test(engine));
});
