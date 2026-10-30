// "My island" (FIXES_ROUND1 item 4): the tiles the user puts on the island —
// their apps, websites and folders, the services they follow, their mail,
// music, agents, reminders and the PC's state. Stored in settings.json under
// `islandTiles`, which is untrusted JSON: read it only through parseTiles.

import type { Settings } from "./state";

/** Spotify's pill: on the island it is the Music tile. */
const SPOTIFY = "integration_spotify";

export type TileKind = "app" | "site" | "folder" | "service" | "mail" | "music" | "agents" | "reminders" | "system";
export type TileSize = "compact" | "expanded";
export type IslandPreset = "developer" | "everyday" | "custom";

export interface Tile {
  /** Stable within the list: "<kind>:<target>" or "<kind>" for one-of-a-kind tiles. */
  id: string;
  kind: TileKind;
  /** What it opens: an app's shortcut path, a URL, a folder, a pill id. Empty for the others. */
  target: string;
  label: string;
  size: TileSize;
}

export interface IslandTiles {
  preset: IslandPreset;
  tiles: Tile[];
  /**
   * The user's own tiles (apps, websites, folders) switched off in Settings
   * but kept, so tapping them brings them back (round 3E item 3). Groovy's
   * own tiles and the services need no keeping: they are always on offer.
   */
  hidden: Tile[];
  /** Apps whose notifications may pop up on the island as mail ("Outlook", "Mail", "Chrome"…). Empty: any mail-looking app. */
  mailApps: string[];
}

/** Generous: the island pages its tiles (round 3), this only stops a runaway file. */
export const MAX_TILES = 60;
const KINDS: readonly TileKind[] = ["app", "site", "folder", "service", "mail", "music", "agents", "reminders", "system"];
const SINGLE: readonly TileKind[] = ["mail", "music", "agents", "reminders", "system"];

export function tile(kind: TileKind, target = "", label = "", size: TileSize = "compact"): Tile {
  return { id: SINGLE.includes(kind) ? kind : `${kind}:${target}`, kind, target, label, size };
}

export const PRESETS: Record<Exclude<IslandPreset, "custom">, () => Tile[]> = {
  /** Today's island: the agents and their hooks first. */
  developer: () => [tile("agents", "", "", "expanded"), tile("music"), tile("system")],
  /** Mail, calendar-ish reminders, music and the user's apps. */
  everyday: () => [tile("mail", "", "", "expanded"), tile("music"), tile("reminders"), tile("system")],
};

export const DEFAULT_TILES: IslandTiles = { preset: "developer", tiles: PRESETS.developer(), hidden: [], mailApps: [] };

/** The kinds the user adds themselves; only these can be removed for good. */
export const OWN_KINDS: readonly TileKind[] = ["app", "site", "folder"];

function cleanText(v: unknown, max: number): string {
  return typeof v === "string" ? v.replace(/[\u0000-\u001f\u007f]/g, "").trim().slice(0, max) : "";
}

/** A tile's target is only taken when it fits its kind. */
export function validTarget(kind: TileKind, target: string): boolean {
  switch (kind) {
    case "site":
      return /^https?:\/\/[^\s]+$/i.test(target);
    case "app":
    case "folder":
      return target.length > 0 && !target.includes("\u0000");
    case "service":
      return /^[a-z0-9_-]+$/i.test(target);
    default:
      return target === "";
  }
}

