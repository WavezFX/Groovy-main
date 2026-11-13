// Round 3G item 2: each window may call only the commands it uses. This works
// out, from the code itself, which of Groovy's commands each window's page can
// reach (its entry's import graph, dynamic imports included, and the Bridge
// wrappers its files call), and writes one capability per window into
// src-tauri/capabilities/. build.rs lists every registered command in the app
// manifest, so a command no capability allows is refused everywhere.
//   node scripts/command-access.mjs          write the capabilities
//   node scripts/command-access.mjs --check  fail if they are out of date
// The test in tests/capabilities.test.mjs runs the check.

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
// Line endings don't count: a Windows checkout has CRLF.
const read = (p) => readFileSync(join(ROOT, p), "utf8").replaceAll("\r\n", "\n");

/** Window label → its page's entry. */
export const WINDOWS = {
  island: "src/main.ts",
  settings: "src/settings/main.ts",
  mochi: "src/desktop/main.ts",
  studio: "src/studio/main.ts",
};

/** What every window keeps from Tauri itself and its plugins, as before. */
const BASE = ["core:default", "autostart:default"];

/** Every command Rust registers (lib.rs, generate_handler!). */
export function registeredCommands() {
  const lib = read("src-tauri/src/lib.rs");
  const at = lib.indexOf("generate_handler![");
  const block = lib.slice(at + "generate_handler![".length, lib.indexOf("]", at));
  return [...new Set(block.split(",").map((s) => s.trim().split("::").pop()).filter((s) => /^[a-z0-9_]+$/.test(s ?? "")))];
}

/** The objects that wrap commands: their member → the commands its body calls. */
const WRAPPERS = { "src/core/bridge.ts": "Bridge", "src/core/flow.ts": "FlowBridge", "src/core/skills.ts": "SkillsBridge" };

function members(file, name, commands) {
  const src = read(file);
  const start = src.indexOf(`export const ${name} = {`);
  if (start < 0) throw new Error(`${file}: no ${name}`);
  // The object ends at the first line that is just "};" after it.
  const end = src.indexOf("\n};", start);
  const body = src.slice(start, end);
  const map = new Map();
  let current = null;
  for (const line of body.split("\n").slice(1)) {
    const m = /^ {2}(?:async\s+)?([A-Za-z_]\w*)\s*[:(<]/.exec(line);
    if (m) current = m[1];
    if (!current) continue;
    for (const [, c] of line.matchAll(/"([a-z0-9_]+)"/g)) if (commands.has(c)) (map.get(current) ?? map.set(current, new Set()).get(current)).add(c);
  }
  // What the rest of the file calls (outside the object) counts for every
  // importer: only names passed to call/invoke, not the lists of names kept
  // there (which commands report errors, which are quiet).
  const rest = src.slice(0, start) + src.slice(end);
  const loose = new Set([...rest.matchAll(/\b(?:call|callOrThrow|invoke)(?:<[^>()]*>)?\(\s*"([a-z0-9_]+)"/g)].map((m) => m[1]).filter((c) => commands.has(c)));
  return { map, loose };
}

function resolveImport(from, spec) {
  if (!spec.startsWith(".") && !spec.startsWith("/src/")) return null;
  const base = spec.startsWith("/src/") ? join(ROOT, spec.slice(1)) : resolve(dirname(join(ROOT, from)), spec);
  for (const p of [base, `${base}.ts`, join(base, "index.ts")]) {
    if (p.endsWith(".ts") && existsSync(p)) return p.slice(ROOT.length + 1).replaceAll("\\", "/");
  }
  return null;
}

/** Every source file a window's page can load, from its entry. */
export function reachable(entry) {
  const seen = new Set();
  const todo = [entry];
  while (todo.length) {
    const f = todo.pop();
    if (seen.has(f)) continue;
    seen.add(f);
    const src = read(f);
    for (const m of src.matchAll(/(?:import|export)\s[^;]*?from\s+["']([^"']+)["']|import\(\s*["']([^"']+)["']\s*\)|import\s+["']([^"']+)["']/g)) {
      const next = resolveImport(f, m[1] ?? m[2] ?? m[3]);
      if (next) todo.push(next);
    }
  }
  return seen;
}

/** Window → the commands its page uses, sorted. */
export function access() {
  const commands = new Set(registeredCommands());
  const wrappers = Object.fromEntries(Object.entries(WRAPPERS).map(([f, name]) => [f, { name, ...members(f, name, commands) }]));
  const out = {};
  for (const [label, entry] of Object.entries(WINDOWS)) {
    const used = new Set();
    for (const f of reachable(entry)) {
      const src = read(f);
      const w = wrappers[f];
      if (w) {
        for (const c of w.loose) used.add(c);
        continue;
      }
      for (const [, c] of src.matchAll(/"([a-z0-9_]+)"/g)) if (commands.has(c)) used.add(c);
      for (const { name, map } of Object.values(wrappers)) {
        for (const [, member] of src.matchAll(new RegExp(`\\b${name}\\.(\\w+)`, "g"))) for (const c of map.get(member) ?? []) used.add(c);
      }
    }
    out[label] = [...used].sort();
  }
  return out;
}

function capability(label, cmds) {
  return {
    $schema: "../gen/schemas/desktop-schema.json",
    identifier: label,
    description: `What the ${label} window may do: Tauri's defaults and the ${cmds.length} Groovy commands its page uses (generated by scripts/command-access.mjs, round 3G item 2).`,
    windows: [label],
    permissions: [...BASE, ...cmds.map((c) => `allow-${c.replaceAll("_", "-")}`)],
  };
}

export function files() {
  const a = access();
  return Object.fromEntries(Object.entries(a).map(([label, cmds]) => [`src-tauri/capabilities/${label}.json`, `${JSON.stringify(capability(label, cmds), null, 2)}\n`]));
}

if (process.argv[1] && fileURLToPath(import.meta.url) === resolve(process.argv[1])) {
  const check = process.argv.includes("--check");
  let stale = 0;
  for (const [path, text] of Object.entries(files())) {
    const now = existsSync(join(ROOT, path)) ? read(path) : "";
    if (now === text) continue;
    if (check) {
      console.error(`${path} is out of date: run node scripts/command-access.mjs`);
      stale++;
    } else writeFileSync(join(ROOT, path), text);
  }
  if (stale) process.exit(1);
  const a = access();
  for (const [label, cmds] of Object.entries(a)) console.log(`${label}: ${cmds.length} commands`);
}
