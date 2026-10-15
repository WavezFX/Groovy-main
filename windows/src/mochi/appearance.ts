// Groovy's own colours and layered accessories, on top of his look (skins.ts)
// and his outfit (wardrobe.ts). Pure logic, shared by the island, the desktop
// window and the Wardrobe Studio in Settings.
//
// Accessories are layered by slot — head, face, neck — with one item per slot;
// a costume (the pumpkin) is worn alone. Colours override the look's body
// gradient, glow and eyes; an empty colour keeps the look's own.
//
// Presets and wardrobe packs are plain JSON naming built-in items and colours
// only (docs/WARDROBE_PACKS.md): nothing in a pack is ever run or drawn as an
// image, so a pack can't carry code or tracking pixels.

import type { SkinStyle, Skin } from "./skins";
import { SKINS } from "./skins";
import { OUTFIT_SELECTIONS, type Outfit } from "./wardrobe";
import { N_ } from "../i18n/i18n";

export type Slot = "head" | "face" | "neck" | "costume";

export const SLOT_OF: Record<Exclude<Outfit, "none">, Slot> = {
  partyHat: "head",
  beanie: "head",
  crown: "head",
  witchHat: "head",
  santaHat: "head",
  bunnyEars: "head",
  bow: "head",
  sunglasses: "face",
  roundGlasses: "face",
  scarf: "neck",
  pumpkin: "costume",
};

export interface Appearance {
  /** Body gradient, top and bottom ("#RRGGBB"); "" keeps the look's. */
  body: string;
  body2: string;
  glow: string;
  eyes: string;
  /** Accessories worn with the outfit, at most one per slot. */
  layers: Outfit[];
}

export const DEFAULT_APPEARANCE: Appearance = { body: "", body2: "", glow: "", eyes: "", layers: [] };

export interface WardrobePreset {
  name: string;
  look: Skin;
  /** An outfit id, "none" or "auto". */
  outfit: string;
  appearance: Appearance;
}

/** What is stored in settings.json under `wardrobe`. */
export interface WardrobePrefs {
  appearance: Appearance;
  presets: WardrobePreset[];
  /** Outfit ids and look ids marked with a star. */
  favourites: string[];
}

export const DEFAULT_WARDROBE: WardrobePrefs = { appearance: DEFAULT_APPEARANCE, presets: [], favourites: [] };

const HEX = /^#[0-9a-f]{6}$/i;
const MAX_PRESETS = 50;
const MAX_NAME = 40;

export function isHex(v: unknown): v is string {
  return typeof v === "string" && HEX.test(v);
}

function hexToUnit(hex: string): [number, number, number] {
  const n = parseInt(hex.slice(1), 16);
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}

function hexToRgb255(hex: string): [number, number, number] {
  const [r, g, b] = hexToUnit(hex);
  return [Math.round(r * 255), Math.round(g * 255), Math.round(b * 255)];
}

/** A colour mixed toward white by `t` (0…1). */
function lighten(rgb: [number, number, number], t: number): [number, number, number] {
  return rgb.map((c) => Math.round(c + (255 - c) * t)) as [number, number, number];
}

/** The look's style with the user's colours applied. */
export function styleWith(base: SkinStyle, a: Appearance | null | undefined): SkinStyle {
  if (!a) return base;
  const s: SkinStyle = { ...base };
  if (isHex(a.body)) {
    s.bodyTop = hexToUnit(a.body);
    s.bodyBottom = isHex(a.body2) ? hexToUnit(a.body2) : (hexToUnit(a.body).map((c) => c * 0.86) as [number, number, number]);
  } else if (isHex(a.body2)) {
    s.bodyBottom = hexToUnit(a.body2);
  }
  if (isHex(a.glow)) {
    const [r, g, b] = hexToRgb255(a.glow);
    s.rimGlow = `rgba(${r},${g},${b},0.9)`;
  }
  if (isHex(a.eyes)) {
    const rgb = hexToRgb255(a.eyes);
    const [lr, lg, lb] = lighten(rgb, 0.45);
    const [cr, cg, cb] = lighten(rgb, 0.8);
    s.iris = `rgb(${rgb.join(",")})`;
    s.eyeInk = `rgb(${lr},${lg},${lb})`;
    s.eyeGlow = `rgba(${rgb.join(",")},0.95)`;
    s.irisCore = `rgb(${cr},${cg},${cb})`;
  }
  return s;
}

/**
 * What is actually worn: the outfit first, then each layer whose slot is still
 * free. A costume is worn alone, and nothing is worn twice.
 */
export function combine(main: Outfit, layers: readonly string[]): Outfit[] {
  const worn: Outfit[] = [];
  const taken = new Set<Slot>();
  const add = (o: string) => {
    if (o === "none" || !(o in SLOT_OF)) return;
    const slot = SLOT_OF[o as Exclude<Outfit, "none">];
    if (taken.has("costume") || taken.has(slot)) return;
    if (slot === "costume" && worn.length > 0) return;
    worn.push(o as Outfit);
    taken.add(slot);
  };
  add(main);
  for (const l of layers) add(l);
  return worn;
}

/** Whether `item` could be added on top of what is worn now. */
export function fits(worn: readonly Outfit[], item: Outfit): boolean {
  if (item === "none") return false;
  return combine(worn[0] ?? "none", [...worn.slice(1), item]).includes(item);
}

// ── Validation: settings, presets and imported packs are untrusted JSON ──────

