// Round 3D item 9: code split out of the startup bundle loads on first use only.
import assert from "node:assert/strict";
import { test } from "node:test";
import { once } from "../src/core/lazy.ts";

test("a page's code is imported on its first visit, then reused", async () => {
  let imports = 0;
  const page = once(async () => {
    imports++;
    return { section: () => "Brains" };
  });
  assert.equal(imports, 0, "nothing is loaded before the page opens");
  const [a, b] = await Promise.all([page(), page()]);
  assert.equal((await page()).section(), "Brains");
  assert.equal(a, b);
  assert.equal(imports, 1, "two visits, one import");
});

test("round 4F item 9: a lazy island view stands in empty, then the real one takes its place on first show", async () => {
  const { installFakeDom } = await import("./fakedom.mjs");
  installFakeDom();
  const { lazyView } = await import("../src/views/views.ts");
  let built = 0;
  const realEl = document.createElement("div");
  const view = lazyView(async () => {
    built++;
    return { el: realEl, sync() {} };
  });
  const parent = document.createElement("div");
  const standIn = view.el;
  parent.append(standIn);
  assert.equal(built, 0);
  standIn.classList.add("on");
  view.sync();
  view.sync();
  await new Promise((r) => setTimeout(r, 0));
  assert.equal(built, 1);
  assert.equal(view.el, realEl);
  assert.ok(realEl.classList.contains("on"));
  assert.equal(parent.childNodes[0], realEl);
});
