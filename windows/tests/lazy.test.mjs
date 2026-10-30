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