/** settings.islandTiles → what the island shows; anything malformed is dropped. */
export function parseTiles(raw: unknown): IslandTiles {
  const o = (raw && typeof raw === "object" ? raw : {}) as Record<string, unknown>;
  const preset: IslandPreset = o.preset === "everyday" || o.preset === "custom" || o.preset === "developer" ? o.preset : DEFAULT_TILES.preset;
  const seen = new Set<string>();
  const list = (raw: unknown, only?: readonly TileKind[]): Tile[] => {
    const out: Tile[] = [];
    if (!Array.isArray(raw)) return out;
    for (const t of raw) {
      if (!t || typeof t !== "object") continue;
      const r = t as Record<string, unknown>;
      const kind = KINDS.find((k) => k === r.kind);
      if (!kind || (only && !only.includes(kind))) continue;
      const target = cleanText(r.target, 1000);
      if (!validTarget(kind, target)) continue;
      const one = tile(kind, target, cleanText(r.label, 40), r.size === "expanded" ? "expanded" : "compact");
      if (seen.has(one.id)) continue;
      seen.add(one.id);
      out.push(one);
      if (out.length >= MAX_TILES) break;
    }
    return out;
  };
  const tiles = list(o.tiles);
  const hidden = list(o.hidden, OWN_KINDS);
  const mailApps = Array.isArray(o.mailApps) ? o.mailApps.map((a) => cleanText(a, 60)).filter(Boolean).slice(0, 20) : [];
  if (!Array.isArray(o.tiles)) return { ...DEFAULT_TILES, preset, tiles: preset === "everyday" ? PRESETS.everyday() : PRESETS.developer(), hidden, mailApps };
  return { preset, tiles, hidden, mailApps };
}

export function currentTiles(settings: Pick<Settings, "islandTiles">): IslandTiles {
  return parseTiles(settings.islandTiles);
}

/** Picking a preset: its tiles; "custom" keeps the current ones. */
export function applyPreset(t: IslandTiles, preset: IslandPreset): IslandTiles {
  if (preset === "custom") return { ...t, preset };
  return { ...t, preset, tiles: PRESETS[preset]() };
}

/** Adds a tile (the list becomes "custom"); a tile already there or past the limit is refused. */
export function addTile(t: IslandTiles, add: Tile): IslandTiles | null {
  if (t.tiles.length >= MAX_TILES || t.tiles.some((x) => x.id === add.id) || !validTarget(add.kind, add.target)) return null;
  return { ...t, preset: "custom", tiles: [...t.tiles, add] };
}

export function removeTile(t: IslandTiles, id: string): IslandTiles {
  return { ...t, preset: "custom", tiles: t.tiles.filter((x) => x.id !== id), hidden: (t.hidden ?? []).filter((x) => x.id !== id) };
}

/** Takes a tile off the island; the user's own are kept to show again. */
export function hideTile(t: IslandTiles, id: string): IslandTiles {
  const it = t.tiles.find((x) => x.id === id);
  const keep = it && OWN_KINDS.includes(it.kind) ? [...(t.hidden ?? []).filter((x) => x.id !== id), it] : (t.hidden ?? []);
  return { ...t, preset: "custom", tiles: t.tiles.filter((x) => x.id !== id), hidden: keep };
}

/** Puts a kept tile back on the island, at the end. */
export function showTile(t: IslandTiles, id: string): IslandTiles {
  const it = (t.hidden ?? []).find((x) => x.id === id);
  if (!it || t.tiles.length >= MAX_TILES) return t;
  return { ...t, preset: "custom", tiles: [...t.tiles, it], hidden: t.hidden.filter((x) => x.id !== id) };
}

/** A tile's own name (empty: its usual one). */
export function renameTile(t: IslandTiles, id: string, label: string): IslandTiles {
  const name = cleanText(label, 40);
  const re = (x: Tile) => (x.id === id ? { ...x, label: name } : x);
  return { ...t, preset: "custom", tiles: t.tiles.map(re), hidden: (t.hidden ?? []).map(re) };
}

/** Moves the tile `id` to position `to` (drag and drop, or the arrows). */
export function moveTile(t: IslandTiles, id: string, to: number): IslandTiles {
  const from = t.tiles.findIndex((x) => x.id === id);
  if (from < 0) return t;
  const tiles = [...t.tiles];
  const [it] = tiles.splice(from, 1);
  tiles.splice(Math.max(0, Math.min(to, tiles.length)), 0, it);
  return { ...t, preset: "custom", tiles };
}