function cleanAppearance(raw: unknown): Appearance {
  const o = (raw && typeof raw === "object" ? raw : {}) as Record<string, unknown>;
  const color = (v: unknown) => (isHex(v) ? v.toLowerCase() : "");
  const layers = Array.isArray(o.layers) ? o.layers.filter((l): l is string => typeof l === "string") : [];
  return {
    body: color(o.body),
    body2: color(o.body2),
    glow: color(o.glow),
    eyes: color(o.eyes),
    layers: combine("none", layers.slice(0, 8)),
  };
}

function cleanName(v: unknown): string {
  return typeof v === "string" ? v.replace(/[\u0000-\u001f\u007f]/g, "").trim().slice(0, MAX_NAME) : "";
}

const OUTFIT_SET: ReadonlySet<string> = new Set(OUTFIT_SELECTIONS);

export function cleanPreset(raw: unknown): WardrobePreset | null {
  if (!raw || typeof raw !== "object") return null;
  const o = raw as Record<string, unknown>;
  const name = cleanName(o.name);
  if (!name) return null;
  const look = (SKINS as readonly string[]).includes(String(o.look)) ? (o.look as Skin) : "night";
  const outfit = OUTFIT_SET.has(String(o.outfit)) ? String(o.outfit) : "none";
  return { name, look, outfit, appearance: cleanAppearance(o.appearance) };
}

export function parseWardrobe(raw: unknown): WardrobePrefs {
  const o = (raw && typeof raw === "object" ? raw : {}) as Record<string, unknown>;
  const presets = Array.isArray(o.presets) ? o.presets.map(cleanPreset).filter((p): p is WardrobePreset => p !== null) : [];
  const favourites = Array.isArray(o.favourites)
    ? [...new Set(o.favourites.filter((f): f is string => typeof f === "string" && (OUTFIT_SET.has(f) || (SKINS as readonly string[]).includes(f))))]
    : [];
  return { appearance: cleanAppearance(o.appearance), presets: presets.slice(0, MAX_PRESETS), favourites };
}

export interface PackProblem {
  /** English text, the key of its translation; `{n}` is filled from `vars`. */
  msg: string;
  vars?: Record<string, number>;
}

export interface PackResult {
  presets: WardrobePreset[];
  /** Why entries were left out, for the person importing it. */
  problems: PackProblem[];
}

/**
 * A wardrobe pack (docs/WARDROBE_PACKS.md): `{ "groovyWardrobePack": 1, "name", "presets": [...] }`.
 * At most 1 MB and 50 presets; every preset is cleaned like a stored one.
 */
export function readPack(text: string): PackResult {
  if (text.length > 1024 * 1024) return { presets: [], problems: [{ msg: N_("The file is larger than 1 MB.") }] };
  let json: unknown;
  try {
    json = JSON.parse(text);
  } catch {
    return { presets: [], problems: [{ msg: N_("The file is not valid JSON.") }] };
  }
  const o = (json && typeof json === "object" ? json : {}) as Record<string, unknown>;
  if (o.groovyWardrobePack !== 1) return { presets: [], problems: [{ msg: N_("This is not a Groovy wardrobe pack (version 1).") }] };
  const list = Array.isArray(o.presets) ? o.presets : [];
  const problems: PackProblem[] = [];
  const presets: WardrobePreset[] = [];
  list.slice(0, MAX_PRESETS).forEach((p, i) => {
    const clean = cleanPreset(p);
    if (clean) presets.push(clean);
    else problems.push({ msg: N_("Preset {n} has no name and was skipped."), vars: { n: i + 1 } });
  });
  if (list.length > MAX_PRESETS) problems.push({ msg: N_("Only the first {n} presets were read."), vars: { n: MAX_PRESETS } });
  return { presets, problems };
}

export function writePack(name: string, presets: readonly WardrobePreset[]): string {
  return JSON.stringify({ groovyWardrobePack: 1, name: cleanName(name) || "Groovy wardrobe", presets }, null, 2);
}

let parsedFrom: unknown = undefined;
let parsed: WardrobePrefs = DEFAULT_WARDROBE;

/** parseWardrobe, remembered for the same stored object (it is asked every frame). */
export function currentWardrobe(raw: unknown): WardrobePrefs {
  if (raw !== parsedFrom) {
    parsedFrom = raw;
    parsed = parseWardrobe(raw);
  }
  return parsed;
}

// ── Randomise ─────────────────────────────────────────────────────────────────

const PALETTE = ["#22c55e", "#3b82f6", "#f97316", "#ec4899", "#a855f7", "#eab308", "#14b8a6", "#ef4444", "#f5f5f5"];

/** A random look, outfit, accessories and colours; `rand` returns 0…1. */
export function randomLook(rand: () => number = Math.random): { look: Skin; outfit: Outfit; appearance: Appearance } {
  const pick = <T>(list: readonly T[]) => list[Math.floor(rand() * list.length) % list.length];
  const items = (Object.keys(SLOT_OF) as Exclude<Outfit, "none">[]).filter((o) => SLOT_OF[o] !== "costume");
  const outfit = rand() < 0.15 ? "none" : pick(items);
  const extra = [pick(items), pick(items)];
  const tinted = rand() < 0.5;
  return {
    look: pick(SKINS),
    outfit,
    appearance: {
      body: "",
      body2: "",
      glow: tinted ? pick(PALETTE) : "",
      eyes: tinted ? pick(PALETTE) : "",
      layers: combine(outfit, extra).slice(1),
    },
  };
}
