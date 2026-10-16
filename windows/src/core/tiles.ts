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

export const DEFAULT_TILES: IslandTiles = { preset: "developer", tiles: PRESETS.developer(), mailApps: [] };

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
  const tiles: Tile[] = [];
  const seen = new Set<string>();
  if (Array.isArray(o.tiles)) {
    for (const t of o.tiles) {
      if (!t || typeof t !== "object") continue;
      const r = t as Record<string, unknown>;
      const kind = KINDS.find((k) => k === r.kind);
      if (!kind) continue;
      const target = cleanText(r.target, 1000);
      if (!validTarget(kind, target)) continue;
      const one = tile(kind, target, cleanText(r.label, 40), r.size === "expanded" ? "expanded" : "compact");
      if (seen.has(one.id)) continue;
      seen.add(one.id);
      tiles.push(one);
      if (tiles.length >= MAX_TILES) break;
    }
  }
  const mailApps = Array.isArray(o.mailApps) ? o.mailApps.map((a) => cleanText(a, 60)).filter(Boolean).slice(0, 20) : [];
  if (!Array.isArray(o.tiles)) return { ...DEFAULT_TILES, preset, tiles: preset === "everyday" ? PRESETS.everyday() : PRESETS.developer(), mailApps };
  return { preset, tiles, mailApps };
}

export function currentTiles(settings: Pick<Settings, "islandTiles">): IslandTiles {
  return parseTiles(settings.islandTiles);
}

/** Whether the island opens on the tiles (not on today's agents overview). */
export function tilesHome(t: IslandTiles): boolean {
  return t.preset !== "developer" && t.tiles.length > 0;
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
  return { ...t, preset: "custom", tiles: t.tiles.filter((x) => x.id !== id) };
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
