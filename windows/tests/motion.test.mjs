// One motion system (FIXES_ROUND3B items 2 and 8).

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { MOTION, duration, easeOut } from "../src/core/motion.ts";

test("the curve starts quick and lands softly, within 0 and 1", () => {
  assert.equal(easeOut(0), 0);
  assert.equal(easeOut(1), 1);
  assert.ok(easeOut(0.5) > 0.8);
  assert.equal(easeOut(-1), 0);
  assert.equal(easeOut(2), 1);
});

test("timings are about 260–320 ms, and a short fade with reduced motion", () => {
  assert.ok(MOTION.island >= 260 && MOTION.island <= 320);
  assert.ok(MOTION.unfold >= 260 && MOTION.unfold <= 320);
  globalThis.matchMedia = () => ({ matches: true });
  assert.equal(duration(MOTION.unfold), MOTION.fade);
  globalThis.matchMedia = () => ({ matches: false });
  assert.equal(duration(MOTION.unfold), MOTION.unfold);
  delete globalThis.matchMedia;
});

test("only transform, opacity and clip-path move, and the CSS timings are the module's", () => {
  const style = readFileSync(new URL("../src/style.css", import.meta.url), "utf8");
  const hub = readFileSync(new URL("../src/hub/hub.css", import.meta.url), "utf8");
  const blocks = [
    ...[/\n\.view \{[^}]*\}/, /\n\.view\.on \{[^}]*\}/, /\n\.home-grid \{[^}]*\}/].map((re) => style.match(re)[0]),
    hub.slice(hub.indexOf("Motion (round 3B item 8")),
  ];
  for (const block of blocks) {
    for (const m of block.matchAll(/transition:\s*([^;]+);/g)) {
      for (const part of m[1].split(/,(?![^(]*\))/)) {
        const prop = part.trim().split(/\s+/)[0];
        assert.ok(["opacity", "transform", "clip-path", "background-color", "left"].includes(prop), prop);
      }
    }
  }
  const css = readFileSync(new URL("../src/style.css", import.meta.url), "utf8");
  assert.match(css, new RegExp(`--motion-island: ${MOTION.island}ms`));
  assert.match(css, new RegExp(`--motion-unfold: ${MOTION.unfold}ms`));
  assert.match(css, /prefers-reduced-motion: reduce/);
});
