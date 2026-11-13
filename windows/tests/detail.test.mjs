// A tile's detail card (FIXES_ROUND3 §8 A and D): the old Coucou island view
// (big character card, 2×2 pill grid) is gone; a tile opens one glass card
// with ← Home, and a service without its key asks for it right there.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { sent } from "./tauri.mjs";
import { installFakeDom } from "./fakedom.mjs";
import { State } from "../src/core/state.ts";
import { VIEW_LAYOUTS, botPosition } from "../src/core/layout.ts";
import { renderIntegrationCard } from "../src/views/integrations.ts";
import { face } from "../src/views/tiles.ts";
import { tile } from "../src/core/tiles.ts";

installFakeDom();

const tick = () => new Promise((r) => setTimeout(r, 0));

test("the detail card has no Groovy and no pill grid", () => {
  assert.equal(VIEW_LAYOUTS.overview.agentMode, "none");
  assert.equal(botPosition("expanded", "overview", VIEW_LAYOUTS.overview.height).opacity, 0);
  const src = readFileSync(new URL("../src/views/views.ts", import.meta.url), "utf8");
  assert.ok(!src.includes('class: "pills"'), "the old pill grid is not built");
  assert.ok(!src.includes("createMiniBot"), "no mini Groovys in the detail card");
  assert.ok(src.includes("detail-back"), "the card has its ← Home button");
});

test("a service without its key shows the field, Save and where to get one", async () => {
  State.loadIntegrationTasks();
  const task = State.tasks.find((t) => t.id === "integration_github");
  State.integrations[task.id] = { loaded: true, error: null, configured: false, data: {} };
  const card = renderIntegrationCard(task, { detailOpen: false, openDetail() {}, closeDetail() {}, openSettings() {} });
  assert.ok(card.textContent.includes("Get a key ↗"));
  assert.ok(!card.textContent.includes("Settings…"), "one tap, not a trip to the settings");
  const input = card.querySelector("int-key-input");
  const save = card.querySelector("int-key-save");
  assert.equal(input.getAttribute("type"), "password");

  input.value = "   ";
  save.fire("click");
  await tick();
  assert.equal(sent("secret_set").length, 0, "nothing typed, nothing saved");

  input.value = "  ghp_example  ";
  save.fire("click");
  await tick();
  await tick();
  assert.deepEqual(sent("secret_set").at(-1), { key: "github-token", value: "ghp_example" });
  assert.deepEqual(sent("refresh_integration").at(-1), { id: "integration_github" });
  assert.equal(input.value, "", "the key does not stay in the field");
});

test("the Agents tile opens the agent that is asking first", () => {
  State.loadIntegrationTasks();
  const views = [];
  const claude = State.tasks.find((t) => t.id === "integration_claude");
  claude.pillBadge = "approval";
  State.focusId = "integration_github";
  face(tile("agents", ""), (v) => views.push(v)).open();
  assert.equal(State.focusId, "integration_claude");
  assert.deepEqual(views, ["overview"]);
  claude.pillBadge = undefined;
});
