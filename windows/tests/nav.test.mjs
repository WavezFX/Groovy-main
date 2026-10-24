// The Settings window's map (FIXES_ROUND3D item 1): eight sections, merged
// pages with tabs, old page ids that still land, and the search box.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { ALIASES, ALL_PAGES, HIDDEN, SECTIONS, TABS, parentOf, resolvePage, search } from "../src/hub/nav.ts";

const root = fileURLToPath(new URL("..", import.meta.url));

test("the approved sidebar: 15 pages in 8 groups, every page once", () => {
  assert.deepEqual(SECTIONS.map((s) => s.id), ["home", "groovy", "ai", "voice", "island", "everyday", "computer", "app"]);
  assert.deepEqual(SECTIONS.flatMap((s) => s.pages.map((p) => p.title)), [
    "Home", "Character & wardrobe", "Brains", "Memory", "Skills", "Speaking", "Listening & calls", "Dictation",
    "Layout & tiles", "Accounts", "Reminders & breaks", "Browser & agents", "Permissions", "General", "System",
  ]);
  assert.equal(SECTIONS[0].title, "", "Home has no group label");
  const ids = ALL_PAGES.map((p) => p.id);
  assert.equal(new Set(ids).size, ids.length, "no page twice");
  assert.deepEqual(TABS.brains.map((t) => t.id), ["cloud", "subscriptions", "local"]);
  // Every page that is no longer in the sidebar is a tab of one that is.
  const sidebar = new Set(SECTIONS.flatMap((s) => s.pages.map((p) => p.id)));
  for (const p of HIDDEN) if (p.id !== "speechsetup") assert.ok(parentOf(p.id) && sidebar.has(parentOf(p.id)), `${p.id} is a tab`);
  // Dictation's other tabs are Flow pages (hub/pages.ts draws them).
  const renderers = readFileSync(join(root, "src/hub/pages.ts"), "utf8");
  for (const tab of TABS.dictation.filter((t) => !["notes", "insights"].includes(t.id))) assert.match(renderers, new RegExp(`\\b${tab.id}: `), tab.id);
});

test("every page id the code opens still lands somewhere, old ones included", () => {
  for (const [old, to] of Object.entries(ALIASES)) assert.ok(ALL_PAGES.some((p) => p.id === to.page), old);
  assert.deepEqual(resolvePage("models"), { page: "brains", tab: "local" });
  assert.deepEqual(resolvePage("assistant"), { page: "brains" });
  assert.deepEqual(resolvePage("voice"), { page: "listening", tab: "calls" });
  // A page that became a tab still opens, under its sidebar page.
  assert.deepEqual(resolvePage("calls"), { page: "listening", tab: "calls" });
  assert.deepEqual(resolvePage("sounds"), { page: "speaking", tab: "sounds" });
  assert.deepEqual(resolvePage("about"), { page: "general", tab: "about" });
  assert.equal(resolvePage("nonsense"), null);
  assert.equal(resolvePage("__proto__"), null);
  // Every page named in the code: openSettingsWindow("…"), navigate("…"), Rust's show_hub(…, Some("…")).
  const named = new Set();
  const walk = (dir) => {
    for (const name of readdirSync(dir)) {
      if (["node_modules", "target", "dist"].includes(name)) continue;
      const path = join(dir, name);
      if (statSync(path).isDirectory()) walk(path);
      else if (/\.(ts|rs)$/.test(name)) {
        const text = readFileSync(path, "utf8");
        for (const m of text.matchAll(/(?:openSettingsWindow|navigate)\("([a-z]+)"/g)) named.add(m[1]);
        for (const m of text.matchAll(/show_hub\([^,]+,\s*Some\("([a-z]+)"\)/g)) named.add(m[1]);
      }
    }
  };
  walk(join(root, "src"));
  walk(join(root, "src-tauri/src"));
  assert.ok(named.size > 5);
  for (const id of named) assert.ok(resolvePage(id), `"${id}" opens a page`);
});

test("search finds settings by name or keyword and says where", () => {
  const r = search("spotify");
  assert.ok(r.some((x) => x.place.page === "island" && x.place.tab === "music"));
  assert.equal(search("ollama")[0].place.page, "brains");
  assert.equal(search("ollama")[0].place.tab, "local");
  assert.ok(search("wake word").some((x) => x.place.page === "listening"));
  assert.deepEqual(search("   "), []);
  assert.deepEqual(search("zzzzqq"), []);
});
