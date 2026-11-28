// Round 7 (FIXES_ROUND7.md): Instagram's DMs on the island. The store keeps
// the newest five in memory, the tile says who wrote and what, the tile kind
// survives the untrusted tile list, and Instagram is off by default.
import assert from "node:assert/strict";
import { test } from "node:test";
import { DEFAULT_INSTAGRAM, Instagram, instagramFace } from "../src/instagram/store.ts";
import { DEFAULT_SETTINGS } from "../src/core/state.ts";
import { addTile, parseTiles, tile } from "../src/core/tiles.ts";

const dm = (id, sender, preview = "hi") => ({ id, sender, username: null, preview, source: "web", appId: "Chrome" });
const same = (s) => s;

test("the newest DM first, five kept, the same one once", () => {
  Instagram.clear();
  for (let i = 1; i <= 7; i++) Instagram.push(dm(i, `p${i}`));
  Instagram.push(dm(7, "p7", "edited"));
  assert.deepEqual(Instagram.dms.map((d) => d.id), [7, 6, 5, 4, 3]);
  assert.equal(Instagram.dms[0].preview, "edited");
  Instagram.clear();
  assert.equal(Instagram.dms.length, 0);
});

test("the tile: sender and preview, or how to start", () => {
  assert.deepEqual(instagramFace([], false, same), { title: "Instagram", sub: "Turn on Instagram in Settings" });
  assert.deepEqual(instagramFace([], true, same), { title: "Instagram", sub: "New messages pop up here" });
  assert.deepEqual(instagramFace([dm(1, "sam.k", "on my way")], true, same), { title: "sam.k", sub: "on my way" });
  assert.deepEqual(instagramFace([dm(1, "sam.k", "")], true, same), { title: "sam.k", sub: "Sent you a message" });
});

test("off by default; the tile is one of a kind and survives the stored list", () => {
  assert.deepEqual(DEFAULT_SETTINGS.instagram, DEFAULT_INSTAGRAM);
  assert.equal(DEFAULT_SETTINGS.instagram.on, false);
  const added = addTile(parseTiles({}), tile("instagram"));
  assert.ok(added);
  assert.equal(addTile(added, tile("instagram")), null, "only one Instagram tile");
  const back = parseTiles(JSON.parse(JSON.stringify(added)));
  assert.ok(back.tiles.some((x) => x.kind === "instagram" && x.id === "instagram"));
  assert.equal(parseTiles({ tiles: [{ kind: "instagram", target: "x" }] }).tiles.some((x) => x.kind === "instagram"), false, "no target for it");
});
