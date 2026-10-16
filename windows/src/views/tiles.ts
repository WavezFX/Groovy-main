// "My island" on the island (FIXES_ROUND1 item 4): the tiles the user chose in
// Control Centre → My island — their apps, websites and folders, services,
// mail, music, agents, reminders and the PC's state — and the pop-up for a new
// mail read from Windows' notifications. Groovy sits at the left as on every view.

import { Bridge } from "../core/bridge";
import type { IslandViewName } from "../core/layout";
import { pillDefinition } from "../core/pills";
import { Spotify } from "../core/spotify";
import { State } from "../core/state";
import { currentTiles, type Tile } from "../core/tiles";
import { t } from "../i18n/i18n";
import { showError } from "./toast";
import { h, clear } from "./dom";
import { icon } from "../hub/ui";
import { Notices, noticeIcon } from "../island/live";
import type { ViewHost } from "./views";

/** A new mail from Windows' notification feed (island_tiles.rs). */
export interface MailNote {
  id: number;
  app: string;
  title: string;
  body: string;
  appId: string;
}

/** The newest mail first; at most five kept, in memory only. */
export const Mail = {
  notes: [] as MailNote[],
  push(n: MailNote) {
    this.notes = [n, ...this.notes.filter((x) => x.id !== n.id)].slice(0, 5);
  },
};


let system: { cpu: number; mem: number } | null = null;
let systemAt = 0;
let nextReminder: string | null = null;
let remindersAt = 0;

function refreshSlowData(onChange: () => void) {
  const now = Date.now();
  if (now - systemAt > 30_000) {
    systemAt = now;
    void Bridge.systemStatus().then((s) => {
      if (!s) return;
      system = { cpu: Math.round(s.cpuUsage), mem: Math.round((1 - s.memoryAvailable / Math.max(1, s.memoryTotal)) * 100) };
      onChange();
    });
  }
  if (now - remindersAt > 60_000) {
    remindersAt = now;
    void Bridge.taskList(false).then((list) => {
      const next = list.filter((x) => !x.done && x.dueAt).sort((a, b) => (a.dueAt ?? 0) - (b.dueAt ?? 0))[0];
      nextReminder = next ? next.text : null;
      onChange();
    }, () => {});
  }
}

function initial(label: string): string {
  return (label.trim()[0] ?? "?").toUpperCase();
}

interface TileFace {
  icon: string;
  color: string;
  title: string;
  sub: string;
  open: () => void;
}

