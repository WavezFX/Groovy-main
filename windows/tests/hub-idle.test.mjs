// Settings left open (round 4A item 3): what counts as "busy" (src/hub/busy.ts).

import { test } from "node:test";
import assert from "node:assert/strict";
import { hubBusy } from "../src/hub/busy.ts";

const doc = (activeElement, found = null) => ({ activeElement, querySelector: () => found });

test("an idle Settings window is not busy", () => {
  assert.equal(hubBusy(doc(null)), false);
  assert.equal(hubBusy(doc({ tagName: "BODY" })), false);
  assert.equal(hubBusy(doc({ tagName: "INPUT", value: "" })), false, "an empty field");
  assert.equal(hubBusy(doc({ tagName: "BUTTON", value: "x" })), false, "a button isn't typing");
});

test("words in the field with the caret, or anything in progress, keep it open", () => {
  assert.equal(hubBusy(doc({ tagName: "INPUT", value: "sk-..." })), true);
  assert.equal(hubBusy(doc({ tagName: "TEXTAREA", value: "a note" })), true);
  assert.equal(hubBusy(doc({ tagName: "DIV", isContentEditable: true, textContent: "draft" })), true);
  assert.equal(hubBusy(doc(null, {})), true, "a progress bar on the page");
});
