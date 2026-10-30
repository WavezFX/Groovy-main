// Round 3G item 2: each window may call only the commands its page uses.
// The capabilities are worked out from the code (scripts/command-access.mjs);
// this keeps them in step with it, and checks the lines the brief draws.
// (src-tauri/tests/acl.rs checks that Tauri really refuses the rest.)

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { access, files, registeredCommands } from "../scripts/command-access.mjs";

test("the capability files are the ones the code calls for (run scripts/command-access.mjs)", () => {
  for (const [path, text] of Object.entries(files())) assert.equal(readFileSync(path, "utf8").replaceAll("\r\n", "\n"), text, path);
});

test("shell and Studio commands only in the Studio; keys, accounts and permissions only in Settings", () => {
  const a = access();
  const studioOnly = registeredCommands().filter((c) => c.startsWith("studio_") && c !== "studio_ai_target");
  for (const window of ["island", "settings", "mochi"]) {
    for (const c of studioOnly) assert.ok(!a[window].includes(c), `${window} may call ${c}`);
  }
  const settingsOnly = [
    // (mail_connect_oauth stays in the island too: its mail panel's "Connect" opens the browser's sign-in.)
    "local_set_key", "tools_set_permission", "mail_connect_imap", "mail_disconnect", "mail_oauth_configure",
    "models_delete", "skills_install", "skills_remove",
  ];
  for (const c of settingsOnly) assert.ok(registeredCommands().includes(c), `${c} is a command`);
  for (const window of ["island", "mochi", "studio"]) {
    for (const c of settingsOnly) assert.ok(!a[window].includes(c), `${window} may call ${c}`);
  }
  // Each window keeps what it really uses.
  assert.ok(a.studio.includes("studio_run") && a.settings.includes("local_set_key") && a.island.includes("tools_decide"));
  assert.ok(a.mochi.length < 20, "the desktop Groovy needs few");
});
