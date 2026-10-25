// The island's normal state (FIXES_ROUND3 item 2, mockups MixF and MixE in
// docs/design/island-round3): "Up next" with Groovy on the left — the one
// thing that matters now and the Dictate, Call and Record buttons — and the
// tiles on the right, four by two, with pages when there are more.
//
// Round 3D item 8 (docs/design/settings-v2/IslandTiles): a tile grows out of
// its own spot while the others slide aside. Spotify takes 4 tiles; Mail,
// This PC and Reminders take 6 with the music strip beside them while music
// plays (held 1.5 s after it stops, so a skip doesn't flicker); Mail can take
// all 8. Back, Esc or a click away folds it (island.ts). Only transform,
// opacity and clip-path move.

import { Bridge } from "../core/bridge";
import { FlowBridge } from "../core/flow";
import { readPulse } from "../core/github";
import {
  CELLS_F, PlayerHold, expandedLayout, expandsTo, pagesOf, upNext, type Expanded, type Meeting, type UpNext, type UpNextInput,
} from "../core/home";
import { EASE_OUT, MOTION, duration } from "../core/motion";
import type { IslandViewName } from "../core/layout";
import { pillDefinition, sanitizeDeclared } from "../core/pills";
import { Spotify, currentArtwork } from "../core/spotify";
import { State } from "../core/state";
import { currentTiles, tile as makeTile, withServices, type Tile } from "../core/tiles";
import { t } from "../i18n/i18n";
import { CallControl } from "../voice/call";
import { h, svg, clear } from "./dom";
import { ICONS } from "./icons";
import { idleStatus } from "./integrations";
import { Mail, face, refreshSlowData, upcomingReminder } from "./tiles";
import { once } from "../core/lazy";
import type { Grown } from "./expand";
import { playOrOpen } from "./spotify";
import { showError } from "./toast";
import type { ViewActions, ViewHost } from "./views";

const ADD: Tile = makeTile("app", "+", "+");

// ── What the cards read ───────────────────────────────────────────────────────

function data(id: string): Record<string, unknown> {
  return (State.integrations[id]?.data ?? {}) as Record<string, unknown>;
}

function list(id: string, key: string): Record<string, unknown>[] {
  const v = data(id)[key];
  return Array.isArray(v) ? (v as Record<string, unknown>[]) : [];
}

function meetings(): Meeting[] {
  return list("integration_calcom", "bookings").map((b) => ({
    title: String(b.title ?? ""),
    start: new Date(String(b.start)).getTime(),
    url: typeof b.meetingUrl === "string" && b.meetingUrl.startsWith("https://") ? b.meetingUrl : null,
    source: "Cal.com",
  }));
}

function agentNews(): UpNextInput["agent"] {
  const req = State.pendingApproval;
  if (req) {
    const task = State.tasks.find((x) => x.id === req.pillId);
    return { name: task?.name ?? "Claude Code", kind: req.questions ? "question" : "approval", text: req.questions ? req.questions[0]?.question ?? "" : `${req.tool} · ${req.command}`.slice(0, 120) };
  }
  const done = State.tasks.find((x) => !x.isIntegration && (x.state === "finished" || x.state === "error"));
  if (!done) return null;
  const text = done.finalLine ?? done.steps[done.steps.length - 1] ?? "";
  return { name: done.name, kind: done.state === "error" ? "error" : "finished", text };
}

export function currentUpNext(now = Date.now()): UpNext {
  const mail = Mail.notes[0];
  return upNext({
    now,
    hour: new Date(now).getHours(),
    meetings: meetings(),
    mail: mail ? { app: mail.app, title: mail.title, body: mail.body } : null,
    agent: agentNews(),
    reminder: State.reminderDue ? { text: State.reminderDue.text, dueAt: now } : upcomingReminder(),
  });
}

