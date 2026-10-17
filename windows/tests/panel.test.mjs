// Tiles open in the island (FIXES_ROUND3C item 4): Reminders, This PC, Mail and
// Spotify unfold into a panel there instead of opening Settings.
import assert from "node:assert/strict";
import { test } from "node:test";
import { installFakeDom } from "./fakedom.mjs";
import "./tauri.mjs";

installFakeDom();
const { face } = await import("../src/views/tiles.ts");
const { buildPanel, bytes, localInput, speed, SYSTEM_EVERY_MS } = await import("../src/views/panel.ts");
const { State } = await import("../src/core/state.ts");
const { VIEW_LAYOUTS } = await import("../src/core/layout.ts");

test("Reminders, This PC, Mail and Spotify tiles open their panel in the island", () => {
  for (const [kind, panel] of [["reminders", "reminders"], ["system", "system"], ["mail", "mail"], ["music", "spotify"]]) {
    const seen = [];
    face({ kind, target: "", label: "" }, (v) => seen.push(v)).open();
    assert.deepEqual(seen, ["panel"], kind);
    assert.equal(State.panel, panel);
  }
  assert.ok(VIEW_LAYOUTS.panel.height >= VIEW_LAYOUTS.home.height, "the panel covers the tiles");
});

test("the panel has a back arrow and All settings, and This PC reads only while open", async () => {
  const seen = [];
  const host = buildPanel({ setView: (v) => seen.push(v) });
  State.mode = "expanded";
  State.view = "panel";
  State.panel = "system";
  host.sync();
  assert.ok(host.el.querySelector(".panel-back"));
  assert.ok(host.el.querySelector(".panel-deep"));
  assert.equal(host.el.dataset.panel, "system");
  host.el.querySelector(".panel-back").fire("click");
  assert.deepEqual(seen, ["home"]);
  State.view = "home";
  host.sync();
  assert.equal(SYSTEM_EVERY_MS, 2000);
});

test("network speed comes from two readings; sizes and picker values read well", () => {
  const a = { netReceived: 1000, netSent: 0 };
  const b = { netReceived: 1000 + 2 * 1024 * 1024, netSent: 4096 };
  assert.deepEqual(speed(a, b, 2000), { down: 1024 * 1024, up: 2048 });
  assert.equal(speed(null, b, 2000), null);
  assert.equal(bytes(3 * 1024 ** 3), "3.0 GB");
  assert.equal(bytes(5 * 1024 ** 2), "5 MB");
  assert.match(localInput(0 + new Date(2026, 9, 17, 9, 5).getTime() / 1000), /^2026-10-17T09:05$/);
});
