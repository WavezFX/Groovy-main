// The Settings window's map (round 3D item 1): eight sections, each a list of
// pages, some pages with tabs. Old page ids still open the right place
// (ALIASES), so every deep link (the island's tiles, the tray, Rust's
// show_hub) keeps working. The search box looks through SETTINGS: every
// setting by its label and a few keywords, with the page it lives on.

import { N_, t } from "../i18n/i18n";

export type GroovyPageId =
  | "home" | "wardrobe" | "sounds" | "brains" | "memory" | "island" | "accounts" | "music"
  | "reminders" | "breaks" | "agents" | "browser" | "permissions" | "phone"
  | "general" | "system" | "about";

export type FlowPageId =
  | "dictation" | "notes" | "insights" | "dictionary" | "snippets" | "styles" | "flowsettings"
  | "listening" | "speaking" | "skills" | "speechsetup";

export type PageId = GroovyPageId | FlowPageId;

export interface NavPage {
  id: PageId;
  /** The page's own heading when the design gives one ("How Groovy sounds"); else its title. */
  heading?: string;
  /** The sidebar's line icon: an SVG path on a 24 × 24 grid (docs/design/settings-v2/Sidebar). */
  icon: string;
  title: string;
  subtitle: string;
}

export interface NavSection {
  id: string;
  /** The small group label; empty for Home. */
  title: string;
  pages: NavPage[];
}

const p = (id: PageId, icon: string, title: string, subtitle: string, heading?: string): NavPage => ({ id, icon, title, subtitle, heading });

/** The sidebar's icons, from the approved design. */
export const NAV_ICONS = {
  home: "M3 11l9-7 9 7v9a1 1 0 01-1 1h-5v-6H9v6H4a1 1 0 01-1-1z",
  character: "M12 3c-3 0-4 2-4 4M6 10h12a3 3 0 013 3v4a4 4 0 01-4 4H7a4 4 0 01-4-4v-4a3 3 0 013-3z",
  brains: "M9 3a3 3 0 00-3 3 3 3 0 00-2 5 3 3 0 002 5 3 3 0 003 3h1V3zM15 3a3 3 0 013 3 3 3 0 012 5 3 3 0 01-2 5 3 3 0 01-3 3h-1V3z",
  memory: "M4 6h16v12H4zM8 10h8M8 14h5",
  skills: "M13 2L4 14h7l-1 8 9-12h-7z",
  speaking: "M4 10v4M8 7v10M12 4v16M16 7v10M20 10v4",
  listening: "M12 3a3 3 0 00-3 3v6a3 3 0 006 0V6a3 3 0 00-3-3zM5 11a7 7 0 0014 0M12 18v3",
  dictation: "M4 19h16M6 15l9-9 3 3-9 9H6z",
  layout: "M3 5h18v6H3zM3 15h8v4H3zM14 15h7v4h-7z",
  accounts: "M4 6h16v12H4zM4 7l8 6 8-6",
  reminders: "M6 16V11a6 6 0 0112 0v5l2 2H4zM10 21h4",
  browser: "M12 3a9 9 0 100 18 9 9 0 000-18zM3 12h18M12 3c3 3 3 15 0 18M12 3c-3 3-3 15 0 18",
  permissions: "M12 3l8 3v6c0 5-4 8-8 9-4-1-8-4-8-9V6z",
  general: "M12 9a3 3 0 100 6 3 3 0 000-6zM19 12l2-1-2-4-2 1-2-2V4h-4v2L9 8 7 7l-2 4 2 1v2l-2 1 2 4 2-1 2 2v2h4v-2l2-2 2 1 2-4-2-1z",
  system: "M4 4h16v12H4zM8 20h8M12 16v4",
} as const;
const I = NAV_ICONS;

