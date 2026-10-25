// Groovy's colours, his Head/Face/Neck items, presets and wardrobe packs
// (src/mochi/appearance.ts, FIXES_ROUND3 item 5).

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  DEFAULT_APPEARANCE, SLOTS, SLOT_OF, clearSlot, currentWardrobe, migrateOutfit, migrateSettingsOutfit,
  parseWardrobe, randomLook, readPack, styleWith, tryOn, wear, worn, writePack,
} from "../src/mochi/appearance.ts";
import { SKIN_STYLES } from "../src/mochi/skins.ts";
import { ACCESSORIES, available } from "../src/mochi/wardrobe.ts";

test("every item has one of three slots: head, face, neck", () => {
  assert.deepEqual([...SLOTS], ["head", "face", "neck"]);
  for (const o of ACCESSORIES) assert.ok(SLOTS.includes(SLOT_OF[o]), o);
  assert.ok(!ACCESSORIES.includes("pumpkin"), "the costume is gone");
});

test("one item per slot, head first, the first one named wins", () => {
  assert.deepEqual(worn(["leafScarf", "shades", "bucketHat", "acornCap", "nope"]), ["bucketHat", "shades", "leafScarf"]);
  assert.deepEqual(worn([]), []);
});

test("choosing an item replaces only its own slot", () => {
  // Bucket hat + shades, then an acorn cap: the shades stay.
  let layers = wear([], "bucketHat").layers;
  layers = wear(layers, "shades").layers;
  assert.deepEqual(layers, ["bucketHat", "shades"]);
  const cap = wear(layers, "acornCap");
  assert.deepEqual(cap.layers, ["acornCap", "shades"]);
  assert.equal(cap.did, "replaced");
  // Clicking what is worn takes it off.
  assert.deepEqual(wear(cap.layers, "acornCap"), { layers: ["shades"], did: "removed" });
  // Seasonal items take their slot too: two for the head, two for the neck.
  assert.equal(SLOT_OF.pumpkinCap, "head");
  assert.equal(SLOT_OF.littleGhostPal, "neck");
  assert.equal(SLOT_OF.poolRing, "neck");
  assert.deepEqual(wear(["bucketHat", "bandana"], "pumpkinCap").layers, ["pumpkinCap", "bandana"]);
});

test("each slot's None empties that slot only; Remove all empties everything", () => {
  const layers = ["bucketHat", "shades", "leafScarf"];
  assert.deepEqual(clearSlot(layers, "face"), ["bucketHat", "leafScarf"]);
  assert.deepEqual(clearSlot(layers, "head"), ["shades", "leafScarf"]);
  assert.deepEqual(clearSlot(layers, "all"), []);
});

test("hovering tries an item on in its slot, the rest as worn", () => {
  assert.deepEqual(tryOn(["bucketHat", "shades"], "roundSpecs"), ["bucketHat", "roundSpecs"]);
  assert.deepEqual(tryOn(["bucketHat"], null), ["bucketHat"]);
  assert.deepEqual(tryOn(["bucketHat"], "none"), ["bucketHat"]);
});

test("a saved Mochi outfit becomes its nearest new item, or nothing", () => {
  // The obvious ones: beanie → bucket hat, bow → bow tie, glasses → specs and shades, scarf → leaf scarf.
  assert.deepEqual(migrateOutfit("beanie", ["roundGlasses"]), ["bucketHat", "roundSpecs"]);
  assert.deepEqual(migrateOutfit("bow", ["sunglasses", "scarf"]), ["shades", "bowTie"]);
  // Everything else is nothing.
  for (const old of ["witchHat", "santaHat", "bunnyEars", "partyHat", "crown", "auto", "pumpkin"]) assert.deepEqual(migrateOutfit(old, []), [], old);
  const settings = { mochiOutfit: "santaHat", wardrobe: { appearance: { layers: ["crown", "sunglasses"] }, favourites: ["crown", "beanie"] } };
  assert.equal(migrateSettingsOutfit(settings), true);
  assert.equal(settings.mochiOutfit, "none");
  assert.deepEqual(settings.wardrobe.appearance.layers, ["shades"]);
  assert.deepEqual(settings.wardrobe.favourites, ["bucketHat"], "favourites move with their item");
  assert.equal(migrateSettingsOutfit(settings), false, "once only");
  // Saved layers are moved on every read too, even without the old single outfit.
  assert.deepEqual(parseWardrobe({ appearance: { layers: ["bow", "witchHat"] } }).appearance.layers, ["bowTie"]);
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
    appearance: { body: "red", glow: "#ABCDEF", eyes: "#12345", layers: ["antenna", "bucketHat", "<script>"] },
    presets: [{ name: "  Party\u0007 ", look: "ember", outfit: "mushroomCap", appearance: {} }, { name: "", look: "x" }, "junk"],
    favourites: ["antenna", "antenna", "night", "evil"],
    autoSeason: false,
    dressedFor: "halloween:2026",
  });
  assert.deepEqual(w.appearance, { body: "", body2: "", glow: "#abcdef", eyes: "", layers: ["antenna"] });
  assert.equal(w.presets.length, 1);
  assert.equal(w.presets[0].name, "Party");
  assert.equal(w.presets[0].look, "ember");
  // A preset's outfit is now a layer of its own.
  assert.equal(w.presets[0].outfit, "none");
  assert.deepEqual(w.presets[0].appearance.layers, ["mushroomCap"]);
  assert.deepEqual(w.favourites, ["antenna", "night"]);
  assert.equal(w.autoSeason, false);
  assert.equal(w.dressedFor, "halloween:2026");
  assert.equal(parseWardrobe({}).autoSeason, true, "dressing for the season is on unless turned off");
  assert.deepEqual(parseWardrobe(null).appearance, DEFAULT_APPEARANCE);
  assert.deepEqual(parseWardrobe(5).presets, []);
  // Asked every frame: the same stored object is parsed once.
  const raw = { favourites: ["antenna"] };
  assert.equal(currentWardrobe(raw), currentWardrobe(raw));
});

test("packs round-trip, and anything else is refused with a reason", () => {
  const presets = [{ name: "Cosy", look: "frost", outfit: "none", appearance: { ...DEFAULT_APPEARANCE, layers: ["bucketHat", "leafScarf"] } }];
  const back = readPack(writePack("Mine", presets));
  assert.deepEqual(back.presets, presets);
  assert.deepEqual(back.problems, []);
  // An older pack's outfit joins the layers.
  const old = readPack(JSON.stringify({ groovyWardrobePack: 1, presets: [{ name: "Old", look: "night", outfit: "beanie", appearance: { layers: ["scarf"] } }] }));
  assert.deepEqual(old.presets[0].appearance.layers, ["bucketHat", "leafScarf"]);
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
    assert.deepEqual(worn(r.appearance.layers), r.appearance.layers);
  }
});

test("a random look never picks an item he can't wear yet", () => {
  let seed = 7;
  const rand = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
  const can = (item) => available(item, 0, []);
  for (let i = 0; i < 200; i++) {
    for (const item of randomLook(rand, can).appearance.layers) assert.ok(can(item), item);
  }
});
