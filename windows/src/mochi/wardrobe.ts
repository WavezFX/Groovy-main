// Groovy's wardrobe — the accessories he can wear, one per slot (FIXES_ROUND3
// item 5: Head, Face and Neck, each on its own; no more outfit, no "Auto"
// seasons, no pumpkin costume). What he wears is `wardrobe.appearance.layers`
// (./appearance.ts); the drawing lives in ./outfits.ts.
//
// `mochiOutfit`, the single outfit of earlier versions (the Mac's raw values),
// is only read once more to move it into its slot (migrateOutfit).

import { N_, labels } from "../i18n/i18n";

/** Every accessory, in the wardrobe's order: head, then face, then neck. */
export const ACCESSORIES = [
  "partyHat", "beanie", "crown", "witchHat", "santaHat", "bunnyEars", "bow",
  "sunglasses", "roundGlasses",
  "scarf",
] as const;

export type Accessory = (typeof ACCESSORIES)[number];

/** An accessory, or nothing. */
export type Outfit = Accessory | "none";

/** What the island's wardrobe buttons and the old setting can name. */
export const OUTFIT_SELECTIONS: readonly Outfit[] = ["none", ...ACCESSORIES];
export type OutfitSelection = Outfit;

export const DEFAULT_OUTFIT: Outfit = "none";

// ── User-visible strings ──────────────────────────────────────────────────────

/** The English names, the keys of their translations (src/i18n). */
export const OUTFIT_KEYS: Record<Outfit, string> = {
  none: N_("None"),
  partyHat: N_("Party hat"),
  beanie: N_("Beanie"),
  crown: N_("Crown"),
  sunglasses: N_("Sunglasses"),
  roundGlasses: N_("Round glasses"),
  bow: N_("Bow"),
  scarf: N_("Scarf"),
  witchHat: N_("Witch hat"),
  santaHat: N_("Santa hat"),
  bunnyEars: N_("Bunny ears"),
};

/** In the current language: every read goes through `t()`. */
export const OUTFIT_LABELS: Record<Outfit, string> = labels(OUTFIT_KEYS);

// ── Logic ─────────────────────────────────────────────────────────────────────

const ACCESSORY_SET: ReadonlySet<string> = new Set(ACCESSORIES);

export function isAccessory(v: unknown): v is Accessory {
  return typeof v === "string" && ACCESSORY_SET.has(v);
}

/**
 * A stored outfit → what it stands for now: an accessory, or "none" for
 * anything else ("auto", the pumpkin, a removed or unknown value).
 */
export function parseOutfit(raw: unknown): Outfit {
  return isAccessory(raw) ? raw : "none";
}
