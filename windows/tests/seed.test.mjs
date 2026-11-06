// The Seed intro (FIXES_ROUND3 item 7) and Groovy's own sounds, made in code.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { SEED_ISLAND, SeedIntro, seedKind, seedPlan, seedPose } from "../src/mochi/seed.ts";
import { RECIPES, SAMPLE_RATE, flavorOf, renderSound } from "../src/core/sounds-synth.ts";

/** Every sound ends within this, in seconds (the longest is approval, about 0.9 s). */
const MAX_SECONDS = 1.2;
import { SOUND_NAMES } from "../src/core/sound.ts";
import { parseGrowth } from "../src/mochi/growth.ts";

globalThis.Path2D ??= class { constructor() { return new Proxy(this, { get: (t, k) => (k in t ? t[k] : () => {}) }); } };

function fakeContext() {
  const gradient = { addColorStop() {} };
  return new Proxy({}, {
    get(t, k) {
      if (k in t) return t[k];
      if (String(k).startsWith("create")) return () => gradient;
      if (k === "measureText") return () => ({ width: 0 });
      return (...args) => {
        for (const a of args) if (typeof a === "number") assert.ok(Number.isFinite(a), `${String(k)} got ${a}`);
      };
    },
    set(t, k, v) {
      t[k] = v;
      return true;
    },
  });
}

const look = (stage, next = stage) => ({ skin: "ember", stage, next, appearance: null, outfit: "none", layers: [], lite: false });
const lines = { title: "Hi", sub: "there" };

test("the first launch, a stage reached since, a plain hello, or still with reduced motion", () => {
  assert.equal(seedKind(-1, 0, false), "first");
  assert.equal(seedKind(2, 3, false), "grew");
  assert.equal(seedKind(3, 3, false), "back");
  assert.equal(seedKind(-1, 0, true), "still");
  assert.equal(seedKind(2, 3, true), "still");
  // The record says which: -1 before the first intro.
  assert.equal(parseGrowth({}).seen, -1);
  assert.equal(parseGrowth({ activeSeconds: 9e6, seen: 5 }).seen, 5);
});

test("about 3 s the first time, about 1 s after, half a second when still", () => {
  assert.ok(Math.abs(seedPlan("first").end - 3) <= 0.2);
  assert.ok(Math.abs(seedPlan("back").end - 1) <= 0.1);
  assert.ok(seedPlan("grew").end <= 2.2);
  assert.equal(seedPlan("still").end, 0.5);
  for (const kind of ["first", "grew", "back"]) {
    const p = seedPlan(kind);
    assert.ok(p.land < p.crack && p.crack <= p.pop && p.pop < p.end, kind);
  }
});

test("the seed falls, cracks, and Groovy pops out with an overshoot", () => {
  const p = seedPlan("first");
  const before = seedPose(p.land - 0.01, "first");
  assert.ok(before.seedAlpha > 0 && before.bot === 0 && before.botAlpha === 0);
  assert.equal(seedPose(p.crack + 0.01, "first").seedScale, 0, "cracked");
  let peak = 0;
  for (let t = p.pop; t < p.pop + 0.4; t += 0.01) peak = Math.max(peak, seedPose(t, "first").bot);
  assert.ok(peak > 1.1 && peak <= 1.16, `peak ${peak}`);
  const end = seedPose(p.end, "first");
  assert.equal(end.bot, 1);
  assert.equal(end.text, 1);
  assert.ok(end.grown);
  assert.ok(!seedPose(p.grow - 0.01, "first").grown, "the island grows only after he is out");
  // Still: no seed, no crack, only a fade.
  const still = seedPose(0.25, "still");
  assert.equal(still.seedAlpha, 0);
  assert.ok(still.botAlpha > 0.4 && still.botAlpha < 0.6);
  assert.ok(still.grown);
  assert.ok(SEED_ISLAND.w < 200 && SEED_ISLAND.h <= 150);
});

