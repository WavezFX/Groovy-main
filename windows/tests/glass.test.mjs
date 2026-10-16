import { test } from "node:test";
import assert from "node:assert/strict";
import { glassAlpha, glassStrength } from "../src/core/glass.ts";

test("the glass island is see-through: a light tint over the blur, a little more without it", () => {
  const blur = glassAlpha("blur", "balanced");
  const css = glassAlpha("css", "balanced");
  assert.ok(blur >= 0.3 && blur <= 0.4, `blur tint ${blur}`);
  assert.ok(css >= 0.5 && css <= 0.55, `css tint ${css}`);
  assert.equal(glassAlpha("solid", "clear"), 1);
});

test("Clear < Balanced < Frosted, and a bright backdrop only raises it so far", () => {
  for (const look of ["blur", "css"]) {
    assert.ok(glassAlpha(look, "clear") < glassAlpha(look, "balanced"));
    assert.ok(glassAlpha(look, "balanced") < glassAlpha(look, "frosted"));
    assert.ok(glassAlpha(look, "balanced", 1) > glassAlpha(look, "balanced"));
    assert.equal(glassAlpha(look, "balanced", 5), glassAlpha(look, "balanced", 1));
    assert.equal(glassAlpha(look, "balanced", Number.NaN), glassAlpha(look, "balanced"));
    assert.ok(glassAlpha(look, "frosted", 1) < 0.85, "still see-through at its most");
  }
});

test("an unknown strength is Balanced", () => {
  assert.equal(glassStrength(undefined), "balanced");
  assert.equal(glassStrength("frosted"), "frosted");
  assert.equal(glassStrength("opaque"), "balanced");
});
