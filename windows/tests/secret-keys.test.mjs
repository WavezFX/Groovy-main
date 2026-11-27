// Every key a Settings page saves must be in the Rust store's KNOWN_KEYS, or
// secret_set refuses it ("unknown key") and the key is never kept. The Fish
// Audio key was missing for weeks (owner, 22 Nov).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const rs = readFileSync(new URL("../src-tauri/src/secrets.rs", import.meta.url), "utf8");
const known = new Set([...rs.split("pub const KNOWN_KEYS")[1].split("];")[0].matchAll(/"([a-z0-9-]+)"/g)].map((m) => m[1]));

function files(dir) {
  return readdirSync(dir).flatMap((f) => {
    const p = join(dir, f);
    return statSync(p).isDirectory() ? files(p) : p.endsWith(".ts") ? [p] : [];
  });
}

test("every key the pages save is one the store accepts", () => {
  const saved = new Set();
  for (const f of files(fileURLToPath(new URL("../src", import.meta.url)))) {
    const s = readFileSync(f, "utf8");
    // keyBox("…"), secretField("…"), secretSet("…"), and constants passed to them.
    for (const m of s.matchAll(/(?:keyBox|secretField|secretSet|secretPresent)\(\s*"([a-z0-9-]+)"/g)) saved.add(m[1]);
    for (const m of s.matchAll(/const [A-Z_]+ = "([a-z0-9]+(?:-[a-z0-9]+)*-(?:api-key|api-secret|key|token))";/g)) saved.add(m[1]);
  }
  assert.ok(saved.has("fish-audio-api-key"), "the scan finds the Fish Audio key");
  const missing = [...saved].filter((k) => !known.has(k));
  assert.deepEqual(missing, [], `not in secrets.rs KNOWN_KEYS: ${missing.join(", ")}`);
});