/** A service's live line on its tile ("2 PRs", "Ready", "14 sent"); good ones in green. */
function serviceLine(id: string): { text: string; good: boolean } {
  const task = State.tasks.find((x) => x.id === id);
  if (task && task.state !== "idle") {
    switch (task.state) {
      case "approval":
      case "question":
        return { text: t("Needs you"), good: false };
      case "finished":
        return { text: task.finalLine ?? task.steps[task.steps.length - 1] ?? t("Done"), good: true };
      case "error":
        return { text: task.steps[task.steps.length - 1] ?? t("Error"), good: false };
      default:
        return { text: task.steps[task.stepIndex] ?? task.steps[task.steps.length - 1] ?? t("Working…"), good: false };
    }
  }
  const info = State.integrations[id];
  if (info && !info.error) {
    switch (id) {
      case "integration_github": {
        const pulse = readPulse(data(id));
        if (pulse) return { text: t("{n} PRs", { n: pulse.myPRs.length }), good: false };
        break;
      }
      case "integration_vercel": {
        const d = list(id, "deployments")[0];
        if (d) return d.state === "READY" ? { text: t("Ready"), good: true } : { text: String(d.state ?? "").toLowerCase(), good: false };
        break;
      }
      case "integration_resend": {
        const total = data(id).total;
        if (typeof total === "number") return { text: t("{n} sent", { n: total }), good: false };
        break;
      }
      case "integration_neon": {
        const total = data(id).total;
        if (typeof total === "number") return { text: t("{n} projects", { n: total }), good: total > 0 };
        break;
      }
      case "integration_notion": {
        const n = list(id, "pages").length;
        if (n) return { text: t("{n} pages", { n }), good: false };
        break;
      }
      case "integration_calcom": {
        const next = meetings().filter((m) => m.start > Date.now()).sort((a, b) => a.start - b.start)[0];
        if (info.loaded) return { text: next ? new Date(next.start).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) : t("No calls"), good: false };
        break;
      }
      case "integration_n8n":
        if (task?.steps[0]) return { text: task.steps[0], good: task.state === "finished" };
        break;
    }
  }
  const status = idleStatus(id, info, State.settings.model);
  return { text: status.label, good: status.color === "#22C55E" };
}

// ── Cells ─────────────────────────────────────────────────────────────────────

function iconBtn(path: string, label: string, onclick: () => void, cls = ""): HTMLElement {
  // Round 3C item 3: 32 px buttons with 20 px icons, the play/pause 40 px with 24 px.
  const b = h("button", { class: `home-ctl ${cls}`.trim(), title: label, "aria-label": label }, svg(path, cls === "main" ? 24 : 20));
  b.addEventListener("click", (e) => {
    e.stopPropagation();
    onclick();
  });
  return b;
}

/**
 * The Spotify tile at its 1-tile size (round 3E item 1): the cover (or
 * Spotify's mark), the song and one play/pause; previous and next are in the
 * grown player. Anywhere else on it grows it to 4.
 */
function musicCell(): HTMLElement {
  const playing = Spotify.state.playing;
  const track = Spotify.state.track;
  const cover = currentArtwork();
  const art = cover
    ? h("img", { class: "home-mark art", src: cover, alt: "", draggable: "false" })
    : h("span", { class: "home-mark", style: "background:#1DB954" });
  const cell = h("div", { class: "home-cell music", title: track ? `Spotify — ${track.title}` : "Spotify", role: "button", tabindex: "0" },
    h("div", { class: "home-cell-head" }, art, iconBtn(playing ? ICONS.pause : ICONS.play, playing ? t("Pause") : t("Play"), playOrOpen, "main")),
    h("span", { class: "home-cell-text" }, h("b", { text: track?.title ?? "Spotify" }), h("span", { text: track?.artist ?? t("Music") })),
  );
  cell.addEventListener("click", () => grow("music"));
  return cell;
}

/** Grows a tile in place (round 3D item 8); only one at a time. */
function grow(kind: Expanded) {
  State.homeExpanded = kind;
  State.notify();
}

function cell(tile: Tile, setView: (v: IslandViewName) => void): HTMLElement {
  const el = cellBody(tile, setView);
  el.dataset.key = tile === ADD ? "+" : tile.id;
  return el;
}

