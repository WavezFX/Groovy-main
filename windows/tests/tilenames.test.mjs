// Round 3E item 1: app tiles show what people call the app, on one line.

import { test } from "node:test";
import assert from "node:assert/strict";
import { shortAppName } from "../src/core/tiles.ts";

test("app names lose launcher words, shortcuts, bitness and versions", () => {
  assert.equal(shortAppName("Visual Studio Code"), "VS Code");
  assert.equal(shortAppName("ArtCraft Launcher"), "ArtCraft");
  assert.equal(shortAppName("Proton VPN"), "Proton VPN");
  assert.equal(shortAppName("Steam - Shortcut"), "Steam");
  assert.equal(shortAppName("7-Zip (x64)"), "7-Zip");
  assert.equal(shortAppName("Blender 4.2"), "Blender");
  assert.equal(shortAppName("Mozilla Firefox"), "Firefox");
  assert.equal(shortAppName("Launcher"), "Launcher", "never empty");
});
