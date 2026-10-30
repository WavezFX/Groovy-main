// What a tile shows once it has grown in the island (round 3C item 4, then
// round 3D item 8's tiles that expand, views/expand.ts): the Reminders list,
// and the helpers This PC and Mail use. Nothing here polls on its own.

import { Bridge, type SystemStatus, type TaskItem } from "../core/bridge";
import { t } from "../i18n/i18n";
import { clear, h, svg } from "./dom";
import { ICONS } from "./icons";
import { forgetSlowData } from "./tiles";
import { errorText, showError } from "./toast";

/** How often This PC reads the computer while its panel is open. */
export const SYSTEM_EVERY_MS = 2000;

export interface Body {
  el: HTMLElement;
  title(): string;
  /** The panel came into view (true) or folded away (false). */
  shown(on: boolean): void;
  sync?(): void;
}

// ── Reminders ─────────────────────────────────────────────────────────────────

/** "Tue 09:00": the day and the time a reminder is due, in local time. */
export function dueLabel(dueAt: number, now = Date.now()): string {
  const d = new Date(dueAt * 1000);
  const today = new Date(now);
  const time = d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  if (d.toDateString() === today.toDateString()) return time;
  const tomorrow = new Date(now + 86_400_000);
  if (d.toDateString() === tomorrow.toDateString()) return t("Tomorrow {time}", { time });
  return `${d.toLocaleDateString([], { weekday: "short", day: "numeric", month: "short" })} ${time}`;
}