function cellBody(tile: Tile, setView: (v: IslandViewName) => void): HTMLElement {
  if (tile === ADD) {
    return h("button", { class: "home-cell add", title: t("Add a tile"), "aria-label": t("Add a tile"), onclick: () => void Bridge.openSettingsWindow("island") }, svg(ICONS.plus, 20));
  }
  if (tile.kind === "music") return musicCell();
  if (tile.kind === "service") {
    const def = pillDefinition(tile.target);
    const line = serviceLine(tile.target);
    const open = face(tile, setView).open;
    return h("button", { class: "home-cell", title: `${def?.name ?? tile.target} — ${line.text}`, onclick: open },
      h("span", { class: "home-mark", style: `background:${def?.color ?? "#8E939C"}` }),
      h("span", { class: "home-cell-text" }, h("b", { text: tile.label || def?.name || tile.target }), h("span", { class: line.good ? "good" : "", text: line.text })),
    );
  }
  const f = face(tile, setView);
  return h("button", { class: "home-cell", title: `${f.title} — ${f.sub}`, onclick: f.open },
    h("span", { class: "home-mark letter", style: `background:${f.color}`, text: f.icon }),
    h("span", { class: "home-cell-text" }, h("b", { text: f.title }), h("span", { text: f.sub })),
  );
}

// ── The view ──────────────────────────────────────────────────────────────────

