// Round 6 (FIXES_ROUND6.md): the Discord tile's data — what discord.rs
// reports over the local RPC pipe (voice state, channel, who's speaking) and
// Discord's Windows notifications — and the pure rules the views follow.

import { t } from "../i18n/i18n";

export const DISCORD_COLOR = "#5865F2";
/** How often the visible tile renews discord.rs's 20 s lease. */
export const WATCH_EVERY_MS = 10_000;

export interface DiscordMember { id: string; name: string; speaking: boolean; muted: boolean; deafened: boolean; me: boolean }
export interface DiscordNote { id: number; title: string; body: string }
export interface DiscordState {
  /** "off", "not-set-up", "not-running", "needs-auth", "refused" or "ready". */
  status: string;
  message: string;
  mute: boolean;
  deaf: boolean;
  channel: { id: string; name: string; guildId: string | null } | null;
  members: DiscordMember[];
  notes: DiscordNote[];
  setUp: boolean;
  authorised: boolean;
}

export const EMPTY_DISCORD: DiscordState = {
  status: "off", message: "", mute: false, deaf: false, channel: null, members: [], notes: [], setUp: false, authorised: false,
};

/** Untrusted event data → a state; anything malformed becomes the empty state's value. */
export function parseDiscord(raw: unknown): DiscordState {
  const o = (raw && typeof raw === "object" ? raw : {}) as Record<string, unknown>;
  const str = (v: unknown) => (typeof v === "string" ? v : "");
  const ch = o.channel && typeof o.channel === "object" ? (o.channel as Record<string, unknown>) : null;
  const members = Array.isArray(o.members) ? o.members.filter((m) => m && typeof m === "object").slice(0, 99).map((m) => {
    const r = m as Record<string, unknown>;
    return { id: str(r.id), name: str(r.name).slice(0, 60), speaking: r.speaking === true, muted: r.muted === true, deafened: r.deafened === true, me: r.me === true };
  }) : [];
  const notes = Array.isArray(o.notes) ? o.notes.filter((n) => n && typeof n === "object").slice(0, 5).map((n) => {
    const r = n as Record<string, unknown>;
    return { id: typeof r.id === "number" ? r.id : 0, title: str(r.title).slice(0, 120), body: str(r.body).slice(0, 400) };
  }) : [];
  return {
    status: str(o.status) || "off",
    message: str(o.message),
    mute: o.mute === true,
    deaf: o.deaf === true,
    channel: ch ? { id: str(ch.id), name: str(ch.name), guildId: typeof ch.guildId === "string" ? ch.guildId : null } : null,
    members,
    notes,
    setUp: o.setUp === true,
    authorised: o.authorised === true,
  };
}

/** The store the tile and its grown panel read. */
export const Discord = {
  state: EMPTY_DISCORD as DiscordState,
  set(raw: unknown) {
    this.state = parseDiscord(raw);
  },
};

/** Who's talking, not counting the user. */
export function speakers(s: DiscordState): string[] {
  return s.members.filter((m) => m.speaking && !m.me).map((m) => m.name);
}

/** The compact tile: a title and a line under it. */
export function discordFace(s: DiscordState): { title: string; sub: string } {
  const note = s.notes[0];
  if (!s.setUp) return { title: "Discord", sub: t("Set up in Settings → Discord") };
  if (!s.authorised || s.status === "needs-auth") return { title: "Discord", sub: t("Connect in Settings → Discord") };
  if (s.status === "not-running") return { title: "Discord", sub: note ? `${note.title}: ${note.body}` : t("Discord isn't open.") };
  if (s.status === "refused") return { title: "Discord", sub: s.message || t("Discord refused voice control.") };
  if (!s.channel) return { title: "Discord", sub: note ? `${note.title}: ${note.body}` : t("Not in a call") };
  const who = speakers(s);
  const sub = s.deaf ? t("Deafened") : s.mute ? t("Muted") : who.length ? t("Speaking: {names}.", { names: who.join(", ") }) : t("{n} in the call", { n: s.members.length });
  return { title: s.channel.name ? `🔊 ${s.channel.name}` : "Discord", sub };
}
