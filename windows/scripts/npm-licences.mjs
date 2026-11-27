// Round 4E item 5: the npm licence gate. Every package that ships (production
// dependencies, all levels) must be under MIT, BSD-2-Clause, BSD-3-Clause, ISC
// or Apache-2.0, or one of the few exceptions below, each with its reason.
// "A OR B" passes when one side passes; "A AND B" when both do. Run from windows/:
//   node scripts/npm-licences.mjs
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";

export const ALLOWED = new Set(["MIT", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Apache-2.0"]);
/** Exceptions, each already present and justified. */
export const EXCEPTIONS = {
  "0BSD": "tslib (through rxjs, livekit-client): public-domain-like BSD with no conditions",
};

export function passes(expr) {
  const s = String(expr ?? "").trim().replace(/^\((.*)\)$/, "$1");
  if (!s) return false;
  if (/\bOR\b/.test(s)) return s.split(/\bOR\b/).some((p) => passes(p));
  if (/\bAND\b/.test(s)) return s.split(/\bAND\b/).every((p) => passes(p));
  const id = s.replace(/[()]/g, "").trim();
  return ALLOWED.has(id) || id in EXCEPTIONS;
}

if (import.meta.url === `file://${process.argv[1]}` || process.argv[1]?.endsWith("npm-licences.mjs")) {
  const out = execFileSync("npm", ["ls", "--omit=dev", "--all", "--parseable"], { encoding: "utf8", shell: process.platform === "win32" });
  const bad = [];
  let n = 0;
  for (const dir of new Set(out.split(/\r?\n/).slice(1).filter(Boolean))) {
    const p = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
    if (p.private) continue;
    n++;
    const lic = typeof p.license === "string" ? p.license : Array.isArray(p.licenses) ? p.licenses.map((l) => l.type ?? l).join(" OR ") : p.license?.type;
    if (!passes(lic)) bad.push(`${p.name}@${p.version}: ${lic ?? "no licence"}`);
  }
  if (bad.length) {
    console.error(`Not allowed (only ${[...ALLOWED].join(", ")}; exceptions ${Object.keys(EXCEPTIONS).join(", ")}):\n  ${bad.join("\n  ")}`);
    process.exit(1);
  }
  console.log(`${n} shipped npm packages, all allowed`);
}
