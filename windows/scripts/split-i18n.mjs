// Round 3F item 6: one translation file per language.
//
// strings.json (from the Mac's catalog) and extra.json (written by hand) hold
// every language side by side, which put all ten into every window (1.2 MB).
// This splits them into src/i18n/lang/<code>.json, each a { English text:
// translation } map, so a window bundles English only and fetches the chosen
// language's file when it is needed (src/i18n/i18n.ts `loadLanguage`). The
// files are generated (git-ignored): `npm run dev`, `build` and `test` make
// them first. The Rust side still embeds the two source files as they are.
//
//   node scripts/split-i18n.mjs [out dir]

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const dir = join(root, "src", "i18n");
const out = process.argv[2] ? resolve(process.argv[2]) : join(dir, "lang");

/** The table the app reads: extra.json wins over the Mac's where both have a string. */
export function mergedTable() {
  const read = (name) => JSON.parse(readFileSync(join(dir, name), "utf8")).strings;
  return { ...read("strings.json"), ...read("extra.json") };
}

export function splitByLanguage(table) {
  const langs = {};
  for (const [key, entry] of Object.entries(table)) {
    for (const [lang, value] of Object.entries(entry)) {
      (langs[lang] ??= {})[key] = value;
    }
  }
  return langs;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const langs = splitByLanguage(mergedTable());
  mkdirSync(out, { recursive: true });
  for (const [lang, strings] of Object.entries(langs)) {
    writeFileSync(join(out, `${lang}.json`), JSON.stringify(strings));
  }
  console.log(`i18n: ${Object.keys(langs).length} language files in ${out}`);
}
