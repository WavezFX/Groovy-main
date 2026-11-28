// Round 6 (FIXES_ROUND6.md): the Discord tile — untrusted state from
// discord.rs, the compact face, and the tile kind on the island.
import assert from "node:assert/strict";
import { test } from "node:test";
import { EMPTY_DISCORD, discordFace, parseDiscord, speakers } from "../src/core/discord.ts";
import { parseTiles, tile } from "../src/core/tiles.ts";
import { expandsTo } from "../src/core/home.ts";

const ready = (more = {}) => parseDiscord({
  status: "ready", setUp: true, authorised: true, mute: false, deaf: false,
  channel: { id: "c", name: "General", guildId: "g" },
  members: [{ id: "1", name: "Me", me: true, speaking: true }, { id: "2", name: "Sam", speaking: true }, { id: "3", name: "Alex" }],
  notes: [],
  ...more,
});

test("malformed state from Rust becomes the empty state", () => {
  assert.deepEqual(parseDiscord(null), EMPTY_DISCORD);
  assert.deepEqual(parseDiscord({ members: "x", notes: [1, null], channel: 5 }).members, []);
  const s = parseDiscord({ members: [{ id: "1", name: "a".repeat(200), speaking: "yes" }] });
  assert.equal(s.members[0].name.length, 60);
  assert.equal(s.members[0].speaking, false);
});

test("the compact face: speaking, muted, deafened", () => {
  assert.deepEqual(speakers(ready()), ["Sam"], "the user isn't listed as a speaker");
  assert.equal(discordFace(ready()).title, "🔊 General");
  assert.match(discordFace(ready()).sub, /Sam/);
  assert.match(discordFace(ready({ mute: true })).sub, /Muted/);
  assert.match(discordFace(ready({ mute: true, deaf: true })).sub, /Deafened/);
  assert.match(discordFace(ready({ channel: null, members: [] })).sub, /Not in a call/);
});

test("the face says what's missing: set up, connect, Discord closed", () => {
  assert.match(discordFace(parseDiscord({ status: "not-set-up" })).sub, /Settings → Discord/);
  assert.match(discordFace(parseDiscord({ status: "needs-auth", setUp: true })).sub, /Connect/);
  assert.match(discordFace(parseDiscord({ status: "not-running", setUp: true, authorised: true })).sub, /isn't open/);
  // A message shows even when Discord's voice can't be reached.
  const n = parseDiscord({ status: "not-running", setUp: true, authorised: true, notes: [{ id: 1, title: "Sam", body: "hi" }] });
  assert.equal(discordFace(n).sub, "Sam: hi");
});

test("the Discord tile is one of a kind, kept by parseTiles, and grows", () => {
  const t = parseTiles({ preset: "custom", tiles: [{ kind: "discord", target: "" }, { kind: "discord", target: "" }, { kind: "discord", target: "x" }] });
  assert.deepEqual(t.tiles.map((x) => x.id), ["discord"]);
  assert.equal(expandsTo(tile("discord")), "discord");
});

test("Discord's notifications aren't mail, and notes are capped at five", async () => {
  const { isMailNotification } = await import("../src/core/tiles.ts");
  assert.equal(isMailNotification("Discord", []), false);
  const notes = Array.from({ length: 9 }, (_, i) => ({ id: i, title: "Sam", body: "hi" }));
  assert.equal(parseDiscord({ notes }).notes.length, 5);
});
