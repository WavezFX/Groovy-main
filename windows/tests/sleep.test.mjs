// The island sleeps at idle (src/island/sleep.ts, src-tauri/src/island_sleep.rs).

import { afterEach, beforeEach, mock, test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { IslandSleep, LONGEST_WAIT_MS, QUIET_MS, canSleep } from "../src/island/sleep.ts";

const resting = { expanded: false, moving: false, talking: false, asking: false, strip: false, away: false };

beforeEach(() => mock.timers.enable({ apis: ["setTimeout"] }));
afterEach(() => mock.timers.reset());

const settle = () => new Promise((r) => setImmediate(r));

test("he may sleep only when nothing at all is going on", () => {
  assert.equal(canSleep(resting), true);
  for (const key of Object.keys(resting)) {
    assert.equal(canSleep({ ...resting, [key]: true }), false, key);
  }
});

test("the island asks to sleep after the quiet time, once", async () => {
  let asked = 0;
  const sleep = new IslandSleep(() => resting, async () => { asked += 1; return true; });
  sleep.rest();
  sleep.rest();
  mock.timers.tick(QUIET_MS - 1);
  assert.equal(asked, 0);
  mock.timers.tick(1);
  await settle();
  assert.equal(asked, 1);
  mock.timers.tick(QUIET_MS * 10);
  await settle();
  assert.equal(asked, 1, "asleep: nothing is asked again until he rests again");
});

test("anything happening stops the count", async () => {
  let asked = 0;
  const sleep = new IslandSleep(() => resting, async () => { asked += 1; return true; });
  sleep.rest();
  mock.timers.tick(QUIET_MS / 2);
  sleep.stir();
  mock.timers.tick(QUIET_MS * 2);
  await settle();
  assert.equal(asked, 0);
});

test("a busy island doesn't ask, and no timer is left running", async () => {
  let asked = 0;
  const sleep = new IslandSleep(() => ({ ...resting, talking: true }), async () => { asked += 1; return true; });
  sleep.rest();
  mock.timers.tick(QUIET_MS);
  await settle();
  mock.timers.tick(LONGEST_WAIT_MS * 2);
  await settle();
  assert.equal(asked, 0);
});

test("told 'not now', it asks again less and less often", async () => {
  const at = [];
  let now = 0;
  const sleep = new IslandSleep(() => resting, async () => { at.push(now); return false; });
  sleep.rest();
  for (let i = 0; i < 40; i++) {
    now += QUIET_MS;
    mock.timers.tick(QUIET_MS);
    await settle();
  }
  assert.deepEqual(at.slice(0, 4), [QUIET_MS, QUIET_MS * 3, QUIET_MS * 7, QUIET_MS * 15]);
  const gaps = at.slice(1).map((t, i) => t - at[i]);
  assert.ok(Math.max(...gaps) <= LONGEST_WAIT_MS, "never rarer than the longest wait");
});

test("a failed call to Rust counts as 'not now'", async () => {
  let asked = 0;
  const sleep = new IslandSleep(() => resting, async () => { asked += 1; throw new Error("no backend"); });
  sleep.rest();
  mock.timers.tick(QUIET_MS);
  await settle();
  mock.timers.tick(QUIET_MS * 2);
  await settle();
  assert.equal(asked, 2);
});

// Rust keeps the events meant for a sleeping page (island_sleep.rs). An event
// the island listens to that Rust doesn't know about would be lost while he sleeps.
test("every event the island's page listens to is watched by the sleeping island", () => {
  const rust = readFileSync(new URL("../src-tauri/src/island_sleep.rs", import.meta.url), "utf8");
  const list = rust.slice(rust.indexOf("pub const ISLAND_EVENTS"), rust.indexOf("];", rust.indexOf("pub const ISLAND_EVENTS")));
  const known = new Set([...list.matchAll(/"([a-z0-9:_-]+)"/g)].map((m) => m[1]));
  const root = fileURLToPath(new URL("../src", import.meta.url));
  // The Settings window, the Coding Studio and the desktop Groovy are other pages.
  const others = ["hub", "settings", "studio", "desktop"];
  const files = [];
  const walk = (dir) => {
    for (const name of readdirSync(dir)) {
      const path = join(dir, name);
      if (statSync(path).isDirectory()) {
        if (!(dir === root && others.includes(name))) walk(path);
      } else if (name.endsWith(".ts")) files.push(path);
    }
  };
  walk(root);
  const heard = new Set();
  for (const file of files) {
    for (const m of readFileSync(file, "utf8").matchAll(/onEvent(?:<[^>]*>)?\(\s*"([a-z0-9:_-]+)"/g)) heard.add(m[1]);
  }
  heard.delete("island-woke");
  const missing = [...heard].filter((name) => !known.has(name));
  assert.deepEqual(missing, [], "add these to ISLAND_EVENTS in island_sleep.rs");
  assert.ok(heard.size > 20, "the page's listeners were found");
});
