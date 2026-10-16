// "My island" tiles (FIXES_ROUND1 item 4): what each tile the user chose shows
// and opens — their apps, websites and folders, services, mail, music, agents,
// reminders and the PC's state — drawn in the home view's grid (views/home.ts),
// and the pop-up for Groovy's messages and a new mail.

import { Bridge } from "../core/bridge";
import type { IslandViewName } from "../core/layout";
import { pillDefinition } from "../core/pills";
import { Spotify } from "../core/spotify";
import { State } from "../core/state";
import { currentTiles, type Tile } from "../core/tiles";
import { t } from "../i18n/i18n";
import { showError } from "./toast";
import { h } from "./dom";
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
let nextReminder: { text: string; dueAt: number } | null = null;
let remindersAt = 0;

/** The PC's load and the next reminder, read at most every 30 s and 60 s. */
export function refreshSlowData(onChange: () => void) {
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
      nextReminder = next?.dueAt ? { text: next.text, dueAt: next.dueAt * 1000 } : null;
      onChange();
    }, () => {});
  }
}

function initial(label: string): string {
  return (label.trim()[0] ?? "?").toUpperCase();
}

/** The next reminder with a time, as last read. */
export function upcomingReminder(): { text: string; dueAt: number } | null {
  return nextReminder;
}

export interface TileFace {
  icon: string;
  color: string;
  title: string;
  sub: string;
  open: () => void;
}

export function face(tile: Tile, setView: (v: IslandViewName) => void): TileFace {
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
        sub: nextReminder?.text ?? t("Nothing due"),
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
    else setView("home");
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

