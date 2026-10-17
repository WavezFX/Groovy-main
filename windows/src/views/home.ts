// The island's normal state (FIXES_ROUND3 item 2, mockups MixF and MixE in
// docs/design/island-round3): "Up next" with Groovy on the left — the one
// thing that matters now and the Dictate, Call and Record buttons — and the
// tiles on the right, four by two, with pages when there are more. While
// music plays, the right card turns into a wide player over one row of tiles,
// and folds back 1.5 s after it stops. Only transform and opacity move: the
// grid keeps its two rows and slides under the player (no layout change mid-way).

import { Bridge } from "../core/bridge";
import { FlowBridge } from "../core/flow";
import { readPulse } from "../core/github";
import {
  CELLS_E, CELLS_F, PlayerHold, cellsUnderPlayer, pagesOf, upNext, type Meeting, type UpNext, type UpNextInput,
} from "../core/home";
import type { IslandViewName } from "../core/layout";
import { pillDefinition, sanitizeDeclared } from "../core/pills";
import { Spotify, currentArtwork, formatTime, isAd, spotifyPosition } from "../core/spotify";
import { State } from "../core/state";
import { currentTiles, tile as makeTile, withServices, type Tile } from "../core/tiles";
import { t } from "../i18n/i18n";
import { CallControl } from "../voice/call";
import { h, svg, clear } from "./dom";
import { ICONS } from "./icons";
import { idleStatus } from "./integrations";
import { Mail, face, refreshSlowData, upcomingReminder } from "./tiles";
import { togglePlay } from "./spotify";
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

function playPause() {
  if (!Spotify.state.running) {
    // Nothing to play in yet: open Spotify (or its Store page).
    void Bridge.spotifyOpen();
    return;
  }
  togglePlay();
}

/** The Spotify tile while nothing plays: its mark and ⏮ ▶ ⏭, all working. */
function musicCell(): HTMLElement {
  const playing = Spotify.state.playing;
  return h("div", { class: "home-cell music", title: "Spotify" },
    h("div", { class: "home-cell-head" }, h("span", { class: "home-mark", style: "background:#1DB954" }), h("b", { text: "Spotify" })),
    h("div", { class: "home-music-ctl" },
      iconBtn(ICONS.backward, t("Previous"), () => void Bridge.spotifyControl("previous")),
      iconBtn(playing ? ICONS.pause : ICONS.play, playing ? t("Pause") : t("Play"), playPause, "main"),
      iconBtn(ICONS.forward, t("Next"), () => void Bridge.spotifyControl("next")),
    ),
  );
}

