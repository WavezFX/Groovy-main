// Groovy's colours, layered accessories, presets and wardrobe packs
// (src/mochi/appearance.ts).

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  DEFAULT_APPEARANCE, SLOT_OF, combine, currentWardrobe, fits, parseWardrobe, randomLook, readPack,
  styleWith, writePack,
} from "../src/mochi/appearance.ts";
import { SKIN_STYLES } from "../src/mochi/skins.ts";
import { OUTFIT_SELECTIONS } from "../src/mochi/wardrobe.ts";

test("every outfit has a slot", () => {
  for (const o of OUTFIT_SELECTIONS) {
    if (o === "auto" || o === "none") continue;
    assert.ok(SLOT_OF[o], o);
  }
});

test("one item per slot, the outfit first, a costume alone", () => {
  assert.deepEqual(combine("beanie", ["sunglasses", "scarf"]), ["beanie", "sunglasses", "scarf"]);
  // A second hat is not worn on top of the first.
  assert.deepEqual(combine("beanie", ["crown", "roundGlasses"]), ["beanie", "roundGlasses"]);
  assert.deepEqual(combine("none", ["sunglasses", "roundGlasses"]), ["sunglasses"]);
  assert.deepEqual(combine("pumpkin", ["sunglasses"]), ["pumpkin"]);
  assert.deepEqual(combine("beanie", ["pumpkin"]), ["beanie"], "no costume over an outfit");
  assert.deepEqual(combine("none", ["nonsense", "crown", "crown"]), ["crown"]);
  assert.ok(fits(["beanie"], "scarf"));
  assert.ok(!fits(["beanie"], "crown"));
  assert.ok(!fits(["pumpkin"], "scarf"));
});

test("colours override the look, empty ones keep it", () => {
  const base = SKIN_STYLES.night;
  assert.equal(styleWith(base, null), base);
  assert.deepEqual(styleWith(base, DEFAULT_APPEARANCE), base);
  const s = styleWith(base, { ...DEFAULT_APPEARANCE, body: "#ff0000", glow: "#00ff00", eyes: "#0000ff" });
  assert.deepEqual(s.bodyTop, [1, 0, 0]);
  assert.deepEqual(s.bodyBottom.map((c) => Math.round(c * 100) / 100), [0.86, 0, 0], "a darker bottom by default");
  assert.equal(s.rimGlow, "rgba(0,255,0,0.9)");
  assert.equal(s.iris, "rgb(0,0,255)");
  assert.equal(s.shape, base.shape, "the shape stays the look's");
  const two = styleWith(base, { ...DEFAULT_APPEARANCE, body: "#ffffff", body2: "#000000" });
  assert.deepEqual(two.bodyBottom, [0, 0, 0]);
});

test("stored wardrobe settings are cleaned on every read", () => {
  const w = parseWardrobe({
    appearance: { body: "red", glow: "#ABCDEF", eyes: "#12345", layers: ["crown", "beanie", "<script>"] },
    presets: [{ name: "  Party\u0007 ", look: "ember", outfit: "partyHat", appearance: {} }, { name: "", look: "x" }, "junk"],
    favourites: ["crown", "crown", "night", "evil"],
  });
  assert.deepEqual(w.appearance, { body: "", body2: "", glow: "#abcdef", eyes: "", layers: ["crown"] });
  assert.equal(w.presets.length, 1);
  assert.equal(w.presets[0].name, "Party");
  assert.equal(w.presets[0].look, "ember");
  assert.deepEqual(w.favourites, ["crown", "night"]);
  assert.deepEqual(parseWardrobe(null).appearance, DEFAULT_APPEARANCE);
  assert.deepEqual(parseWardrobe(5).presets, []);
  // Asked every frame: the same stored object is parsed once.
  const raw = { favourites: ["crown"] };
  assert.equal(currentWardrobe(raw), currentWardrobe(raw));
});

