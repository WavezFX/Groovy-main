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

// Round 4E item 3: the MIT credit for the code Groovy started from (copyright line and
// licence text, which the MIT licence requires), until docs/plan/REWRITE_LEDGER.md is empty.
const STARTED_FROM = `## Code Groovy started from\n\nGroovy's code began from an MIT-licensed project. Files still holding its code are listed in docs/plan/REWRITE_LEDGER.md; this section goes once that list has no inherited file left.\n\n\`\`\`\nMIT License\n\nCopyright (c) 2026 Louis Raillé\n\nPermission is hereby granted, free of charge, to any person obtaining a copy\nof this software and associated documentation files (the "Software"), to deal\nin the Software without restriction, including without limitation the rights\nto use, copy, modify, merge, publish, distribute, sublicense, and/or sell\ncopies of the Software, and to permit persons to whom the Software is\nfurnished to do so, subject to the following conditions:\n\nThe above copyright notice and this permission notice shall be included in all\ncopies or substantial portions of the Software.\n\nTHE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR\nIMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,\nFITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE\nAUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER\nLIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,\nOUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE\nSOFTWARE.\n\`\`\`\n\n`;

const row = ([n, v, l, r]) => `| ${n} | ${v} | ${l} | ${r} |`;
const head = "| Name | Version | Licence | Source |\n|---|---|---|---|";
writeFileSync(
  "THIRD_PARTY_NOTICES.md",
  `# Third-party notices\n\nGroovy is released under the Apache License 2.0. It is built with the open-source packages below, each under its own licence. Models Groovy downloads are listed in docs/licenses/MODELS.md.\n\n${STARTED_FROM}## Rust crates (${crates.length})\n\n${head}\n${crates.map(row).join("\n")}\n\n## npm packages (${npm.length})\n\n${head}\n${npm.map(row).join("\n")}\n`,
);
console.log(`${crates.length} crates, ${npm.length} npm packages`);