function face(tile: Tile, setView: (v: IslandViewName) => void): TileFace {
  const label = tile.label || tile.target.split(/[\\/]/).pop()?.replace(/\.(lnk|exe|url)$/i, "") || "";
  switch (tile.kind) {
    case "app":
    case "site":
    case "folder":
      return {
        icon: tile.kind === "site" ? "🌐" : tile.kind === "folder" ? "📁" : initial(label),
        color: tile.kind === "site" ? "#3B9EFF" : tile.kind === "folder" ? "#F7B32B" : "#A78BFA",
        title: tile.kind === "site" ? (tile.label || tile.target.replace(/^https?:\/\//, "").split("/")[0]) : label,
        sub: tile.kind === "app" ? t("App") : tile.kind === "site" ? t("Website") : t("Folder"),
        open: () => void Bridge.islandLaunch(tile.kind, tile.target).catch(showError),
      };
    case "service": {
      const def = pillDefinition(tile.target);
      return {
        icon: initial(def?.name ?? tile.target),
        color: def?.color ?? "#8E939C",
        title: tile.label || def?.name || tile.target,
        sub: t("Service"),
        open: () => {
          State.focusId = tile.target;
          setView("overview");
        },
      };
    }
    case "mail": {
      const last = Mail.notes[0];
      return {
        icon: "✉",
        color: "#3B9EFF",
        title: last ? last.title || last.app : t("Mail"),
        sub: last ? last.body || last.app : t("New mail pops up here"),
        open: () => {
          if (last) void Bridge.mailOpen(last.appId).catch(showError);
          else void Bridge.openSettingsWindow("island");
        },
      };
    }
    case "music": {
      const s = Spotify.state;
      return {
        icon: "♪",
        color: "#1DB954",
        title: s.track ? s.track.title : t("Spotify"),
        sub: s.track ? s.track.artist : s.installed || s.running ? t("Play something in the Spotify app and it appears here") : t("Get Spotify"),
        // Opens Spotify, or its Microsoft Store page when it isn't installed (spotify.rs).
        open: () => void Bridge.spotifyOpen(),
      };
    }
    case "agents": {
      const busy = State.tasks.filter((x) => !x.isIntegration || x.state !== "idle").length;
      return {
        icon: "⌘",
        color: "#E07950",
        title: t("Agents"),
        sub: busy ? t("{n} at work", { n: busy }) : t("Claude Code, Codex…"),
        open: () => setView("overview"),
      };
    }
    case "reminders":
      return {
        icon: "⏰",
        color: "#F7B32B",
        title: t("Reminders"),
        sub: nextReminder ?? t("Nothing due"),
        open: () => void Bridge.openSettingsWindow("reminders"),
      };
    case "system":
      return {
        icon: "▤",
        color: "#34D399",
        title: t("This PC"),
        sub: system ? t("CPU {cpu} % · memory {mem} %", { cpu: system.cpu, mem: system.mem }) : "…",
        open: () => void Bridge.openSettingsWindow("system"),
      };
  }
}

export function buildTiles(setView: (v: IslandViewName) => void): ViewHost {
  const grid = h("div", { class: "tiles-grid" });
  const empty = h("div", { class: "tiles-empty" },
    h("div", { text: t("Your island is empty.") }),
    h("button", { class: "tile-add", text: t("Choose tiles"), onclick: () => void Bridge.openSettingsWindow("island") }),
  );
  const el = h("div", { class: "view tiles-view" }, grid, empty);
  let last = "";
  const draw = () => {
    const tiles = currentTiles(State.settings).tiles;
    const faces = tiles.map((tile) => ({ tile, f: face(tile, setView) }));
    const key = JSON.stringify(faces.map(({ tile, f }) => [tile.id, tile.size, f.title, f.sub]));
    if (key === last) return;
    last = key;
    clear(grid);
    empty.style.display = tiles.length ? "none" : "";
    for (const { tile, f } of faces) {
      const b = h("button", { class: `tile ${tile.size}`, title: `${f.title} — ${f.sub}`, onclick: f.open },
        h("span", { class: "tile-icon", style: `background:${f.color}`, text: f.icon }),
        h("span", { class: "tile-text" }, h("b", { text: f.title }), h("span", { text: f.sub })),
      );
      grid.append(b);
    }
  };
  return {
    el,
    sync() {
      refreshSlowData(() => {
        last = "";
        if (State.view === "tiles") draw();
      });
      draw();
    },
  };
}

/**
 * The pop-up for Groovy's own messages (flow/live.rs: what the bottom pill
 * used to say) and for a new mail: who or what, Open or the message's button,
 * and Dismiss. Groovy's messages come first.
 */
export function buildNotify(setView: (v: IslandViewName) => void, collapse: () => void): ViewHost {
  const who = h("b", {});
  const what = h("div", { class: "notify-body" });
  const app = h("div", { class: "notify-app" });
  const mark = h("span", { class: "notify-icon" });
  const open = h("button", { class: "primary", text: t("Open") });
  const dismiss = h("button", { text: t("Dismiss") });
  const after = () => {
    if (Notices.current || Mail.notes.length) setView("notify");
    else if (currentTiles(State.settings).preset !== "developer") setView("tiles");
    else collapse();
  };
  open.addEventListener("click", () => {
    const notice = Notices.current;
    if (notice) {
      Notices.act(notice.id);
      collapse();
      return;
    }
    const n = Mail.notes[0];
    if (n) void Bridge.mailOpen(n.appId).catch(showError);
    collapse();
  });
  dismiss.addEventListener("click", () => {
    const notice = Notices.current;
    if (notice) Notices.dismiss(notice.id);
    else Mail.notes.shift();
    after();
  });
  const el = h("div", { class: "view notify-view" },
    h("div", { class: "notify-card" }, app, who, what, h("div", { class: "notify-actions" }, open, dismiss)),
  );
  return {
    el,
    sync() {
      const notice = Notices.current;
      if (notice) {
        mark.replaceChildren(icon(noticeIcon(notice.icon)));
        mark.style.color = notice.tint;
        app.replaceChildren(mark, document.createTextNode("Groovy"));
        who.textContent = "";
        what.textContent = notice.text;
        what.classList.add("notice");
        open.textContent = notice.action ?? "";
        open.hidden = !notice.action;
        return;
      }
      const n = Mail.notes[0];
      what.classList.remove("notice");
      app.textContent = n ? `✉ ${n.app}` : "";
      who.textContent = n?.title ?? "";
      what.textContent = n?.body ?? "";
      open.textContent = t("Open");
      open.hidden = !n;
    },
  };
}

/** Whether a mail notification should pop up: the island has a Mail tile. */
export function wantsMail(): boolean {
  return currentTiles(State.settings).tiles.some((x) => x.kind === "mail");
}