/** The sidebar (docs/design/settings-v2/Sidebar): 15 pages in 8 groups. Titles are keys of t(). */
export const SECTIONS: NavSection[] = [
  { id: "home", title: "", pages: [
    p("home", I.home, N_("Home"), N_("Everything Groovy is doing, at a glance.")),
  ] },
  { id: "groovy", title: "Groovy", pages: [
    p("wardrobe", I.character, N_("Character & wardrobe"), N_("His look, how far he has grown, and what he's wearing."), N_("Your Groovy")),
  ] },
  { id: "ai", title: N_("AI"), pages: [
    p("brains", I.brains, N_("Brains"), N_("Which AI Groovy thinks with, and what he switches to when one runs out.")),
    p("memory", I.memory, N_("Memory"), N_("What Groovy remembers about you, on this computer only.")),
    p("skills", I.skills, N_("Skills"), N_("Extra things Groovy can do, added and removed here.")),
  ] },
  { id: "voice", title: N_("Voice"), pages: [
    p("speaking", I.speaking, N_("Speaking"), N_("Pick his voice. Fish Audio goes first; Kokoro takes over offline so he's never silent."), N_("How Groovy sounds")),
    p("listening", I.listening, N_("Listening & calls"), N_("« OK Groovy », the microphone, and talking with him like a phone call.")),
    p("dictation", I.dictation, N_("Dictation"), N_("Speak instead of typing, in any app.")),
  ] },
  { id: "island", title: N_("Island"), pages: [
    p("island", I.layout, N_("Layout & tiles"), N_("What the island shows, how it looks and when it opens. Changes appear live."), N_("Your island")),
    p("accounts", I.accounts, N_("Accounts"), N_("Mail, Spotify, GitHub and your other services, in one place.")),
  ] },
  { id: "everyday", title: N_("Everyday"), pages: [
    p("reminders", I.reminders, N_("Reminders & breaks"), N_("Tasks Groovy keeps for you, and reminders to rest.")),
  ] },
  { id: "computer", title: N_("Computer"), pages: [
    p("browser", I.browser, N_("Browser & agents"), N_("The browser Groovy uses, and the coding agents he follows.")),
    p("permissions", I.permissions, N_("Permissions"), N_("What Groovy may do on this computer, and what he did.")),
  ] },
  { id: "app", title: N_("App"), pages: [
    p("general", I.general, N_("General"), N_("Language, theme, shortcuts and startup.")),
    p("system", I.system, N_("System"), N_("How much of your PC Groovy uses. He stays under 300 MB idle and never goes over 400 MB.")),
  ] },
];

/**
 * Pages shown as a tab of a sidebar page (TABS), or reached from another
 * page: they keep their own id, so every old link still opens them.
 */
export const HIDDEN: NavPage[] = [
  p("sounds", I.speaking, N_("Sounds"), N_("The little sounds the island makes.")),
  p("notes", I.dictation, N_("Notetaker"), N_("Record meetings and calls, with a summary.")),
  p("insights", I.dictation, N_("Statistics"), N_("How much you dictated, and when.")),
  p("music", I.layout, N_("Music"), N_("Spotify and your own music folder.")),
  p("breaks", I.reminders, N_("Breaks"), N_("Reminders to stand up and rest your eyes.")),
  p("agents", I.browser, N_("Coding agents"), N_("Claude Code, Codex and the other agents Groovy follows.")),
  p("phone", I.browser, N_("Android phone"), N_("Your agents and their approvals on your phone.")),
  p("about", I.general, N_("About & updates"), N_("Version, licences and credits.")),
  p("speechsetup", I.listening, N_("Set up speech"), ""),
];

export const ALL_PAGES: NavPage[] = [...SECTIONS.flatMap((s) => s.pages), ...HIDDEN];

/**
 * Tabs of the merged pages: the first is the default. A tab whose id is a
 * page's (sounds, calls, notes…) shows that page under the sidebar page.
 */
