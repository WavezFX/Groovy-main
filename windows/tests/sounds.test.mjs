// Groovy's own sounds (round 3 item 7, checked again for round 4 item 6):
// every event has a sound made in code, in the three flavours of his looks,
// and no audio file ships — none of Coucou's, nor any other.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { MAX_SECONDS, RECIPES, SAMPLE_RATE, renderSound } from "../src/core/sounds-synth.ts";
import { SOUND_NAMES } from "../src/core/sound.ts";

// A real path on every system (a URL's pathname is "/D:/…" on Windows).
const root = fileURLToPath(new URL("..", import.meta.url));

test("every sound event is made in code, in wood, warm and glass", () => {
  for (const name of SOUND_NAMES) {
    assert.ok(RECIPES[name], `${name} has a recipe`);
    for (const flavor of ["wood", "warm", "glass"]) {
      const s = renderSound(name, flavor);
      assert.ok(s && s.length > 0, `${name}/${flavor}`);
      assert.ok(s.length <= MAX_SECONDS * SAMPLE_RATE, `${name}/${flavor} is short`);
      let peak = 0;
      for (const v of s) {
        assert.ok(Number.isFinite(v));
        peak = Math.max(peak, Math.abs(v));
      }
      assert.ok(peak > 0.05 && peak <= 0.81, `${name}/${flavor} peak ${peak}`);
    }
  }
});

test("Night, Ember and Frost don't sound the same", () => {
  const wood = renderSound("finish", "wood");
  const glass = renderSound("finish", "glass");
  assert.ok(wood.length !== glass.length || wood.some((v, i) => Math.abs(v - glass[i]) > 1e-3));
});

test("no audio file ships: only the speech tests' recordings are in the repository", () => {
  const found = [];
  const walk = (dir) => {
    for (const name of readdirSync(dir)) {
      if (["node_modules", "target", "dist", ".git", "release"].includes(name)) continue;
      const path = join(dir, name);
      if (statSync(path).isDirectory()) walk(path);
      else if (/\.(wav|mp3|ogg|m4a|aiff?|caf|flac|opus)$/i.test(name)) found.push(relative(root, path).split(sep).join("/"));
    }
  };
  walk(root);
  assert.deepEqual(found.filter((f) => !f.startsWith("src-tauri/tests/audio/")), []);
  const conf = JSON.parse(readFileSync(join(root, "src-tauri/tauri.conf.json"), "utf8"));
  assert.ok(!JSON.stringify(conf.bundle?.resources ?? {}).match(/sound|\.wav|\.mp3/i), "the installer carries no sounds");
});