export function resizeTile(t: IslandTiles, id: string, size: TileSize): IslandTiles {
  return { ...t, preset: "custom", tiles: t.tiles.map((x) => (x.id === id ? { ...x, size } : x)) };
}

/** Whether a Windows notification is mail for the island: from a chosen app, or looking like mail. */
export function isMailNotification(app: string, mailApps: readonly string[]): boolean {
  const a = app.toLowerCase();
  if (mailApps.length > 0) return mailApps.some((m) => a.includes(m.toLowerCase()));
  return ["mail", "outlook", "thunderbird", "gmail", "proton", "spark"].some((m) => a.includes(m));
}

// ── One tile system (round 3 item 4) ──────────────────────────────────────────
// The services turned on (settings.activeIntegrations, which Rust polls) and
// the island's tiles are one list: every service on has its tile, in the
// user's order, and a service tile means the service is on. Spotify is the
// Music tile.

/** The integration a tile stands for, if any. */
export function tileService(x: Tile): string | null {
  if (x.kind === "service") return x.target;
  if (x.kind === "music") return SPOTIFY;
  return null;
}

/**
 * The tiles with the services as they are: a tile appended for each service
 * turned on that has none, the tiles of services turned off removed. The
 * main tool (Claude Code by default) is always on.
 */
export function withServices(t: IslandTiles, active: readonly string[], mainPill: string): IslandTiles {
  const on = new Set([mainPill, ...active]);
  const tiles = t.tiles.filter((x) => {
    const id = tileService(x);
    return id === null || on.has(id);
  });
  for (const id of [mainPill, ...active]) {
    if (tiles.some((x) => tileService(x) === id)) continue;
    tiles.push(id === SPOTIFY ? tile("music") : tile("service", id));
  }
  const same = tiles.length === t.tiles.length && tiles.every((x, i) => x === t.tiles[i]);
  return same ? t : { ...t, tiles };
}

/**
 * The services on after a change of tiles: those with a tile, in tile order,
 * then the ones that were on — all but the one whose tile was removed.
 */
export function keptServices(fromTiles: readonly string[], wereOn: readonly string[], removed?: string): string[] {
  const out = [...fromTiles];
  for (const id of wereOn) if (id !== removed && !out.includes(id)) out.push(id);
  return out.filter((id) => id !== removed);
}

/** The services the tiles turn on, in tile order, without the main tool. */
export function servicesOf(t: IslandTiles, mainPill: string): string[] {
  const ids: string[] = [];
  for (const x of t.tiles) {
    const id = tileService(x);
    if (id && id !== mainPill && !ids.includes(id)) ids.push(id);
  }
  return ids;
}

/** Names apps give themselves that are longer than what people call them. */
const SHORT_NAMES: Record<string, string> = {
  "visual studio code": "VS Code",
  "microsoft visual studio code": "VS Code",
  "google chrome": "Chrome",
  "microsoft edge": "Edge",
  "microsoft teams": "Teams",
  "microsoft word": "Word",
  "microsoft excel": "Excel",
  "microsoft powerpoint": "PowerPoint",
  "microsoft outlook": "Outlook",
  "mozilla firefox": "Firefox",
  "windows terminal": "Terminal",
  "file explorer": "Explorer",
};

/**
 * An app's name as a tile shows it (round 3E item 1): what people call it,
 * without "Launcher", "- Shortcut", "(x64)" or a version. The user's own
 * label is never changed.
 */
export function shortAppName(name: string): string {
  const trimmed = name.trim();
  const known = SHORT_NAMES[trimmed.toLowerCase()];
  if (known) return known;
  const short = trimmed
    .replace(/\s*-\s*shortcut$/i, "")
    .replace(/\s*\((x64|x86|64-bit|32-bit)\)$/i, "")
    .replace(/\s+v?\d+(\.\d+)+$/i, "")
    .replace(/\s+(launcher|desktop|app)$/i, "")
    .trim();
  return short || trimmed;
}