export const TABS: Partial<Record<PageId, { id: string; title: string }[]>> = {
  brains: [
    { id: "cloud", title: N_("Cloud") },
    { id: "subscriptions", title: N_("Subscriptions") },
    { id: "local", title: N_("On this PC") },
  ],
  speaking: [
    { id: "speaking", title: N_("Voice") },
    { id: "sounds", title: N_("Sounds") },
  ],
  dictation: [
    { id: "dictation", title: N_("Dictation") },
    { id: "flowsettings", title: N_("Settings") },
    { id: "dictionary", title: N_("Dictionary") },
    { id: "snippets", title: N_("Snippets") },
    { id: "styles", title: N_("Styles") },
    { id: "notes", title: N_("Notetaker") },
    { id: "insights", title: N_("Statistics") },
  ],
  island: [
    { id: "island", title: N_("Layout") },
    { id: "music", title: N_("Music") },
  ],
  reminders: [
    { id: "reminders", title: N_("Reminders") },
    { id: "breaks", title: N_("Breaks") },
  ],
  browser: [
    { id: "browser", title: N_("Browser") },
    { id: "agents", title: N_("Coding agents") },
    { id: "phone", title: N_("Android phone") },
  ],
  general: [
    { id: "general", title: N_("General") },
    { id: "about", title: N_("About & updates") },
  ],
};

/** The ids of before round 3D, and where they are now. */
export const ALIASES: Record<string, { page: PageId; tab?: string }> = {
  assistant: { page: "brains" },
  chat: { page: "brains", tab: "cloud" },
  models: { page: "brains", tab: "local" },
  pills: { page: "island" },
  mail: { page: "accounts" },
  // Round 3F item 8: the old "Live calls" page is part of Listening & calls.
  voice: { page: "listening" },
  calls: { page: "listening" },
  settings: { page: "dictation", tab: "flowsettings" },
  flowsettings: { page: "dictation", tab: "flowsettings" },
  dictionary: { page: "dictation", tab: "dictionary" },
  snippets: { page: "dictation", tab: "snippets" },
  styles: { page: "dictation", tab: "styles" },
};

/** The sidebar page a tab page lives under: "sounds" → speaking. */
export function parentOf(id: string): PageId | null {
  for (const [page, tabs] of Object.entries(TABS)) {
    if (page !== id && tabs?.some((x) => x.id === id)) return page as PageId;
  }
  return null;
}

export interface Place {
  page: PageId;
  tab?: string;
}

/** A page id as asked for (current or old) → where to go; unknown → null. */
export function resolvePage(raw: string | null | undefined): Place | null {
  if (!raw) return null;
  const alias = Object.hasOwn(ALIASES, raw) ? ALIASES[raw] : null;
  if (alias) return { ...alias };
  const parent = parentOf(raw);
  if (parent) return { page: parent, tab: raw };
  return ALL_PAGES.some((x) => x.id === raw) ? { page: raw as PageId } : null;
}

export function pageDef(id: PageId): NavPage {
  return ALL_PAGES.find((x) => x.id === id) ?? SECTIONS[0].pages[0];
}

/** "Good morning", "Good afternoon" or "Good evening" (Home's heading). */
export function greeting(hour = new Date().getHours()): string {
  return hour < 12 ? t("Good morning") : hour < 18 ? t("Good afternoon") : t("Good evening");
}

export function sectionOf(id: PageId): NavSection | undefined {
  const page = parentOf(id) ?? id;
  return SECTIONS.find((s) => s.pages.some((x) => x.id === page));
}

// ── Search ────────────────────────────────────────────────────────────────────

export interface SettingEntry {
  /** The row's own label (a key of t()): the search shows it and finds the row by it. */
  label: string;
  page: PageId;
  tab?: string;
  /** More words that find it (English; the label is also searched in the current language). */
  keywords?: string;
}

const s = (label: string, page: PageId, keywords = "", tab?: string): SettingEntry => ({ label, page, tab, keywords });

