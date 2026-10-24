// "My island" (FIXES_ROUND1 item 4): the tile list is untrusted JSON, presets,
// editing, and which notifications count as mail.
import assert from "node:assert/strict";
import { keptServices } from "../src/core/tiles.ts";
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

// ── One tile system (FIXES_ROUND3 item 4) ─────────────────────────────────────

import { servicesOf, tileService, withServices } from "../src/core/tiles.ts";

test("every service turned on has its tile, in the user's order; Spotify is Music", () => {
  const base = { preset: "custom", tiles: [tile("mail"), tile("service", "integration_github")], mailApps: [] };
  const on = ["integration_github", "integration_notion", "integration_spotify", "integration_calcom"];
  const t = withServices(base, on, "integration_claude");
  assert.deepEqual(t.tiles.map((x) => x.id), [
    "mail", "service:integration_github", "service:integration_claude", "service:integration_notion", "music", "service:integration_calcom",
  ]);
  // Nothing to add: the very same object comes back (no needless save).
  assert.equal(withServices(t, on, "integration_claude"), t);
  assert.equal(tileService(tile("music")), "integration_spotify");
});

test("a service turned off loses its tile; removing a tile turns its service off", () => {
  const t = withServices({ preset: "custom", tiles: [], mailApps: [] }, ["integration_github", "integration_spotify"], "integration_claude");
  assert.deepEqual(withServices(t, ["integration_spotify"], "integration_claude").tiles.map((x) => x.id), ["service:integration_claude", "music"]);
  const without = removeTile(t, "music");
  assert.deepEqual(servicesOf(without, "integration_claude"), ["integration_github"]);
  // The main tool is never in the list of services: it is always on.
  assert.ok(!servicesOf(t, "integration_claude").includes("integration_claude"));
});

test("no small limit on tiles any more", () => {
  assert.ok(MAX_TILES >= 40);
  let t = { preset: "custom", tiles: [], mailApps: [] };
  for (let i = 0; i < 20; i++) t = addTile(t, tile("site", `https://example.com/${i}`)) ?? t;
  assert.equal(t.tiles.length, 20);
});

// Round 3B item 7: a preset or the mail apps used to switch every integration off.
test("only removing a service's tile turns it off: a preset or the mail apps never do", () => {
  const on = ["integration_github", "integration_vercel"];
  // A preset whose tiles have no services keeps the ones that were on.
  assert.deepEqual(keptServices([], on), on);
  // Tile order wins for the services with a tile; the others follow.
  assert.deepEqual(keptServices(["integration_vercel"], on), ["integration_vercel", "integration_github"]);
  // Removing GitHub's tile turns GitHub off, nothing else.
  assert.deepEqual(keptServices(["integration_vercel"], on, "integration_github"), ["integration_vercel"]);
  // A new service tile turns it on.
  assert.deepEqual(keptServices(["integration_neon"], on), ["integration_neon", ...on]);
});

// Round 3D item 3: Island → Layout & tiles shows or hides a tile with one tap.
import { toggled } from "../src/settings/islandpage.ts";
import { currentTiles as tilesNow, tile as makeTile } from "../src/core/tiles.ts";

test("a tile on the Layout & tiles page turns on and off, its service with it", () => {
  const settings = structuredClone(DEFAULT_SETTINGS);
  const before = tilesNow(settings);
  const github = { tile: makeTile("service", "integration_github"), name: "GitHub", color: "#fff" };
  const on = toggled(settings, before, github, true);
  assert.ok(on.tiles.tiles.some((x) => tileService(x) === "integration_github"));
  assert.ok(on.active.includes("integration_github"));
  assert.equal(on.tiles.preset, "custom");
  const off = toggled({ ...settings, islandTiles: on.tiles, activeIntegrations: on.active }, on.tiles, github, false);
  assert.ok(!off.tiles.tiles.some((x) => tileService(x) === "integration_github"));
  assert.ok(!off.active.includes("integration_github"));
});