/** The value a datetime-local input wants, for Unix seconds. */
export function localInput(secs: number): string {
  const d = new Date(secs * 1000);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}T${p(d.getHours())}:${p(d.getMinutes())}`;
}

/** The reminders list with add, edit, done and delete (Reminders grown in the island). */
export function remindersBody(back: () => void): Body {
  const list = h("div", { class: "panel-list" });
  const field = h("input", { class: "panel-input", type: "text", maxlength: "300", placeholder: t("Add a reminder: “call mum tomorrow 9am”") }) as HTMLInputElement;
  const when = h("input", { class: "panel-when", type: "datetime-local" }) as HTMLInputElement;
  const hint = h("div", { class: "panel-hint" });
  const save = h("button", { class: "btn primary panel-save", text: t("Save") });
  let items: TaskItem[] = [];
  let editing: number | null = null;

  async function load() {
    try {
      items = (await Bridge.taskList(false)).filter((x) => !x.done)
        .sort((a, b) => (a.dueAt ?? Number.MAX_SAFE_INTEGER) - (b.dueAt ?? Number.MAX_SAFE_INTEGER));
    } catch {
      items = [];
    }
    draw();
  }

  function draw() {
    clear(list);
    if (!items.length) list.append(h("div", { class: "panel-empty", text: t("Nothing due. Add one below.") }));
    for (const item of items) {
      if (editing === item.id) {
        const text = h("input", { class: "panel-input", type: "text", maxlength: "300", value: item.text }) as HTMLInputElement;
        const due = h("input", { class: "panel-when shown", type: "datetime-local", value: item.dueAt ? localInput(item.dueAt) : "" }) as HTMLInputElement;
        const done = async () => {
          const at = due.value ? Math.floor(new Date(due.value).getTime() / 1000) : null;
          try {
            await Bridge.taskEdit(item.id, text.value, at);
            editing = null;
            forgetSlowData();
            await load();
          } catch (e) {
            showError(e);
          }
        };
        text.addEventListener("keydown", (e) => {
          if (e.key === "Enter") void done();
        });
        list.append(h("div", { class: "panel-row editing" }, text, due,
          h("button", { class: "panel-icon", "aria-label": t("Save"), onclick: () => void done() }, svg(ICONS.check, 16, { stroke: 2.2 }))));
        queueMicrotask(() => text.focus());
        continue;
      }
      list.append(h("div", { class: "panel-row" },
        h("button", {
          class: "panel-tick",
          "aria-label": t("Done"),
          onclick: () => void Bridge.taskSetDone(item.id, true).then(() => {
            forgetSlowData();
            return load();
          }, showError),
        }),
        h("button", {
          class: "panel-text",
          title: t("Edit"),
          onclick: () => {
            editing = item.id;
            draw();
          },
        }, h("span", { text: item.text }), item.dueAt ? h("span", { class: "panel-due", text: dueLabel(item.dueAt) }) : null),
        h("button", {
          class: "panel-icon",
          "aria-label": t("Delete"),
          onclick: () => void Bridge.taskDelete(item.id).then(() => {
            forgetSlowData();
            return load();
          }, showError),
        }, svg(ICONS.xmark, 12)),
      ));
    }
  }

  async function add() {
    const typed = field.value.trim();
    if (!typed) return;
    let text = typed;
    let dueAt: number | null = when.value ? Math.floor(new Date(when.value).getTime() / 1000) : null;
    if (dueAt === null) {
      const parsed = await Bridge.taskParse(typed);
      if (parsed) {
        text = parsed.text;
        dueAt = parsed.dueAt;
      } else if (!when.classList.contains("shown")) {
        // Not understood: the quick picker, then Save again.
        when.classList.add("shown");
        when.value = localInput(Math.floor(Date.now() / 1000) + 3600);
        hint.textContent = t("When? Pick a time, then Save.");
        when.focus();
        return;
      }
    }
    try {
      await Bridge.taskAdd(text, dueAt);
    } catch (e) {
      hint.textContent = errorText(e);
      return;
    }
    field.value = "";
    when.value = "";
    when.classList.remove("shown");
    hint.textContent = "";
    forgetSlowData();
    // Saved: it folds back, and the tile shows the next reminder.
    back();
  }
  save.addEventListener("click", () => void add());
  field.addEventListener("keydown", (e) => {
    if (e.key === "Enter") void add();
  });

  return {
    el: h("div", { class: "panel-reminders" }, list, h("div", { class: "panel-add" }, field, when, save), hint),
    title: () => t("Reminders"),
    shown(on) {
      editing = null;
      hint.textContent = "";
      if (on) {
        void load();
        queueMicrotask(() => field.focus());
      }
    },
  };
}

// ── This PC ───────────────────────────────────────────────────────────────────

export function bytes(n: number): string {
  if (n >= 1024 ** 3) return t("{n} GB", { n: (n / 1024 ** 3).toFixed(1) });
  if (n >= 1024 ** 2) return t("{n} MB", { n: Math.round(n / 1024 ** 2) });
  return t("{n} KB", { n: Math.max(0, Math.round(n / 1024)) });
}

/** Download and upload speed from two readings, bytes a second. */
export function speed(prev: SystemStatus | null, now: SystemStatus, ms: number): { down: number; up: number } | null {
  if (!prev || ms <= 0) return null;
  return {
    down: Math.max(0, now.netReceived - prev.netReceived) * 1000 / ms,
    up: Math.max(0, now.netSent - prev.netSent) * 1000 / ms,
  };
}

// ── Mail ──────────────────────────────────────────────────────────────────────

/** "09:05" today, "Tue" this week, else "17 Oct". */
export function mailTime(at: number, now = Date.now()): string {
  const d = new Date(at * 1000);
  if (d.toDateString() === new Date(now).toDateString()) return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  if (now - d.getTime() < 6 * 86_400_000) return d.toLocaleDateString([], { weekday: "short" });
  return d.toLocaleDateString([], { day: "numeric", month: "short" });
}

/** Gmail and Outlook sign in here; iCloud and IMAP need a password, in Settings → Mail. */
export async function connectAccount(kind: "gmail" | "outlook" | "icloud" | "imap"): Promise<void> {
  if (kind === "icloud" || kind === "imap") return void (await Bridge.openSettingsWindow("mail"));
  const ready = await Bridge.mailOauthReady();
  if (kind === "gmail" ? ready?.google : ready?.microsoft) await Bridge.mailConnectOauth(kind);
  else await Bridge.openSettingsWindow("mail");
}