/** Every setting worth finding, with where it lives. */
export const SETTINGS: SettingEntry[] = [
  s(N_("Wardrobe Studio"), "wardrobe", "outfit clothes hat look colour color stage growth"),
  s(N_("Sound"), "sounds", "sounds volume mute effects"),
  s(N_("Groovy's AI"), "brains", "ai brain provider setup"),
  s(N_("Assistant"), "brains", "personality tone style answers"),
  s(N_("Run setup again"), "general", "first setup start again"),
  s(N_("Chat providers"), "brains", "anthropic claude api key openai google gemini openrouter", "cloud"),
  s("Claude Code", "brains", "subscription claude code", "subscriptions"),
  s(N_("Plan usage"), "brains", "subscription limit usage", "subscriptions"),
  s(N_("Local models"), "brains", "ollama lm studio server local", "local"),
  s(N_("Models"), "brains", "local model download gguf llama", "local"),
  s(N_("Memory"), "memory", "remember facts forget"),
  s(N_("Groovy's voice"), "speaking", "fish audio kokoro voice tts speak"),
  s(N_("Groovy answers out loud"), "speaking", "speak elevenlabs system voice"),
  s(N_("Listen for « OK Groovy »"), "listening", "wake word ok groovy hey groovy"),
  s(N_("Test OK Groovy"), "listening", "check test wake"),
  s(N_("Microphone"), "listening", "mic input"),
  s(N_("Let me interrupt him"), "listening", "barge interrupt"),
  s(N_("Dictation"), "dictation", "dictate speech to text hold key", "dictation"),
  s(N_("Dictionary"), "dictation", "words names spelling", "dictionary"),
  s(N_("Snippets"), "dictation", "shortcut text expand", "snippets"),
  s(N_("Styles"), "dictation", "tone formal casual", "styles"),
  s(N_("Notetaker"), "notes", "record meeting summary"),
  s(N_("My island"), "island", "tiles layout preset order"),
  s(N_("Active pills"), "island", "pills main tool"),
  s(N_("Mail accounts"), "accounts", "gmail outlook icloud imap email"),
  s(N_("Integrations"), "accounts", "github vercel neon resend notion stripe n8n cal"),
  s(N_("Music"), "music", "spotify folder songs"),
  s(N_("Reminders and to-dos"), "reminders", "task todo remind"),
  s(N_("Breaks"), "breaks", "stand rest eyes"),
  s(N_("Browser"), "browser", "duckduckgo chrome edge search engine"),
  s(N_("Permissions"), "permissions", "allow ask deny tools"),
  s(N_("Coding agents"), "agents", "hooks codex claude studio"),
  s(N_("Android phone"), "phone", "pair qr relay"),
  s(N_("Language"), "general", "translate language"),
  s(N_("Appearance"), "general", "glass solid theme island style"),
  s(N_("Shortcuts"), "general", "keyboard keys hotkey"),
  s(N_("Launch at startup"), "general", "startup login autostart windows"),
  s(N_("Free memory after"), "listening", "unload idle memory"),
  s(N_("System"), "system", "diagnostics log logs performance"),
  s(N_("Version"), "about", "update licence license credits"),
];

/** Lower case, accents off: "Réveil" finds "reveil". */
export function fold(text: string): string {
  return text.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();
}

/** Settings and pages matching every word typed, best first (at most `limit`). */
export function search(query: string, limit = 8): { label: string; place: Place; where: string }[] {
  const words = fold(query).split(/\s+/).filter(Boolean);
  if (!words.length) return [];
  const scored: { score: number; label: string; place: Place; where: string }[] = [];
  const consider = (label: string, haystack: string, at: Place) => {
    // A tab page ("sounds") opens under its sidebar page ("Speaking").
    const place = !at.tab && parentOf(at.page) ? { page: parentOf(at.page)!, tab: at.page } : at;
    const hay = fold(haystack);
    if (!words.every((w) => hay.includes(w))) return;
    const name = fold(label);
    const score = name.startsWith(words[0]) ? 3 : name.includes(words[0]) ? 2 : 1;
    scored.push({ score, label, place, where: t(pageDef(place.page).title) });
  };
  for (const page of ALL_PAGES) consider(t(page.title), `${t(page.title)} ${page.title} ${t(page.subtitle)}`, { page: page.id });
  for (const e of SETTINGS) consider(t(e.label), `${t(e.label)} ${e.label} ${e.keywords ?? ""}`, { page: e.page, tab: e.tab });
  const seen = new Set<string>();
  return scored
    .sort((a, b) => b.score - a.score)
    .filter((r) => {
      const key = `${r.label}|${r.place.page}|${r.place.tab ?? ""}`;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    })
    .slice(0, limit)
    .map(({ label, place, where }) => ({ label, place, where }));
}
