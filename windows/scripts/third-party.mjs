// Writes THIRD_PARTY_NOTICES.md: every crate in the Windows release build and every
// shipped npm package, with its licence (round 3J item 1). Run from windows/:
//   node scripts/third-party.mjs
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync, writeFileSync, existsSync } from "node:fs";

const meta = JSON.parse(execFileSync("cargo", ["metadata", "--format-version", "1", "--filter-platform", "x86_64-pc-windows-msvc"], { maxBuffer: 1 << 28 }).toString());
const used = new Set(meta.resolve.nodes.map((n) => n.id));
const own = new Set(meta.workspace_members);
const crates = meta.packages
  .filter((p) => used.has(p.id) && !own.has(p.id))
  .map((p) => [p.name, p.version, p.license ?? "see the crate", p.repository ?? ""])
  .sort();

const npm = [];
const pkg = JSON.parse(readFileSync("package.json", "utf8"));
for (const name of Object.keys(pkg.dependencies ?? {})) {
  const f = `node_modules/${name}/package.json`;
  if (!existsSync(f)) continue;
  const p = JSON.parse(readFileSync(f, "utf8"));
  npm.push([name, p.version, typeof p.license === "string" ? p.license : "see the package", p.repository?.url ?? p.repository ?? ""]);
}

const row = ([n, v, l, r]) => `| ${n} | ${v} | ${l} | ${r} |`;
const head = "| Name | Version | Licence | Source |\n|---|---|---|---|";
writeFileSync(
  "THIRD_PARTY_NOTICES.md",
  `# Third-party notices\n\nGroovy is released under the Apache License 2.0. It is built with the open-source packages below, each under its own licence. Models Groovy downloads are listed in docs/licenses/MODELS.md.\n\n## Rust crates (${crates.length})\n\n${head}\n${crates.map(row).join("\n")}\n\n## npm packages (${npm.length})\n\n${head}\n${npm.map(row).join("\n")}\n`,
);
console.log(`${crates.length} crates, ${npm.length} npm packages`);
