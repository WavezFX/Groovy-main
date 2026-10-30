// "My island" (FIXES_ROUND1 item 4): the tile list is untrusted JSON, presets,
// editing, and which notifications count as mail.
import assert from "node:assert/strict";
import { keptServices } from "../src/core/tiles.ts";
import { test } from "node:test";
import {
  DEFAULT_TILES, MAX_TILES, addTile, applyPreset, isMailNotification, moveTile, parseTiles, removeTile, resizeTile, tile,
} from "../src/core/tiles.ts";
import { DEFAULT_SETTINGS } from "../src/core/state.ts";

test("nothing stored: today's developer island", () => {
  const t = parseTiles(DEFAULT_SETTINGS.islandTiles);
  assert.equal(t.preset, "developer");
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

// Round 3E item 3: one Tiles list. The user's own tiles are kept while hidden;
// Groovy's own and the services are always on offer; Remove is for good.
import { currentTiles as tilesNow, tile as makeTile, hideTile, showTile, renameTile } from "../src/core/tiles.ts";
import { entries } from "../src/settings/tilesgrid.ts";

test("hiding an app keeps it to show again; removing it forgets it", () => {
  const app = makeTile("app", "C:\\Apps\\ArtCraft Launcher.lnk");
  const cur = { ...tilesNow(structuredClone(DEFAULT_SETTINGS)), tiles: [makeTile("system"), app] };
  const hidden = hideTile(cur, app.id);
  assert.ok(!hidden.tiles.some((x) => x.id === app.id));
  assert.deepEqual(hidden.hidden.map((x) => x.id), [app.id]);
  assert.equal(hidden.preset, "custom");
  const back = showTile(hidden, app.id);
  assert.deepEqual(back.tiles.map((x) => x.id), ["system", app.id], "back at the end");
  assert.equal(back.hidden.length, 0);
  assert.equal(removeTile(hidden, app.id).hidden.length, 0, "removed for good");
  // A hidden Groovy tile isn't kept: it is always on offer.
  assert.equal(hideTile(cur, "system").hidden.length, 0);
  // Saved and read back.
  assert.deepEqual(parseTiles(JSON.parse(JSON.stringify(hidden))).hidden.map((x) => x.id), [app.id]);
  assert.equal(parseTiles({ tiles: [], hidden: [{ kind: "mail" }] }).hidden.length, 0, "only the user's own kinds are kept hidden");
});

test("a tile can be renamed, and an empty name gives its usual one back", () => {
  const app = makeTile("app", "C:\\Apps\\Code.exe");
  const cur = { ...tilesNow(structuredClone(DEFAULT_SETTINGS)), tiles: [app] };
  assert.equal(renameTile(cur, app.id, "  Editor ").tiles[0].label, "Editor");
  assert.equal(renameTile(cur, app.id, "").tiles[0].label, "");
});

test("the Tiles list shows the island's tiles in order, then everything else on offer", () => {
  const app = makeTile("app", "C:\\Apps\\Obsidian.exe");
  const cur = { ...tilesNow(structuredClone(DEFAULT_SETTINGS)), tiles: [makeTile("music"), makeTile("system")], hidden: [app] };
  const list = entries(cur);
  assert.deepEqual(list.slice(0, 3).map((e) => [e.tile.id, e.shown]), [["music", true], ["system", true], [app.id, false]]);
  assert.ok(list.some((e) => e.tile.id === "mail" && !e.shown));
  assert.ok(list.some((e) => e.tile.kind === "service" && e.tile.target === "integration_github" && !e.shown));
  assert.ok(!list.some((e) => e.tile.target === "integration_spotify"), "Spotify is the Music tile");
  assert.equal(new Set(list.map((e) => e.tile.id)).size, list.length, "each once");
});
