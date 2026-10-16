// The Control Centre window: a sidebar with Groovy's own pages (assistant,
// models, memory… every setting) and Voice and dictation (dictation, the
// Notetaker, « OK Groovy », the statistics, the dictionary, the snippets, the
// styles and their settings), and the page on the right, under its coloured
// band. Ported from Groovy 0.5.0's hub (MIT).

import "./hub.css";
import { h, clear } from "../views/dom";
import { t } from "../i18n/i18n";
import { onEvent } from "../core/bridge";
import { FlowBridge, type FlowPrefs, type FlowStatus, type ModelsStatus } from "../core/flow";
import { hero, liveGroovy } from "./hero";
import { icon, keyLabel } from "./ui";

export type PageId =
  | "dictation" | "notes" | "insights" | "dictionary" | "snippets" | "styles" | "flowsettings" | "voice" | "skills" | "speechsetup"
  | GroovyPageId;

/** The pages drawn by settings/main.ts from Groovy's own sections. */
export type GroovyPageId =
  | "assistant" | "island" | "wardrobe" | "models" | "memory" | "reminders" | "music" | "calls" | "chat"
  | "agents" | "pills" | "phone" | "breaks" | "general" | "permissions" | "system";

interface PageDef {
  id: PageId;
  icon: string;
  title: () => string;
  /** The band's line; none for pages that draw their own band. */
  subtitle?: () => string;
}

export const FLOW_PAGES: PageDef[] = [
  { id: "dictation", icon: "mic", title: () => t("Dictation") },
  { id: "notes", icon: "record", title: () => t("Notetaker") },
  { id: "voice", icon: "wave", title: () => "OK Groovy" },
  { id: "insights", icon: "chart", title: () => t("Statistics") },
  { id: "dictionary", icon: "book", title: () => t("Dictionary") },
  { id: "snippets", icon: "bolt", title: () => t("Snippets") },
  { id: "styles", icon: "text", title: () => t("Styles") },
  { id: "flowsettings", icon: "sliders", title: () => t("Dictation settings") },
];

export const GROOVY_PAGES: PageDef[] = [
  { id: "assistant", icon: "chat", title: () => t("Assistant"), subtitle: () => t("How Groovy thinks: local, online or both, and how he talks to you.") },
  { id: "island", icon: "grid", title: () => t("My island"), subtitle: () => t("Your apps, mail, music and services on the island, in your order.") },
  { id: "models", icon: "chip", title: () => t("Models"), subtitle: () => t("The AI models that run on this computer.") },
  { id: "memory", icon: "book", title: () => t("Memory"), subtitle: () => t("What Groovy remembers about you, on this computer only.") },
  { id: "reminders", icon: "bell", title: () => t("Reminders"), subtitle: () => t("Tasks and reminders Groovy keeps for you.") },
  { id: "music", icon: "music", title: () => t("Music"), subtitle: () => t("Spotify and your own music folder.") },
  { id: "calls", icon: "phone", title: () => t("Voice calls"), subtitle: () => t("Talking with Groovy: languages, microphone and voices.") },
  { id: "wardrobe", icon: "shirt", title: () => t("Wardrobe"), subtitle: () => t("Groovy's looks, outfits and colours.") },
  { id: "chat", icon: "chat", title: () => t("Chat providers"), subtitle: () => t("Online AI with your own keys, and local servers.") },
  { id: "skills", icon: "bolt", title: () => t("Skills") },
  { id: "agents", icon: "terminal", title: () => t("Coding agents"), subtitle: () => t("Claude Code, Codex and the other agents Groovy follows.") },
  { id: "pills", icon: "grid", title: () => t("Pills and integrations"), subtitle: () => t("The pills shown in the island, and the services behind them.") },
  { id: "phone", icon: "phone", title: () => t("Android phone"), subtitle: () => t("Your agents and their approvals on your Android phone, over your Wi-Fi.") },
  { id: "breaks", icon: "clock", title: () => t("Breaks"), subtitle: () => t("Reminders to stand up and rest your eyes.") },
  { id: "general", icon: "gear", title: () => t("General"), subtitle: () => t("Language, sounds, shortcuts and how the island behaves.") },
  { id: "permissions", icon: "shield", title: () => t("Permissions"), subtitle: () => t("What Groovy may do on this computer, and what he did.") },
  { id: "system", icon: "sliders", title: () => t("System"), subtitle: () => t("Performance, logs and diagnostics.") },
];

/** Pages reached from other pages, not listed in the sidebar. */
const HIDDEN_PAGES: PageDef[] = [{ id: "speechsetup", icon: "mic", title: () => t("Set up speech") }];

const ALL = [...GROOVY_PAGES, ...FLOW_PAGES, ...HIDDEN_PAGES];

/** What a page gets to draw itself and reach the rest of the window. */
export interface HubContext {
  status: FlowStatus;
  navigate(page: PageId): void;
  /** Reads the status again (prefs, models) and redraws the sidebar's line. */
  refresh(): Promise<FlowStatus>;
  /** Saves new preferences and keeps them as the window's. */
  setPrefs(change: (p: FlowPrefs) => void): Promise<void>;
  /** The band of a page, Groovy strolling on it. */
  hero(page: PageId, title: string, subtitle: string | null, actions?: HTMLElement[], compact?: boolean): HTMLElement;
  /** Set by a page: called when the page is left. */
  onLeave?: () => void;
}

export type FlowPageRenderer = (container: HTMLElement, ctx: HubContext) => void | Promise<void>;

export interface HubOptions {
  version: string;
  flowPages: Partial<Record<PageId, FlowPageRenderer>>;
  /** Draws Groovy's own sections for one of the Groovy pages into `container`. */
  renderGroovy(page: GroovyPageId, container: HTMLElement): Promise<void>;
}

