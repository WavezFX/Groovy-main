// The translations, one file per language (round 3F item 6). English is the
// key itself, so it needs no file; every other language is fetched with a
// dynamic import when the window first shows it, and only once. The files are
// made from strings.json and extra.json by scripts/split-i18n.mjs.

import type { Language } from "./i18n";

export type PluralForms = Partial<Record<Intl.LDMLPluralRule, string>>;
export type LanguageTable = Record<string, string | PluralForms>;

export const LOADERS: Record<Exclude<Language, "en">, () => Promise<{ default: unknown }>> = {
  "zh-Hans": () => import("./lang/zh-Hans.json"),
  hi: () => import("./lang/hi.json"),
  es: () => import("./lang/es.json"),
  ar: () => import("./lang/ar.json"),
  fr: () => import("./lang/fr.json"),
  bn: () => import("./lang/bn.json"),
  "pt-BR": () => import("./lang/pt-BR.json"),
  ru: () => import("./lang/ru.json"),
  id: () => import("./lang/id.json"),
};
