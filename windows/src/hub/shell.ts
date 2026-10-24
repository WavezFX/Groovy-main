// The Settings window (round 3D item 1, docs/design/settings-v2/Sidebar):
// Groovy's card, a search box (Ctrl K), then 15 pages in 8 groups, and the
// page on the right with tabs for the merged pages. It remembers the last
// page. Ported from Groovy 0.5.0's hub (MIT).

import "./fonts/manrope.css";
import "./hub.css";
import { h, clear } from "../views/dom";
import { t } from "../i18n/i18n";
import { Bridge, onEvent } from "../core/bridge";
import { FlowBridge, type FlowPrefs, type FlowStatus, type ModelsStatus } from "../core/flow";
import { hero } from "./hero";
import { parseGrowth, shownStage, stageProgress } from "../mochi/growth";
import { SKIN_KEYS, parseSkin, type Skin } from "../mochi/skins";
import { lookSprite, spriteCanvas } from "../mochi/sprites";
import { ALL_PAGES, SECTIONS, TABS, pageDef, resolvePage, search, type Place } from "./nav";
import type { GroovyPageId, PageId } from "./nav";

export type { FlowPageId, GroovyPageId, PageId } from "./nav";

/** What a page gets to draw itself and reach the rest of the window. */
export interface HubContext {
  status: FlowStatus;
  /** A page id, current or old (nav.ts ALIASES), and a tab for merged pages. */
  navigate(page: string, tab?: string): void;
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
  /** Draws Groovy's own sections for one of the Groovy pages (and its tab) into `container`. */
  renderGroovy(page: GroovyPageId, container: HTMLElement, tab?: string): Promise<void>;
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


/** Per-viewer conveniences only: the last page and its tab. */
const REMEMBER = "groovy.settings.nav";
interface Remembered {
  page?: string;
  tab?: string;
}
function recall(): Remembered {
  try {
    return JSON.parse(localStorage.getItem(REMEMBER) ?? "{}") as Remembered;
  } catch {
    return {};
  }
}
function remember(r: Remembered) {
  try {
    localStorage.setItem(REMEMBER, JSON.stringify(r));
  } catch {
    // Private mode or storage off: nothing is remembered.
  }
}

/** After a jump from the search: the row whose label reads `label`, briefly lit. */
function highlight(container: HTMLElement, label: string) {
  const wanted = label.trim().toLowerCase();
  const candidates = container.querySelectorAll<HTMLElement>("label, h2, h3, .hub-row-title, .hub-label, .subhead, b, span");
  for (const el of candidates) {
    if ((el.textContent ?? "").trim().toLowerCase() !== wanted) continue;
    const row = el.closest<HTMLElement>(".hub-row, .row, section, .hub-card") ?? el;
    row.scrollIntoView({ block: "center", behavior: "smooth" });
    row.classList.add("hub-found");
    window.setTimeout(() => row.classList.remove("hub-found"), 1800);
    return;
  }
}

/** The accent follows his look (the approved design: #46EB78 for Night). */
export const LOOK_ACCENT: Record<Skin, string> = { night: "#46EB78", ember: "#FF7A2F", frost: "#4AA8FF" };

const SVG = "http://www.w3.org/2000/svg";

/** A sidebar icon from its path (nav.ts NAV_ICONS). */
function pathIcon(d: string): SVGSVGElement {
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("aria-hidden", "true");
  const path = document.createElementNS(SVG, "path");
  path.setAttribute("d", d);
  svg.append(path);
  return svg;
}

function searchIcon(): SVGSVGElement {
  const svg = pathIcon("M20 20l-4-4");
  const c = document.createElementNS(SVG, "circle");
  c.setAttribute("cx", "11");
  c.setAttribute("cy", "11");
  c.setAttribute("r", "7");
  svg.prepend(c);
  return svg;
}

export async function startHub(root: HTMLElement, opts: HubOptions) {
  document.body.classList.add("hub-window");
  const saved = recall();
  // Opened on a page (lib.rs show_hub: settings.html?page=…), else where it was left, else Home.
  let place: Place = resolvePage(new URLSearchParams(location.search).get("page"))
    ?? resolvePage(saved.page) ?? { page: "home" };
  if (!place.tab && saved.page === place.page && saved.tab) place.tab = saved.tab;
  let status: FlowStatus = (await FlowBridge.status()) ?? DEFAULT_STATUS;
  const items = new Map<PageId, HTMLElement>();
  const scroll = h("div", { class: "hub-scroll" });
  let rendering = 0;

  const keep = () => remember({ page: place.page, tab: place.tab });

  const ctx: HubContext = {
    get status() {
      return status;
    },
    navigate: (page, tab) => {
      const to = resolvePage(page);
      if (to) void show({ page: to.page, tab: tab ?? to.tab });
    },
    refresh: async () => {
      status = (await FlowBridge.status()) ?? status;
      return status;
    },
    setPrefs: async (change) => {
      const next = { ...status.prefs };
      change(next);
      status = { ...status, prefs: next };
      await FlowBridge.setPrefs(next);
    },
    hero: (page, title, subtitle, actions = [], compact = false) => hero(page, title, subtitle, actions, compact),
  };

  /** The tab bar of a merged page; the tab shown is `tab`. */
  function tabBar(page: PageId, tab: string): HTMLElement | null {
    const tabs = TABS[page];
    if (!tabs) return null;
    const bar = h("div", { class: "hub-tabs", role: "tablist" });
    for (const x of tabs) {
      bar.append(h("button", {
        class: x.id === tab ? "hub-tab on" : "hub-tab",
        role: "tab",
        "aria-selected": String(x.id === tab),
        text: t(x.title),
        onclick: () => void show({ page, tab: x.id }),
      }));
    }
    return bar;
  }

  async function show(to: Place, find?: string) {
    const def = pageDef(to.page);
    const tabs = TABS[def.id];
    const tab = tabs ? (tabs.some((x) => x.id === to.tab) ? to.tab! : tabs[0].id) : undefined;
    ctx.onLeave?.();
    ctx.onLeave = undefined;
    place = { page: def.id, tab };
    keep();
    for (const [id, el] of items) el.classList.toggle("on", id === def.id);
    const token = ++rendering;
    const page_ = h("div", { class: "hub-page" });
    const bar = tab ? tabBar(def.id, tab) : null;
    // A tab may be a page of its own (Speaking › Sounds, Dictation › Styles…);
    // Brains' tabs are its own sections.
    const tabPage = tab && tab !== def.id && (opts.flowPages[tab as PageId] || ALL_PAGES.some((x) => x.id === tab));
    const contentId = (tabPage ? tab : def.id) as PageId;
    const flowId = contentId;
    const renderer = opts.flowPages[flowId];
    if (renderer) {
      const inner = h("div", {});
      await renderer(inner, ctx);
      // The renderer drew its own band: the tabs go under it.
      if (bar) inner.insertBefore(bar, inner.children[1] ?? null);
      page_.append(inner);
    } else {
      page_.append(hero(def.id, t(def.title), t(def.subtitle) || null));
      if (bar) page_.append(bar);
      const container = h("div", { id: "settings-root" });
      page_.append(container);
      await opts.renderGroovy(contentId as GroovyPageId, container, tabPage ? undefined : tab);
      // Sections every tab shares come first; the page marks where the tabs go.
      const slot = container.querySelector(".hub-tab-slot");
      if (bar && slot) slot.replaceWith(bar);
    }
    if (token !== rendering) return;
    clear(scroll);
    scroll.append(page_);
    scroll.scrollTop = 0;
    if (find) highlight(page_, find);
  }

  // ── The sidebar (docs/design/settings-v2/Sidebar) ──
  const side = h("nav", { class: "hub-side", "aria-label": t("Settings sections") });
  const card = h("div", { class: "hub-me" });
  const query = h("input", { class: "hub-search", type: "search", placeholder: t("Search settings"), "aria-label": t("Search settings") }) as HTMLInputElement;
  const results = h("div", { class: "hub-results", role: "listbox" });
  const sections = h("div", { class: "hub-sections" });
  /** Accounts that need the user (a mail account to sign in again…): the sidebar's badge. */
  let attention = 0;

  /** Groovy's card: his portrait, look and stage, and how far to the next. */
  async function drawCard() {
    const boot = await Bridge.boot();
    const settings = boot?.settings;
    const look = parseSkin(settings?.groovyLook);
    const g = parseGrowth(settings?.growth);
    document.body.style.setProperty("--hub-accent", LOOK_ACCENT[look]);
    document.body.style.setProperty("--hub-accent-soft", `${LOOK_ACCENT[look]}21`);
    const portrait = h("span", { class: "hub-me-portrait" });
    requestAnimationFrame(() => portrait.append(spriteCanvas(lookSprite(look, shownStage(g), 52, Math.min(2, window.devicePixelRatio || 1)), 52)));
    const bar = h("div", { class: "hub-me-bar" }, h("i", { style: `transform:scaleX(${stageProgress(g).toFixed(3)})` }));
    clear(card);
    card.append(portrait, h("div", { class: "hub-me-text" },
      h("div", { class: "hub-me-name", text: "Groovy" }),
      h("div", { class: "hub-me-stage", text: t("{look} · Stage {stage}", { look: t(SKIN_KEYS[look]), stage: shownStage(g) }) }),
      bar,
    ));
  }

  function drawSide() {
    clear(sections);
    items.clear();
    for (const sec of SECTIONS) {
      if (sec.title) sections.append(h("div", { class: "hub-group", text: t(sec.title) }));
      for (const pg of sec.pages) {
        const b = h("button", { class: pg.id === place.page ? "hub-item on" : "hub-item", type: "button", onclick: () => void show({ page: pg.id }) });
        b.append(pathIcon(pg.icon), h("span", { class: "hub-item-label", text: t(pg.title) }));
        if (pg.id === "accounts" && attention > 0) b.append(h("span", { class: "hub-badge", text: String(attention) }));
        items.set(pg.id, b);
        sections.append(b);
      }
    }
  }

  async function countAttention() {
    const inboxes = (await Bridge.mailInboxes()) ?? [];
    const n = inboxes.filter((x) => x.error).length;
    if (n !== attention) {
      attention = n;
      drawSide();
    }
  }

  function drawResults() {
    clear(results);
    const found = search(query.value);
    results.classList.toggle("shown", query.value.trim().length > 0);
    if (!found.length && query.value.trim()) {
      results.append(h("div", { class: "hub-result-none", text: t("Nothing found") }));
      return;
    }
    for (const r of found) {
      results.append(h("button", {
        class: "hub-result",
        role: "option",
        onclick: () => {
          query.value = "";
          drawResults();
          void show(r.place, r.label);
        },
      }, h("span", { text: r.label }), h("span", { class: "hub-result-where", text: r.where })));
    }
  }
  query.addEventListener("input", drawResults);
  query.addEventListener("keydown", (e) => {
    if (e.key === "Enter") (results.querySelector(".hub-result") as HTMLButtonElement | null)?.click();
    if (e.key === "Escape") {
      query.value = "";
      drawResults();
    }
  });

  const searchBox = h("label", { class: "hub-search-box" }, searchIcon(), query, h("kbd", { class: "hub-kbd", text: "Ctrl K" }), results);
  side.append(card, searchBox, sections);
  drawSide();
  void drawCard();
  void countAttention();
  // Ctrl K (or ⌘K) finds a setting from anywhere in the window.
  document.addEventListener("keydown", (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
      e.preventDefault();
      query.focus();
      query.select();
    }
  });

  clear(root);
  root.append(side, h("main", { class: "hub-panel" }, scroll));
  await show(place);

  // Opening (round 3C item 6): Rust shows the window only after this first
  // paint, then grows it out of the island; the page slides in after it.
  void onEvent<null>("hub-shown", () => document.body.classList.add("shown"));
  requestAnimationFrame(() => requestAnimationFrame(() => void Bridge.hubPainted()));
  // Outside Groovy (the dev page) nothing says so: shown after a moment.
  window.setTimeout(() => document.body.classList.add("shown"), 3000);
  // Closing: the page folds up while the window shrinks back into the island.
  void onEvent<null>("hub-closing", () => document.body.classList.add("closing"));
  void onEvent<string>("hub-page", (page) => {
    const to = resolvePage(page);
    if (to) void show(to);
  });
  void onEvent<FlowPrefs>("flow-prefs", (prefs) => {
    status = { ...status, prefs };
  });
  void onEvent<ModelsStatus>("speech-models", (models) => {
    status = { ...status, models };
  });
  return {
    redraw: () => void show(place),
    current: () => place.page,
  };
}

/** For the tests: every page the sidebar or a link can open. */
export const PAGE_IDS: readonly PageId[] = ALL_PAGES.map((x) => x.id);