const DEFAULT_STATUS: FlowStatus = {
  prefs: {
    enabled: false, key: "right_ctrl", mic: "", keepDays: 30, cleanup: false,
    defaultStyle: "natural", summaryEngine: "ai", engine: "parakeet", speechSetupDone: false, openAtLaunch: true, voice: false, voiceOutput: "system", voiceGender: "female", fishVoice: "", fishModel: "s2-pro", cloudSpeech: true, learnWords: true,
  },
  models: { parakeet: { state: "missing" }, downloadMb: 700 },
  keys: "ok",
  aiReady: false,
  engineReady: false,
  mics: [],
  platform: navigator.userAgent.includes("Windows") ? "windows" : "linux",
  autostarted: false,
};

export async function startHub(root: HTMLElement, opts: HubOptions) {
  document.body.classList.add("hub-window");
  // Opened on a page (lib.rs show_hub: settings.html?page=…), else the first one.
  const asked = new URLSearchParams(location.search).get("page");
  const askedId = (asked === "settings" ? "flowsettings" : asked) as PageId | null;
  let current: PageId = askedId && ALL.some((p) => p.id === askedId) ? askedId : "assistant";
  let status: FlowStatus = (await FlowBridge.status()) ?? DEFAULT_STATUS;
  const items = new Map<PageId, HTMLElement>();
  const statusLine = h("div", { class: "hub-status" });
  const scroll = h("div", { class: "hub-scroll" });
  let rendering = 0;

  const ctx: HubContext = {
    get status() {
      return status;
    },
    navigate: (page) => void show(page),
    refresh: async () => {
      status = (await FlowBridge.status()) ?? status;
      drawStatus();
      return status;
    },
    setPrefs: async (change) => {
      const next = { ...status.prefs };
      change(next);
      status = { ...status, prefs: next };
      drawStatus();
      await FlowBridge.setPrefs(next);
    },
    hero: (page, title, subtitle, actions = [], compact = false) => hero(page, title, subtitle, actions, compact),
  };

  function drawStatus() {
    clear(statusLine);
    const models = status.models.parakeet.state;
    let color = "#6b7079";
    let text: string;
    if (!status.prefs.enabled) {
      text = t("Dictation is off");
    } else if (status.prefs.engine === "whisper" && !status.engineReady) {
      color = "#f7b32b";
      text = t("Download a Whisper model in Groovy → Voice calls");
    } else if (status.prefs.engine !== "whisper" && models !== "ready") {
      color = "#f7b32b";
      text = models === "downloading" ? t("Downloading the voice model…") : t("Download the voice model to dictate");
    } else if (status.keys !== "ok" && status.keys !== "wayland") {
      color = "#f7b32b";
      text = t("Use the pill's microphone to dictate");
    } else {
      color = "#34d399";
      text = t("Ready · hold {0}", { 0: keyLabel(status.keys === "wayland" ? "wayland" : status.prefs.key, status.platform) });
    }
    statusLine.append(h("i", { class: "dot", style: `background:${color}` }), h("span", { text }));
  }

  async function show(page: PageId) {
    const def = ALL.find((p) => p.id === page) ?? GROOVY_PAGES[0];
    ctx.onLeave?.();
    ctx.onLeave = undefined;
    current = def.id;
    for (const [id, el] of items) el.classList.toggle("on", id === current);
    const token = ++rendering;
    const page_ = h("div", { class: "hub-page" });
    const renderer = opts.flowPages[def.id];
    if (renderer) {
      await renderer(page_, ctx);
    } else {
      page_.append(hero(def.id, def.title(), def.subtitle?.() ?? null));
      const container = h("div", { id: "settings-root" });
      page_.append(container);
      await opts.renderGroovy(def.id as GroovyPageId, container);
    }
    if (token !== rendering) return;
    clear(scroll);
    scroll.append(page_);
    scroll.scrollTop = 0;
  }

  // ── The sidebar ──
  const side = h("nav", { class: "hub-side" });
  const logo = h("canvas", {});
  side.append(h("div", { class: "hub-brand" }, logo, h("span", { text: "Groovy" })));
  liveGroovy(logo as HTMLCanvasElement);
  const group = (title: string, pages: PageDef[]) => {
    side.append(h("div", { class: "hub-group", text: title }));
    for (const p of pages) {
      const b = h("button", { class: "hub-item", onclick: () => void show(p.id) });
      b.append(icon(p.icon), h("span", { text: p.title() }));
      items.set(p.id, b);
      side.append(b);
    }
  };
  group("Groovy", GROOVY_PAGES);
  group(t("Voice and dictation"), FLOW_PAGES);
  side.append(h("div", { class: "spacer" }), statusLine, h("div", { class: "hub-version", text: `Groovy ${opts.version}` }));

  clear(root);
  root.append(side, h("main", { class: "hub-panel" }, scroll));
  drawStatus();
  await show(current);

  // Closing: the page folds up while the window shrinks back into the island.
  void onEvent<null>("hub-closing", () => document.body.classList.add("closing"));
  void onEvent<string>("hub-page", (page) => {
    const id = (page === "settings" ? "flowsettings" : page) as PageId;
    if (ALL.some((p) => p.id === id)) void show(id);
  });
  void onEvent<FlowPrefs>("flow-prefs", (prefs) => {
    status = { ...status, prefs };
    drawStatus();
  });
  void onEvent<ModelsStatus>("speech-models", (models) => {
    status = { ...status, models };
    drawStatus();
  });
  return {
    redraw: () => void show(current),
    current: () => current,
  };
}
