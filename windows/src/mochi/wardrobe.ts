// Groovy's wardrobe (round 5 items 3 and 4, round 3E item 7): the 28 items of
// docs/design/wardrobe-v2/Wardrobe.dc.html, the Diamond crown for six months,
// and the 16 seasonal items of Seasonal.dc.html, one per slot (Head, Face,
// Neck). What he wears is `wardrobe.appearance.layers` (./appearance.ts); the
// drawing lives in ./outfits.ts. Some items wait for a stage
// (STAGE_LOCK); seasonal ones are earned in their season (./seasons.ts).
//
// The Mochi outfits of earlier versions are gone: a saved one becomes its
// nearest new item (OLD_IDS) or nothing.

import { N_ } from "../i18n/i18n";

export const HEAD_ITEMS = [
  "leafCrown", "acornCap", "mushroomCap", "flowerCrown", "bucketHat", "backwardsCap",
  "headphones", "pilotGoggles", "beretPin", "miniTopHat", "cloudBuddy", "antenna", "diamondCrown",
] as const;
export const FACE_ITEMS = [
  "roundSpecs", "shades", "heartGlasses", "glowVisor", "monocle", "frecklesBlush", "starStickers", "piratePatch",
] as const;
export const NECK_ITEMS = [
  "bandana", "bowTie", "leafScarf", "seedPendant", "neckPhones", "goldChain", "flowerLei", "necktie",
] as const;
/** By season: two for the head, then two for the neck. */
export const SEASONAL_ITEMS = [
  "pumpkinCap", "batWingBand", "candyCornScarf", "littleGhostPal",
  "bobbleBeanie", "twigAntlers", "knitStripeScarf", "jingleBellCollar",
  "bunnyEarBand", "eggshellHat", "pastelRibbonBow", "daisyChain",
  "strawSunHat", "watermelonCap", "shellNecklace", "poolRing",
] as const;

/** Every item, in the wardrobe's order: head, face, neck, then the seasonal ones. */
export const ACCESSORIES = [...HEAD_ITEMS, ...FACE_ITEMS, ...NECK_ITEMS, ...SEASONAL_ITEMS] as const;

export type Accessory = (typeof ACCESSORIES)[number];
export type SeasonalItem = (typeof SEASONAL_ITEMS)[number];

/** An item, or nothing. */
export type Outfit = Accessory | "none";

export const DEFAULT_OUTFIT: Outfit = "none";

/** Items that wait for a stage: they unlock once it is reached, for good. */
export const STAGE_LOCK: Partial<Record<Accessory, number>> = {
  seedPendant: 1,
  flowerCrown: 2,
  miniTopHat: 3,
  cloudBuddy: 4,
  goldChain: 5,
  diamondCrown: 6,
};

/** Mochi outfits of earlier versions → their nearest new item (anything else: nothing). */
export const OLD_IDS: Readonly<Record<string, Accessory>> = {
  beanie: "bucketHat",
  bow: "bowTie",
  sunglasses: "shades",
  roundGlasses: "roundSpecs",
  scarf: "leafScarf",
};

// ── User-visible strings ──────────────────────────────────────────────────────

/** The English names, the keys of their translations (src/i18n). */
export const OUTFIT_KEYS: Record<Outfit, string> = {
  none: N_("None"),
  leafCrown: N_("Leaf crown"),
  acornCap: N_("Acorn cap"),
  mushroomCap: N_("Mushroom cap"),
  flowerCrown: N_("Flower crown"),
  bucketHat: N_("Bucket hat"),
  backwardsCap: N_("Backwards cap"),
  headphones: N_("Headphones"),
  pilotGoggles: N_("Pilot goggles"),
  beretPin: N_("Beret & pin"),
  miniTopHat: N_("Mini top hat"),
  cloudBuddy: N_("Cloud buddy"),
  antenna: N_("Antenna"),
  diamondCrown: N_("Diamond crown"),
  roundSpecs: N_("Round specs"),
  shades: N_("Shades"),
  heartGlasses: N_("Heart glasses"),
  glowVisor: N_("Glow visor"),
  monocle: N_("Monocle"),
  frecklesBlush: N_("Freckles & blush"),
  starStickers: N_("Star stickers"),
  piratePatch: N_("Pirate patch"),
  bandana: N_("Bandana"),
  bowTie: N_("Bow tie"),
  leafScarf: N_("Leaf scarf"),
  seedPendant: N_("Seed pendant"),
  neckPhones: N_("Neck phones"),
  goldChain: N_("Gold chain"),
  flowerLei: N_("Flower lei"),
  necktie: N_("Necktie"),
  pumpkinCap: N_("Pumpkin cap"),
  batWingBand: N_("Bat-wing band"),
  candyCornScarf: N_("Candy-corn scarf"),
  littleGhostPal: N_("Little ghost pal"),
  bobbleBeanie: N_("Bobble beanie"),
  twigAntlers: N_("Twig antlers"),
  knitStripeScarf: N_("Knit stripe scarf"),
  jingleBellCollar: N_("Jingle-bell collar"),
  bunnyEarBand: N_("Bunny-ear band"),
  eggshellHat: N_("Eggshell hat"),
  pastelRibbonBow: N_("Pastel ribbon bow"),
  daisyChain: N_("Daisy chain"),
  strawSunHat: N_("Straw sun hat"),
  watermelonCap: N_("Watermelon cap"),
  shellNecklace: N_("Shell necklace"),
  poolRing: N_("Pool ring"),
};

// ── Logic ─────────────────────────────────────────────────────────────────────

const ACCESSORY_SET: ReadonlySet<string> = new Set(ACCESSORIES);
const SEASONAL_SET: ReadonlySet<string> = new Set(SEASONAL_ITEMS);

export function isAccessory(v: unknown): v is Accessory {
  return typeof v === "string" && ACCESSORY_SET.has(v);
}

export function isSeasonal(v: unknown): v is SeasonalItem {
  return typeof v === "string" && SEASONAL_SET.has(v);
}

/**
 * A stored outfit → what it stands for now: an item, a Mochi outfit's
 * nearest new item, or "none" for anything else ("auto", a removed or
 * unknown value).
 */
export function parseOutfit(raw: unknown): Outfit {
  if (isAccessory(raw)) return raw;
  return (typeof raw === "string" && Object.hasOwn(OLD_IDS, raw) && OLD_IDS[raw]) || "none";
}

/** Whether an item can be worn: its stage is reached and, if seasonal, it was earned. */
export function available(item: Accessory, reached: number, earned: readonly string[]): boolean {
  if (isSeasonal(item)) return earned.includes(item);
  return reached >= (STAGE_LOCK[item] ?? 0);
}
