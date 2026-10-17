import { test } from "node:test";
import assert from "node:assert/strict";
import { glassAlpha, glassStrength } from "../src/core/glass.ts";

test("the glass island is see-through: a light tint over the blur, a little more without it", () => {
  const blur = glassAlpha("blur", "balanced");
  const css = glassAlpha("css", "balanced");
  assert.equal(blur, 0.45, "Balanced over a blur: rgba(20,22,28,0.45)");
  assert.ok(css >= 0.78 && css <= 0.82, `no blur: opaque enough (${css})`);
  assert.equal(glassAlpha("solid", "clear"), 1);
});

test("Clear < Balanced < Frosted, and a bright backdrop only raises it so far", () => {
  for (const look of ["blur", "css"]) {
    assert.ok(glassAlpha(look, "clear") < glassAlpha(look, "balanced"));
    assert.ok(glassAlpha(look, "balanced") < glassAlpha(look, "frosted"));
    assert.ok(glassAlpha(look, "balanced", 1) > glassAlpha(look, "balanced"));
    assert.equal(glassAlpha(look, "balanced", 5), glassAlpha(look, "balanced", 1));
    assert.equal(glassAlpha(look, "balanced", Number.NaN), glassAlpha(look, "balanced"));
    assert.ok(glassAlpha(look, "frosted", 1) <= 0.97, "never quite solid");
  }
});

test("an unknown strength is Balanced", () => {
  assert.equal(glassStrength(undefined), "balanced");
  assert.equal(glassStrength("frosted"), "frosted");
  assert.equal(glassStrength("opaque"), "balanced");
});

test("white text keeps 4.5:1 over a white page at every strength (round 3C item 1)", async () => {
  const { TINT_RGB } = await import("../src/core/glass.ts");
  const lin = (c) => ((c /= 255) <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
  const lum = (rgb) => 0.2126 * lin(rgb[0]) + 0.7152 * lin(rgb[1]) + 0.0722 * lin(rgb[2]);
  for (const look of ["blur", "css"]) {
    for (const strength of ["clear", "balanced", "frosted"]) {
      // Rust raises the boost to 1 over a bright backdrop.
      const a = glassAlpha(look, strength, 1);
      const bg = TINT_RGB.map((t) => a * t + (1 - a) * 255);
      const ratio = 1.05 / (lum(bg) + 0.05);
      assert.ok(ratio >= 4.5, `${look}/${strength}: ${ratio.toFixed(2)}:1`);
    }
  }
});