function cell(tile: Tile, setView: (v: IslandViewName) => void): HTMLElement {
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

  // Right: the player (while music plays) over the tiles, with page dots.
  const art = h("div", { class: "home-art" });
  const pTitle = h("b", {});
  const pArtist = h("span", {});
  const bar = h("i", {});
  const elapsed = h("span", { class: "home-time" });
  const total = h("span", { class: "home-time" });
  const playBtn = iconBtn(ICONS.play, t("Play"), playPause, "main");
  const player = h("div", { class: "home-player", role: "button", title: t("Open Spotify") },
    art,
    h("div", { class: "home-player-text" }, pTitle, pArtist, h("div", { class: "home-progress" }, elapsed, h("div", { class: "home-track" }, bar), total)),
    h("div", { class: "home-player-ctl" },
      iconBtn(ICONS.backward, t("Previous"), () => void Bridge.spotifyControl("previous")),
      playBtn,
      iconBtn(ICONS.forward, t("Next"), () => void Bridge.spotifyControl("next")),
    ),
  );
  player.addEventListener("click", () => {
    if (!Spotify.state.source || Spotify.state.source === "Spotify") void Bridge.spotifyOpen();
  });
  const grid = h("div", { class: "home-grid" });
  const dots = h("div", { class: "home-dots" });
  const right = h("div", { class: "home-card home-right" }, player, grid, dots);
  let page = 0;
  let pageCount = 1;
  right.addEventListener("wheel", (e) => {
    if (pageCount < 2) return;
    e.preventDefault();
    page = (page + (e.deltaY > 0 || e.deltaX > 0 ? 1 : pageCount - 1)) % pageCount;
    lastGrid = "";
    draw();
  }, { passive: false });

  const hold = new PlayerHold();
  let holdTimer: number | null = null;
  let progressAt = 0;
  let lastGrid = "";
  let lastLayout = "";
  let lastPlayer = "";

  function drawPlayer() {
    const s = Spotify.state;
    const track = s.track;
    const key = `${track?.id}|${s.playing}|${s.source}|${currentArtwork() ? 1 : 0}`;
    if (key === lastPlayer) return;
    lastPlayer = key;
    pTitle.textContent = track?.title ?? "";
    const source = s.source && s.source !== "Spotify" ? s.source : "";
    pArtist.textContent = [isAd(track) ? t("Advertisement") : track?.artist ?? "", source].filter(Boolean).join(" · ");
    const cover = currentArtwork();
    clear(art);
    if (cover) art.append(h("img", { src: cover, alt: "" }));
    else art.append(svg(ICONS.musicNote, 16));
    playBtn.replaceChildren(svg(s.playing ? ICONS.pause : ICONS.play, 11));
    playBtn.title = s.playing ? t("Pause") : t("Play");
    player.title = source ? source : t("Open Spotify");
  }

  function drawProgress(now: number) {
    const s = Spotify.state;
    const d = s.track?.duration ?? 0;
    const p = spotifyPosition(s, now);
    bar.style.transform = `scaleX(${d > 0 ? Math.min(1, p / d) : 0})`;
    elapsed.textContent = formatTime(p);
    total.textContent = d > 0 ? formatTime(d) : "";
  }

  function draw() {
    const playing = right.classList.contains("playing");
    const declared = sanitizeDeclared(State.settings, State.os);
    let cells = withServices(currentTiles(State.settings), declared.activeIntegrations, declared.mainPill).tiles;
    if (playing) cells = cellsUnderPlayer(cells);
    const pages = pagesOf(cells, playing ? CELLS_E : CELLS_F, ADD);
    pageCount = pages.length;
    page = Math.min(page, pageCount - 1);
    const faces = pages[page].map((c) => (c === ADD ? "+" : c.kind === "service" ? `${c.id}:${serviceLine(c.target).text}` : c.kind === "music" ? `music:${Spotify.state.playing}` : `${c.id}:${face(c, actions.setView).sub}`));
    const key = `${playing}|${page}/${pageCount}|${faces.join("|")}`;
    if (key === lastGrid) return;
    lastGrid = key;
    // The tiles themselves changed (not just their text): they grow in (round 3B item 8).
    const layout = `${playing}|${page}|${pages[page].map((c) => (c === ADD ? "+" : c.id)).join(",")}`;
    if (layout !== lastLayout) {
      lastLayout = layout;
      grid.classList.remove("entering");
      void grid.offsetWidth;
      grid.classList.add("entering");
      window.setTimeout(() => grid.classList.remove("entering"), 500);
    }
    clear(grid);
    for (const c of pages[page]) grid.append(cell(c, actions.setView));
    clear(dots);
    dots.hidden = pageCount < 2;
    for (let i = 0; i < pageCount; i++) {
      const dot = h("button", { class: i === page ? "on" : "", "aria-label": t("Page {n}", { n: i + 1 }) });
      dot.addEventListener("click", (e) => {
        e.stopPropagation();
        page = i;
        lastGrid = "";
        draw();
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
      const playing = hold.update(Spotify.state.playing && !!Spotify.state.track, now);
      right.classList.toggle("playing", playing);
      if (playing) {
        drawPlayer();
        drawProgress(now);
      }
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
      // The player's clock, four times a second while music plays.
      if (!Spotify.state.playing || !right.classList.contains("playing")) return false;
      if (now - progressAt >= 250) {
        progressAt = now;
        drawProgress(Date.now());
      }
      return true;
    },
  };
}
