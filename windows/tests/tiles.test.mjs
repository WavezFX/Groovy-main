// "My island" (FIXES_ROUND1 item 4): the tile list is untrusted JSON, presets,
// editing, and which notifications count as mail.
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  DEFAULT_TILES, MAX_TILES, addTile, applyPreset, isMailNotification, moveTile, parseTiles, removeTile, resizeTile, tile, tilesHome,
} from "../src/core/tiles.ts";
import { DEFAULT_SETTINGS } from "../src/core/state.ts";

test("nothing stored: today's developer island", () => {
  const t = parseTiles(DEFAULT_SETTINGS.islandTiles);
  assert.equal(t.preset, "developer");
  assert.equal(tilesHome(t), false, "the agents overview stays the home");
  assert.deepEqual(t, parseTiles(undefined));
  assert.deepEqual(DEFAULT_TILES.mailApps, []);
});

test("malformed tiles are dropped, good ones kept in order", () => {
  const t = parseTiles({
    preset: "custom",
    tiles: [
      { kind: "site", target: "javascript:alert(1)" },
      { kind: "site", target: "https://mail.google.com", label: "Gmail", size: "expanded" },
      { kind: "nope", target: "" },
      { kind: "mail", target: "x" },
      { kind: "mail" },
      { kind: "mail" },
      { kind: "service", target: "integration_github" },
      "garbage",
    ],
    mailApps: ["Outlook", 3, ""],
  });
  assert.deepEqual(t.tiles.map((x) => x.id), ["site:https://mail.google.com", "mail", "service:integration_github"]);
  assert.equal(t.tiles[0].size, "expanded");
  assert.deepEqual(t.mailApps, ["Outlook"]);
  assert.equal(tilesHome(t), true);
});

test("presets, adding, moving, resizing and removing", () => {
  let t = applyPreset(DEFAULT_TILES, "everyday");
  assert.equal(t.tiles[0].kind, "mail");
  t = addTile(t, tile("site", "https://example.com"));
  assert.equal(t.preset, "custom");
  assert.equal(addTile(t, tile("site", "https://example.com")), null, "no duplicates");
  assert.equal(addTile(t, tile("site", "ftp://x")), null, "a website is http(s)");
  const last = t.tiles.at(-1).id;
  t = moveTile(t, last, 0);
  assert.equal(t.tiles[0].id, last);
  t = resizeTile(t, last, "expanded");
  assert.equal(t.tiles[0].size, "expanded");
  t = removeTile(t, last);
  assert.ok(!t.tiles.some((x) => x.id === last));
  let full = applyPreset(DEFAULT_TILES, "custom");
  for (let i = 0; full.tiles.length < MAX_TILES; i++) full = addTile(full, tile("site", `https://s${i}.example`)) ?? full;
  assert.equal(addTile(full, tile("site", "https://one-more.example")), null, "a limit");
});

test("which notifications are mail", () => {
  assert.ok(isMailNotification("Outlook", []));
  assert.ok(isMailNotification("Mail", []));
  assert.ok(!isMailNotification("Google Chrome", []));
  assert.ok(isMailNotification("Google Chrome", ["chrome"]), "a browser for Gmail, when chosen");
  assert.ok(!isMailNotification("Outlook", ["chrome"]), "only the chosen apps");
});
