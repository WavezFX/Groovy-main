// His name (round 5 item 6): chosen when he hatches, changed in Settings at
// any time. It is the assistant's name (`assistant.name`), so greetings, the
// stage line, the tray, Settings and his replies all use it. The wake word
// stays « OK Groovy »; « Also wake to his name » adds it to the spotter when it
// is long enough to be heard reliably.

import { N_ } from "../i18n/i18n";

export const MAX_NAME = 20;
export const DEFAULT_NAME = "Groovy";

/** Letters and numbers of any script, spaces and simple punctuation. */
const ALLOWED = /^[\p{L}\p{M}\p{N} .,'’\-!?]+$/u;

export type NameCheck = { ok: true; name: string } | { ok: false; why: string };

/** A typed name, cleaned (spaces trimmed and collapsed), or why it can't be used. */
export function checkName(raw: string): NameCheck {
  const name = raw.replace(/\s+/g, " ").trim();
  if (!name) return { ok: false, why: N_("Give him a name: one letter at least.") };
  if ([...name].length > MAX_NAME) return { ok: false, why: N_("20 characters at most.") };
  if (!ALLOWED.test(name) || !/[\p{L}\p{N}]/u.test(name)) return { ok: false, why: N_("Letters, numbers, spaces and simple punctuation only.") };
  return { ok: true, name };
}

/** His name as stored, or Groovy. */
export function nameOf(settings: { assistant?: { name?: unknown } }): string {
  const n = settings.assistant?.name;
  return typeof n === "string" && n.trim() ? n.trim() : DEFAULT_NAME;
}

/** Rough English syllables: groups of vowels, a silent final e left out. 0 for other scripts. */
export function syllables(name: string): number {
  const words = name.toLowerCase().split(/[\s\-]+/).filter(Boolean);
  if (!words.length || !words.every((w) => /^[a-z'.,!?]+$/.test(w))) return 0;
  let n = 0;
  for (const raw of words) {
    const w = raw.replace(/[^a-z]/g, "");
    let groups = (w.match(/[aeiouy]+/g) ?? []).length;
    if (groups > 1 && /[^aeiouy]e$/.test(w) && !/le$/.test(w)) groups--;
    n += Math.max(groups, w ? 1 : 0);
  }
  return n;
}

/** Whether the spotter can listen for this name, and if not, why (for the hint). */
export function wakeName(name: string): "ok" | "short" | "letters" | "groovy" {
  if (name.trim().toLowerCase() === "groovy") return "groovy";
  const s = syllables(name);
  if (s === 0) return "letters";
  return s >= 2 ? "ok" : "short";
}