test("Groovy comes out at his saved stage; a new one grows in on the extra beat", () => {
  const back = new SeedIntro("back", look(3), lines);
  assert.equal(back.engine.stage, 3);
  const first = new SeedIntro("first", look(0), lines);
  assert.equal(first.engine.stage, 0);
  const grew = new SeedIntro("grew", look(2, 3), lines);
  grew.start(0);
  const ctx = fakeContext();
  grew.draw(ctx, 500);
  assert.equal(grew.engine.stage, 2, "the stage he had");
  grew.draw(ctx, (grew.plan.touch + 0.1) * 1000);
  assert.equal(grew.engine.stage, 3, "then the new one");
});

test("every kind draws to the end, grows the island once and lets go of everything", () => {
  for (const kind of ["first", "grew", "back", "still"]) {
    const intro = new SeedIntro(kind, look(kind === "first" ? 0 : 4, 5), lines);
    let grown = 0;
    let done = 0;
    intro.onGrow = () => grown++;
    intro.onComplete = () => done++;
    intro.start(0);
    const ctx = fakeContext();
    for (let ms = 0; ms <= (intro.plan.end + 0.1) * 1000; ms += 16) intro.draw(ctx, ms);
    assert.equal(done, 1, kind);
    assert.ok(intro.done);
    assert.equal(intro.engine, null, "not kept in memory");
    if (kind !== "still") assert.equal(grown, 1, kind);
    assert.equal(intro.draw(ctx, 99999), false);
  }
});

test("a click skips it; the pointer on the island holds the end", () => {
  const intro = new SeedIntro("first", look(0), lines);
  let grown = 0;
  let done = 0;
  intro.onGrow = () => grown++;
  intro.onComplete = () => done++;
  intro.start(0);
  intro.draw(fakeContext(), 100);
  intro.skip();
  assert.deepEqual([grown, done], [1, 1]);
  intro.skip();
  assert.equal(done, 1, "once");

  const held = new SeedIntro("back", look(1), lines);
  held.start(0);
  held.hover();
  held.draw(fakeContext(), 5000);
  assert.ok(!held.done, "held while the pointer is on it");
  held.interrupt();
  held.draw(fakeContext(), 5016);
  assert.ok(held.done);
});

// ── Sounds ─────────────────────────────────────────────────────────────────────

test("every sound the island plays is made in code, short and clean", () => {
  for (const name of SOUND_NAMES) {
    assert.ok(RECIPES[name], `${name} has a recipe`);
    for (const flavor of ["wood", "warm", "glass"]) {
      const s = renderSound(name, flavor);
      assert.ok(s.length > 0 && s.length <= MAX_SECONDS * SAMPLE_RATE, `${name} ${flavor}: ${s.length / SAMPLE_RATE} s`);
      let peak = 0;
      for (const v of s) {
        assert.ok(Number.isFinite(v));
        peak = Math.max(peak, Math.abs(v));
      }
      assert.ok(Math.abs(peak - 0.8) < 1e-3, `${name} peak ${peak}`);
    }
  }
  assert.equal(renderSound("nope", "wood"), null);
  assert.ok(renderSound("greeting", "wood").length < SAMPLE_RATE, "the intro's sound is under a second");
});

test("the look sets the timbre, and a sound is the same every time", () => {
  assert.deepEqual(["night", "ember", "frost", "grove"].map(flavorOf), ["wood", "warm", "glass", "wood"]);
  const a = renderSound("finish", "wood");
  const b = renderSound("finish", "glass");
  assert.notDeepEqual(a.slice(2000, 2100), b.slice(2000, 2100));
  assert.deepEqual(renderSound("slap", "warm"), renderSound("slap", "warm"));
  assert.equal(renderSound("finish", "wood", 22050).length < a.length, true, "follows the context's rate");
});

test("no audio file ships, and SOURCES.md says how each sound is made", () => {
  const dir = new URL("../sounds/", import.meta.url);
  const files = readdirSync(dir);
  assert.deepEqual(files.filter((f) => /\.(wav|mp3|ogg|m4a|flac)$/i.test(f)), []);
  const sources = readFileSync(new URL("SOURCES.md", dir), "utf8");
  for (const name of SOUND_NAMES) assert.ok(sources.includes(`\`${name}\``), `${name} in SOURCES.md`);
});
