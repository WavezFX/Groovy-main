// The island's normal state (FIXES_ROUND3 item 2): "Up next" on the left, the
// tiles on the right, and the player on top of them while music plays. This
// file is the pure part — what comes up next, which cells show on which page,
// when the player shows — so the rules can be tested without a window.

import { t } from "../i18n/i18n";
import { SPOTIFY_ID } from "./spotify";
import type { Tile } from "./tiles";
import { tile as makeTile } from "./tiles";

/** A meeting starting within this many minutes comes first. */
export const MEETING_WINDOW_MIN = 30;
/** A reminder due within this many minutes is "up next". */
export const REMINDER_WINDOW_MIN = 15;
/** After a pause or stop, the player stays this long before the tiles come back. */
export const PLAYER_HOLD_MS = 5000;
/** Cells per page: 4 × 2 normally, one row of 4 under the player. */
export const CELLS_F = 8;
export const CELLS_E = 4;

export interface Meeting {
  title: string;
  /** Unix ms. */
  start: number;
  /** The video link, https only; null opens the calendar. */
  url: string | null;
  source: string;
}

export interface UpNextInput {
  now: number;
  /** The local hour, for the greeting. */
  hour: number;
  meetings: Meeting[];
  mail: { app: string; title: string; body: string } | null;
  agent: { name: string; kind: "approval" | "question" | "finished" | "error"; text: string } | null;
  reminder: { text: string; dueAt: number } | null;
}

export type UpNextKind = "meeting" | "mail" | "agent" | "reminder" | "greeting";

export interface UpNext {
  kind: UpNextKind;
  label: string;
  title: string;
  detail: string;
  /** The meeting's link, when it has one. */
  url?: string | null;
}

function clock(ms: number): string {
  const d = new Date(ms);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

/** The meeting coming up within the window (or started a few minutes ago). */
export function nextMeeting(meetings: Meeting[], now: number): Meeting | null {
  const soon = meetings
    .filter((m) => Number.isFinite(m.start) && m.start - now <= MEETING_WINDOW_MIN * 60_000 && now - m.start <= 5 * 60_000)
    .sort((a, b) => a.start - b.start);
  return soon[0] ?? null;
}

/**
 * The single most important thing right now: a meeting within 30 minutes,
 * then a new mail, then an agent that finished or needs you, then a reminder
 * due, otherwise a greeting.
 */
export function upNext(input: UpNextInput): UpNext {
  const meeting = nextMeeting(input.meetings, input.now);
  if (meeting) {
    const minutes = Math.round((meeting.start - input.now) / 60_000);
    return {
      kind: "meeting",
      label: minutes > 0 ? t("UP NEXT · IN {n} MIN", { n: minutes }) : t("UP NEXT · NOW"),
      title: meeting.title || t("Meeting"),
      detail: `${meeting.source} · ${clock(meeting.start)}`,
      url: meeting.url,
    };
  }
  if (input.mail) {
    return { kind: "mail", label: t("NEW MAIL"), title: input.mail.title || input.mail.app, detail: input.mail.body || input.mail.app };
  }
  if (input.agent) {
    const label = input.agent.kind === "approval" || input.agent.kind === "question" ? t("NEEDS YOU") : input.agent.kind === "error" ? t("STOPPED") : t("FINISHED");
    return { kind: "agent", label, title: input.agent.name, detail: input.agent.text };
  }
  if (input.reminder && input.reminder.dueAt - input.now <= REMINDER_WINDOW_MIN * 60_000) {
    return { kind: "reminder", label: t("REMINDER"), title: input.reminder.text, detail: clock(input.reminder.dueAt) };
  }
  const hello = input.hour < 5 ? t("Up late?") : input.hour < 12 ? t("Good morning") : input.hour < 18 ? t("Good afternoon") : t("Good evening");
  return { kind: "greeting", label: t("UP NEXT"), title: hello, detail: t("Nothing urgent. Ask me anything.") };
}

/**
 * Everything the tile grid shows, in order: the user's tiles, then the
 * services they turned on that have no tile yet. Spotify is the Music tile.
 */
export function homeCells(tiles: Tile[], integrations: string[]): Tile[] {
  const cells = [...tiles];
  const has = (id: string) => cells.some((c) => c.id === id || (c.kind === "service" && c.target === id));
  for (const id of integrations) {
    if (id === SPOTIFY_ID) {
      if (!cells.some((c) => c.kind === "music")) cells.push(makeTile("music"));
      continue;
    }
    if (!has(id)) cells.push(makeTile("service", id));
  }
  return cells;
}

/** The cells on each page, the "+ Add tile" cell last. */
export function pagesOf<T>(cells: T[], perPage: number, add: T): T[][] {
  const all = [...cells, add];
  const pages: T[][] = [];
  for (let i = 0; i < all.length; i += perPage) pages.push(all.slice(i, i + perPage));
  return pages;
}

/** While the player shows, the Music tile isn't repeated under it. */
export function cellsUnderPlayer(cells: Tile[]): Tile[] {
  return cells.filter((c) => c.kind !== "music");
}

/**
 * Whether the big player shows: at once when music plays, and still for
 * PLAYER_HOLD_MS after it stops, so a quick pause or skip doesn't flicker.
 */
export class PlayerHold {
  private shown = false;
  private stoppedAt: number | null = null;

  update(playing: boolean, now: number): boolean {
    if (playing) {
      this.shown = true;
      this.stoppedAt = null;
    } else if (this.shown) {
      this.stoppedAt ??= now;
      if (now - this.stoppedAt >= PLAYER_HOLD_MS) {
        this.shown = false;
        this.stoppedAt = null;
      }
    }
    return this.shown;
  }

  /** Milliseconds until the player should be looked at again, or null. */
  wait(now: number): number | null {
    return this.stoppedAt === null ? null : Math.max(0, PLAYER_HOLD_MS - (now - this.stoppedAt));
  }
}
