// The Settings page template (FIXES_ROUND3D item 2): danger asks first, keys
// stay masked, statuses look the same everywhere, every control has a label.

import { test, mock } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { installFakeDom } from "./fakedom.mjs";

installFakeDom();
const ui = await import("../src/hub/ui.ts");

test("a danger button asks first, then acts on the second click, and forgets after a while", () => {
  mock.timers.enable({ apis: ["setTimeout"] });
  try {
    let done = 0;
    const b = ui.dangerBtn("Forget", "Sure?", () => done++);
    assert.ok(b.className.includes("danger"));
    b.onclick();
    assert.equal(done, 0);
    assert.equal(b.textContent, "Sure?");
    b.onclick();
    assert.equal(done, 1);
    assert.equal(b.textContent, "Forget");
    b.onclick();
    mock.timers.tick(4000);
    assert.equal(b.textContent, "Forget");
    b.onclick();
    assert.equal(done, 1, "a click after the ask ran out only asks again");
  } finally {
    mock.timers.reset();
  }
});

test("a secret field starts masked, never shows the stored value, and show/hide toggles", () => {
  const f = ui.secretField({ label: "API key", placeholder: "sk-…", present: true, save: async () => {} });
  const input = f.querySelector(".hub-t-input");
  const kind = () => input.type ?? input.getAttribute("type");
  assert.equal(kind(), "password");
  assert.ok(!input.value, "the stored key never comes back");
  assert.equal(input.getAttribute("aria-label"), "API key");
  const eye = f.find("BUTTON")[0];
  eye.onclick();
  assert.equal(kind(), "text");
  eye.onclick();
  assert.equal(kind(), "password");
});

test("status pills share one class per state, with progress while downloading", () => {
  for (const s of ["connected", "unset", "error", "downloading"]) assert.ok(ui.statusPill(s).className.includes(s));
  const p = ui.statusPill("downloading", undefined, 0.42);
  assert.match(p.textContent, /42%/);
});

test("a setting row labels its control for screen readers, and rows are at least 56 px", () => {
  const control = document.createElement("button");
  const row = ui.setting("Launch at startup", "Opens with Windows.", control);
  assert.ok(row.className.includes("hub-t-row"));
  assert.equal(control.getAttribute("aria-label"), "Launch at startup");
  const css = readFileSync(new URL("../src/hub/hub.css", import.meta.url), "utf8");
  assert.match(css, /\.hub-t-row[^{]*\{[^}]*min-height:\s*56px/);
  assert.match(css, /\.hub-page[^{]*\{[^}]*max-width:\s*760px/);
  assert.match(css, /data-theme="light"/, "a light theme exists");
});

test("an empty state is never blank: icon, sentence, action", () => {
  const e = ui.emptyState("check", "Nothing yet.", ui.btn("Add", () => {}, "primary"));
  assert.ok(e.querySelector(".hub-t-empty-icon"));
  assert.equal(e.find("P")[0].textContent, "Nothing yet.");
  assert.equal(e.find("BUTTON").length, 1);
});
