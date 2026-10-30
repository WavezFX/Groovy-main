// The island's normal state (FIXES_ROUND3 item 2): "Up next" on the left, the
// tiles on the right, and the player on top of them while music plays. This
// file is the pure part — what comes up next, which cells show on which page,
// when the player shows — so the rules can be tested without a window.

import { t } from "../i18n/i18n";
import type { Tile } from "./tiles";

/** A meeting starting within this many minutes comes first. */
export const MEETING_WINDOW_MIN = 30;
/** A reminder due within this many minutes is "up next". */
export const REMINDER_WINDOW_MIN = 15;
/** After a pause or stop, the player stays this long before the tiles come back. */
export const PLAYER_HOLD_MS = 1500;
/** Cells per page: 4 × 2. */
export const CELLS_F = 8;

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

/** The cells on each page, the "+ Add tile" cell last. */
export function pagesOf<T>(cells: T[], perPage: number, add: T): T[][] {
  const all = [...cells, add];
  const pages: T[][] = [];
  for (let i = 0; i < all.length; i += perPage) pages.push(all.slice(i, i + perPage));
  return pages;
}

// ── Tiles that expand (round 3D item 8, docs/design/settings-v2/IslandTiles) ──

/** A tile grown in the island: Spotify to 4, the others to 6 (Mail to all 8). */
export type Expanded = "music" | "mail" | "system" | "reminders";

/** A grid spot: CSS grid-column / grid-row, 1-based in the 4 × 2 grid. */
export interface Spot { col: string; row: string }

export interface ExpandedLayout<T> {
  panel: Spot;
  /** The music strip in the fourth column, while something plays. */
  strip: Spot | null;
  /** The tiles still shown beside it, each in its spot. */
  cells: { tile: T; spot: Spot }[];
}

/**
 * Where everything goes while `kind` is grown: Spotify takes 2 × 2 and four
 * tiles stay to its right; the others take three columns and leave the fourth
 * to the music strip (while music plays) or to two tiles. Mail can take all 8.
 */
export function expandedLayout<T>(kind: Expanded, others: T[], playing: boolean, full: boolean): ExpandedLayout<T> {
  if (kind === "music") {
    const spots = [["3", "1"], ["4", "1"], ["3", "2"], ["4", "2"]];
    return {
      panel: { col: "1 / span 2", row: "1 / span 2" },
      strip: null,
      cells: others.slice(0, 4).map((tile, i) => ({ tile, spot: { col: spots[i][0], row: spots[i][1] } })),
    };
  }
  if (kind === "mail" && full) return { panel: { col: "1 / span 4", row: "1 / span 2" }, strip: null, cells: [] };
  const panel = { col: "1 / span 3", row: "1 / span 2" };
  if (playing) return { panel, strip: { col: "4", row: "1 / span 2" }, cells: [] };
  return { panel, strip: null, cells: others.slice(0, 2).map((tile, i) => ({ tile, spot: { col: "4", row: String(i + 1) } })) };
}

/** The tile kind that grows in place, if this one does. */
export function expandsTo(tile: Tile): Expanded | null {
  switch (tile.kind) {
    case "music":
      return "music";
    case "mail":
      return "mail";
    case "system":
      return "system";
    case "reminders":
      return "reminders";
    default:
      return null;
  }
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