export function buildHome(actions: ViewActions): ViewHost {
  // Left: Groovy (drawn by the island's own canvas at this view's bot spot), then the text.
  const label = h("div", { class: "home-label" });
  const title = h("div", { class: "home-title" });
  const detail = h("div", { class: "home-detail" });
  const pill = (text: string, cls: string, onclick: () => void) => h("button", { class: `home-pill ${cls}`, text, onclick });
  const buttons = h("div", { class: "home-actions" },
    pill(t("Dictate"), "", () => void FlowBridge.liveAction("dictate")),
    pill(t("Call"), "", () => {
      CallControl.toggle();
      actions.setView("prompt");
    }),
    pill(t("Record"), "record", () => void FlowBridge.liveAction("note")),
  );
  const what = h("button", { class: "home-what" }, label, title, detail);
  const left = h("div", { class: "home-card home-next" }, h("div", { class: "home-bot" }), h("div", { class: "home-text" }, what, buttons));
  let shown: UpNext | null = null;
  what.addEventListener("click", () => {
    switch (shown?.kind) {
      case "meeting":
        void Bridge.openUrl(shown.url ?? "https://app.cal.com/bookings");
        break;
      case "mail": {
        const n = Mail.notes[0];
        if (n) void Bridge.mailOpen(n.appId).catch(showError);
        break;
      }
      case "agent":
        if (State.pendingApproval) actions.setView(State.pendingApproval.questions ? "question" : "approval");
        else actions.setView("overview");
        break;
      case "reminder":
        void Bridge.openSettingsWindow("reminders");
        break;
      default:
        actions.setView("prompt");
    }
  });

  // Right: the tiles, with page dots, or one of them grown (round 3D item 8).
  const grid = h("div", { class: "home-grid" });
  const dots = h("div", { class: "home-dots", role: "tablist" });
  const right = h("div", { class: "home-card home-right" }, grid, dots);
  let page = 0;
  let pageCount = 1;
  /** Which way the last page change went, for the slide (round 3E item 1). */
  let turned = 0;
  const turn = (to: number) => {
    if (pageCount < 2 || State.homeExpanded) return;
    const next = ((to % pageCount) + pageCount) % pageCount;
    if (next === page) return;
    turned = next > page ? 1 : -1;
    page = next;
    lastGrid = "";
    draw();
  };
  // The wheel, and a horizontal swipe over the tiles, change pages too.
  let wheelAt = 0;
  right.addEventListener("wheel", (e) => {
    if (pageCount < 2 || State.homeExpanded) return;
    e.preventDefault();
    const now = Date.now();
    if (now - wheelAt < 350) return;
    wheelAt = now;
    turn(page + (e.deltaY > 0 || e.deltaX > 0 ? 1 : -1));
  }, { passive: false });
  let swipeFrom: { x: number; y: number } | null = null;
  right.addEventListener("pointerdown", (e) => {
    swipeFrom = { x: e.clientX, y: e.clientY };
  });
  right.addEventListener("pointerup", (e) => {
    if (!swipeFrom) return;
    const dx = e.clientX - swipeFrom.x;
    const dy = e.clientY - swipeFrom.y;
    swipeFrom = null;
    if (Math.abs(dx) > 40 && Math.abs(dx) > Math.abs(dy) * 1.5) turn(page + (dx < 0 ? 1 : -1));
  });

  const fold = () => {
    State.homeExpanded = null;
    State.notify();
  };
  // The grown tiles' code loads the first time one opens, and each is built
  // then (round 3D item 9: nothing before it's needed).
  let expand: typeof import("./expand") | null = null;
  const loadExpand = once(() => import("./expand").then((m) => (expand = m)));
  const grownTiles = new Map<Expanded | "strip", Grown>();
  // Back in Standby (round 3D item 10): the grown tiles not on screen are rebuilt when next opened.
  window.addEventListener("groovy-release", () => {
    for (const k of [...grownTiles.keys()]) if (k !== open && !(k === "strip" && stripOn)) grownTiles.delete(k);
  });
  const grown = (k: Expanded | "strip"): Grown => {
    let g = grownTiles.get(k);
    if (!g) {
      const { mailGrown, musicGrown, musicStrip, remindersGrown, systemGrown } = expand!;
      g = k === "music" ? musicGrown(fold)
        : k === "mail" ? mailGrown(fold, () => {
          State.mailFull = !State.mailFull;
          State.notify();
        })
        : k === "system" ? systemGrown(fold)
        : k === "reminders" ? remindersGrown(fold)
        : musicStrip();
      grownTiles.set(k, g);
    }
    return g;
  };

  const hold = new PlayerHold();
  let holdTimer: number | null = null;
  let progressAt = 0;
  let lastGrid = "";
  let lastLayout = "";
  let open: Expanded | null = null;
  let stripOn = false;
  // Its own key, so a data refresh doesn't rebuild a grown tile (a reply being typed keeps its focus).
  let lastGrown = "";

  /** Where each tile was, so the move to where it is now can be animated. */
  function spots(): Map<string, DOMRect> {
    const m = new Map<string, DOMRect>();
    for (const c of Array.from(grid.children) as HTMLElement[]) if (c.dataset.key) m.set(c.dataset.key, c.getBoundingClientRect());
    return m;
  }

  /** Tiles slide from where they were; a grown tile opens out of its own spot. */
  function slide(before: Map<string, DOMRect>) {
    const ms = duration(MOTION.unfold);
    for (const c of Array.from(grid.children) as HTMLElement[]) {
      const was = c.dataset.key ? before.get(c.dataset.key) : undefined;
      if (!was || typeof c.animate !== "function") continue;
      const now = c.getBoundingClientRect();
      if (!now.width || !now.height) continue;
      if (now.width > was.width + 1 || now.height > was.height + 1) {
        // Grows: revealed from the tile's own box outwards.
        const inset = `inset(${was.top - now.top}px ${now.right - was.right}px ${now.bottom - was.bottom}px ${was.left - now.left}px round 12px)`;
        c.animate([{ clipPath: inset }, { clipPath: "inset(0px 0px 0px 0px round 14px)" }], { duration: ms, easing: EASE_OUT });
      } else if (Math.abs(was.left - now.left) > 1 || Math.abs(was.top - now.top) > 1 || now.width < was.width - 1) {
        c.animate([{ transform: `translate(${was.left - now.left}px, ${was.top - now.top}px)`, opacity: 0.6 }, { transform: "none", opacity: 1 }], { duration: ms, easing: EASE_OUT });
      }
    }
  }

  function place(el: HTMLElement, spot: { col: string; row: string }, key: string): HTMLElement {
    el.style.gridColumn = spot.col;
    el.style.gridRow = spot.row;
    el.dataset.key = key;
    return el;
  }

  function draw() {
    const declared = sanitizeDeclared(State.settings, State.os);
    const cells = withServices(currentTiles(State.settings), declared.activeIntegrations, declared.mainPill).tiles;
    let exp = State.mode === "expanded" ? State.homeExpanded : null;
    if (exp && !expand) {
      // Its code is on its way: the tiles stay as they are a moment longer.
      void loadExpand().then(() => State.notify());
      exp = null;
      if (open === null) return;
    }
    if (exp !== open) {
      if (open) grown(open).shown(false);
      open = exp;
      if (open) grown(open).shown(true);
    }
    const playing = hold.update(Spotify.state.playing && !!Spotify.state.track, Date.now());
    right.classList.toggle("grown", !!exp);
    if (exp) {
      const tile = cells.find((c) => expandsTo(c) === exp);
      const others = cells.filter((c) => c !== tile);
      const lay = expandedLayout(exp, others, playing && exp !== "music", State.mailFull);
      if (!!lay.strip !== stripOn) {
        stripOn = !!lay.strip;
        grown("strip").shown(stripOn);
      }
      const key = `${exp}|${State.mailFull}|${stripOn}|${lay.cells.map((c) => `${c.tile.id}:${c.tile.kind === "service" ? serviceLine(c.tile.target).text : c.tile.kind === "music" ? Spotify.state.playing : face(c.tile, actions.setView).sub}`).join("|")}`;
      if (key !== lastGrown) {
        lastGrown = key;
        lastGrid = "grown";
        lastLayout = "";
        const before = spots();
        clear(grid);
        grid.append(place(grown(exp).el, lay.panel, tile?.id ?? exp));
        if (lay.strip) grid.append(place(grown("strip").el, lay.strip, "strip"));
        for (const c of lay.cells) grid.append(place(cell(c.tile, actions.setView), c.spot, c.tile.id));
        slide(before);
      }
      grown(exp).sync?.();
      if (stripOn) grown("strip").sync?.();
      dots.hidden = true;
      return;
    }
    if (stripOn) {
      stripOn = false;
      grown("strip").shown(false);
    }
    const pages = pagesOf(cells, CELLS_F, ADD);
    pageCount = pages.length;
    page = Math.min(page, pageCount - 1);
    const faces = pages[page].map((c) => (c === ADD ? "+" : c.kind === "service" ? `${c.id}:${serviceLine(c.target).text}` : c.kind === "music" ? `music:${Spotify.state.playing}:${Spotify.state.track?.title ?? ""}:${!!currentArtwork()}` : `${c.id}:${face(c, actions.setView).sub}`));
    const key = `${page}/${pageCount}|${faces.join("|")}`;
    if (key === lastGrid) return;
    const folding = lastGrown !== "";
    lastGrown = "";
    lastGrid = key;
    const before = folding ? spots() : new Map<string, DOMRect>();
    // The tiles themselves changed (not just their text): they grow in (round 3B item 8).
    const layout = `${page}|${pages[page].map((c) => (c === ADD ? "+" : c.id)).join(",")}`;
    if (layout !== lastLayout && !folding) {
      grid.classList.remove("entering");
      void grid.offsetWidth;
      grid.classList.add("entering");
      window.setTimeout(() => grid.classList.remove("entering"), 500);
    }
    lastLayout = layout;
    clear(grid);
    for (const c of pages[page]) grid.append(cell(c, actions.setView));
    if (folding) slide(before);
    if (turned && typeof grid.animate === "function") {
      grid.animate([{ transform: `translateX(${turned * 24}px)`, opacity: 0.4 }, { transform: "none", opacity: 1 }], { duration: duration(MOTION.unfold), easing: EASE_OUT });
    }
    turned = 0;
    clear(dots);
    dots.hidden = pageCount < 2;
    for (let i = 0; i < pageCount; i++) {
      // An 8 px dot in a 32 × 32 px button.
      const dot = h("button", { class: i === page ? "on" : "", role: "tab", "aria-selected": String(i === page), "aria-label": t("Page {n}", { n: i + 1 }) }, h("i"));
      dot.addEventListener("click", (e) => {
        e.stopPropagation();
        turn(i);
      });
      dots.append(dot);
    }
  }

  const el = h("div", { class: "view home-view" }, left, right);
  return {
    el,
    sync() {
      const now = Date.now();
      refreshSlowData(() => {
        if (State.view === "home") {
          lastGrid = "";
          this.sync();
        }
      });
      shown = currentUpNext(now);
      label.textContent = shown.label;
      title.textContent = shown.title;
      detail.textContent = shown.detail;
      left.dataset.kind = shown.kind;
      draw();
      // Paused or stopped: look again once the hold is over.
      const wait = hold.wait(now);
      if (holdTimer !== null) clearTimeout(holdTimer);
      holdTimer = wait === null ? null : window.setTimeout(() => {
        holdTimer = null;
        State.notify();
      }, wait + 50);
    },
    tick(now: number) {
      // The grown player's bar, four times a second while music plays.
      if (!Spotify.state.playing || open !== "music") return false;
      if (now - progressAt >= 250) {
        progressAt = now;
        grown("music").sync?.();
      }
      return true;
    },
  };
}