test("packs round-trip, and anything else is refused with a reason", () => {
  const presets = [{ name: "Cosy", look: "frost", outfit: "beanie", appearance: { ...DEFAULT_APPEARANCE, layers: ["scarf"] } }];
  const back = readPack(writePack("Mine", presets));
  assert.deepEqual(back.presets, presets);
  assert.deepEqual(back.problems, []);
  assert.equal(readPack("{not json").problems[0].msg, "The file is not valid JSON.");
  assert.match(readPack('{"presets": []}').problems[0].msg, /not a Groovy wardrobe pack/);
  assert.match(readPack("x".repeat(1024 * 1024 + 1)).problems[0].msg, /1 MB/);
  const partial = readPack(JSON.stringify({ groovyWardrobePack: 1, presets: [{ name: "ok" }, { look: "night" }] }));
  assert.equal(partial.presets.length, 1);
  assert.deepEqual(partial.problems, [{ msg: "Preset {n} has no name and was skipped.", vars: { n: 2 } }]);
  const many = readPack(JSON.stringify({ groovyWardrobePack: 1, presets: Array.from({ length: 60 }, (_, i) => ({ name: `p${i}` })) }));
  assert.equal(many.presets.length, 50);
  assert.equal(many.problems.length, 1);
});

test("a random look is always one that can be worn", () => {
  let seed = 1;
  const rand = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
  for (let i = 0; i < 200; i++) {
    const r = randomLook(rand);
    assert.ok(SKIN_STYLES[r.look]);
    assert.deepEqual(combine(r.outfit, r.appearance.layers), [r.outfit, ...r.appearance.layers].filter((o) => o !== "none"));
  }
});

// ── Wardrobe Studio clicks are never refused (FIXES_ROUND1 item 1) ───────────
import { layersOn, wear } from "../src/mochi/appearance.ts";

const OCTOBER = new Date(2026, 9, 15);
const DECEMBER = new Date(2026, 11, 10);
const MAY = new Date(2026, 4, 12);

test("October on Auto: another hat replaces the witch hat", () => {
  const w = wear("auto", [], "crown", OCTOBER);
  assert.equal(w.selection, "none");
  assert.deepEqual(w.layers, ["crown"]);
  assert.equal(w.did, "replaced");
});

test("October on Auto: clicking the witch hat keeps it as the user's own", () => {
  const w = wear("auto", ["sunglasses"], "witchHat", OCTOBER);
  assert.equal(w.selection, "none");
  assert.deepEqual(w.layers, ["witchHat", "sunglasses"]);
  assert.equal(w.did, "kept");
  // Clicked again, it comes off like any layer.
  const off = wear(w.selection, w.layers, "witchHat", OCTOBER);
  assert.deepEqual(off.layers, ["sunglasses"]);
  assert.equal(off.did, "removed");
});

test("December on Auto: the Santa hat gives way, other slots stay with the outfit", () => {
  const glasses = wear("auto", [], "roundGlasses", DECEMBER);
  assert.equal(glasses.selection, "auto");
  assert.deepEqual(glasses.layers, ["roundGlasses"]);
  assert.equal(glasses.did, "added");
  const beanie = wear(glasses.selection, glasses.layers, "beanie", DECEMBER);
  assert.equal(beanie.selection, "none");
  assert.deepEqual(beanie.layers, ["roundGlasses", "beanie"]);
});

test("a costume makes way for an accessory", () => {
  const w = wear("pumpkin", [], "scarf", MAY);
  assert.equal(w.selection, "none");
  assert.deepEqual(w.layers, ["scarf"]);
  assert.equal(w.did, "costumeOff");
});

test("an item in a taken layer slot replaces the layer", () => {
  const w = wear("none", ["sunglasses", "scarf"], "roundGlasses", MAY);
  assert.deepEqual(w.layers, ["scarf", "roundGlasses"]);
  assert.equal(w.did, "replaced");
});

test("with no outfit, every layer counts (the first one is not dropped)", () => {
  assert.deepEqual(layersOn("none", ["beanie", "scarf"]), ["beanie", "scarf"]);
  assert.deepEqual(layersOn("beanie", ["crown", "scarf"]), ["scarf"]);
});
