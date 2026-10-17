// A tile opened in the island (round 3C item 4, "soft settings"): Reminders,
// This PC, Mail and Spotify unfold into one wide panel over the tiles, with a
// back arrow, Esc or a click away to fold back, and "All settings" at the
// bottom for the deep setup. Nothing here polls while the panel is shut; This
// PC reads the computer every 2 s only while it is open.

import { Bridge, type MailMessage, type MemoryHog, type SystemStatus, type TaskItem } from "../core/bridge";
import { State } from "../core/state";
import { t } from "../i18n/i18n";
import { clear, h, svg } from "./dom";
import { ICONS } from "./icons";
import { buildSpotifyCard } from "./spotify";
import { Mail, forgetSlowData } from "./tiles";
import { errorText, showError } from "./toast";
import type { ViewActions, ViewHost } from "./views";

export type PanelName = "reminders" | "system" | "mail" | "spotify";

/** How often This PC reads the computer while its panel is open. */
export const SYSTEM_EVERY_MS = 2000;

/** Settings pages behind "All settings". */
const DEEP: Record<PanelName, string> = { reminders: "reminders", system: "system", mail: "mail", spotify: "island" };

interface Body {
  el: HTMLElement;
  title(): string;
  /** The panel came into view (true) or folded away (false). */
  shown(on: boolean): void;
  sync?(): void;
}

export function buildPanel(actions: ViewActions): ViewHost {
  const back = () => actions.setView("home");
  const bodies: Record<PanelName, Body> = {
    reminders: reminders(back),
    system: system(),
    mail: mail(),
    spotify: spotify(),
  };
  const title = h("div", { class: "panel-title" });
  const slot = h("div", { class: "panel-body" });
  const deep = h("button", {
    class: "panel-deep",
    text: t("All settings →"),
    onclick: () => void Bridge.openSettingsWindow(DEEP[State.panel]),
  });
  const el = h("div", { class: "view panel-view" },
    h("div", { class: "panel-card" },
      h("div", { class: "panel-head" },
        h("button", { class: "panel-back", "aria-label": t("Back"), onclick: back }, svg(ICONS.chevronLeft, 18, { stroke: 2.2 })),
        title,
      ),
      slot,
      deep,
    ),
  );

  let open: PanelName | null = null;
  return {
    el,
    sync() {
      const now = State.mode === "expanded" && State.view === "panel" ? State.panel : null;
      if (now !== open) {
        if (open) bodies[open].shown(false);
        open = now;
        if (open) {
          clear(slot);
          slot.append(bodies[open].el);
          el.dataset.panel = open;
          // Unfolds out of the tile each time (style.css: .panel-card.unfold).
          el.firstElementChild?.classList.remove("unfold");
          void (el as HTMLElement).offsetWidth;
          el.firstElementChild?.classList.add("unfold");
          bodies[open].shown(true);
        }
      }
      if (open) {
        title.textContent = bodies[open].title();
        bodies[open].sync?.();
      }
    },
  };
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

function reminders(back: () => void): Body {
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

function system(): Body {
  const stats = h("div", { class: "panel-stats" });
  const hogs = h("div", { class: "panel-list panel-hogs" });
  const answer = h("div", { class: "panel-hint" });
  let timer: number | null = null;
  let last: SystemStatus | null = null;
  let lastAt = 0;

  const say = (text: string, pending: { id: number; label: string } | null) => {
    clear(answer);
    answer.append(h("span", { text: pending ? pending.label : text }));
    if (!pending) return;
    // Round 4 item 5's permissions: the island asks before it acts.
    const decide = (d: "once" | "always" | "deny") => void Bridge.toolsDecide(pending.id, d).then((a) => say(a, null), showError);
    answer.append(
      h("button", { class: "btn primary", text: t("Allow once"), onclick: () => decide("once") }),
      h("button", { class: "btn secondary", text: t("Always"), onclick: () => decide("always") }),
      h("button", { class: "btn secondary", text: t("Deny"), onclick: () => decide("deny") }),
    );
  };
  const act = (action: Parameters<typeof Bridge.pcDo>[0], value: number | null, app: string | null = null) =>
    void Bridge.pcDo(action, value, app).then((r) => say(r.answer, r.pending), (e) => say(errorText(e), null));

  const slider = (label: string, action: "volume" | "brightness") => {
    const input = h("input", { type: "range", min: "0", max: "100", step: "5", value: "50", "aria-label": label }) as HTMLInputElement;
    input.addEventListener("change", () => act(action, Number(input.value)));
    return h("label", { class: "panel-slider" }, h("span", { text: label }), input);
  };
  const toggle = (label: string, action: "wifi" | "bluetooth") =>
    h("div", { class: "panel-toggle" }, h("span", { text: label }),
      h("button", { class: "btn secondary", text: t("On"), onclick: () => act(action, 1) }),
      h("button", { class: "btn secondary", text: t("Off"), onclick: () => act(action, 0) }));

  const controls = h("div", { class: "panel-controls" },
    slider(t("Volume"), "volume"),
    slider(t("Brightness"), "brightness"),
    h("div", { class: "panel-toggles" }, toggle(t("Wi-Fi"), "wifi"), toggle(t("Bluetooth"), "bluetooth")),
    h("button", { class: "btn secondary panel-dnd", text: t("Do not disturb"), onclick: () => act("dnd", null) }),
  );

  function drawStats(s: SystemStatus, rate: { down: number; up: number } | null) {
    clear(stats);
    const memUsed = s.memoryTotal - s.memoryAvailable;
    const disk = s.disks[0];
    const stat = (label: string, value: string) => h("div", { class: "panel-stat" }, h("b", { text: value }), h("span", { text: label }));
    const cells: (HTMLElement | null)[] = [
      stat(t("CPU"), `${Math.round(s.cpuUsage)} %`),
      stat(t("Memory"), `${bytes(memUsed)} / ${bytes(s.memoryTotal)}`),
      disk ? stat(t("Disk {mount}", { mount: disk.mount }), t("{n} free", { n: bytes(disk.available) })) : null,
      s.battery ? stat(t("Battery"), `${s.battery.percent} %${s.battery.charging ? " ⚡" : ""}`) : null,
      stat(t("Network"), rate ? `↓ ${bytes(rate.down)}/s ↑ ${bytes(rate.up)}/s` : "…"),
    ];
    stats.append(...cells.filter((c): c is HTMLElement => c !== null));
  }

  function drawHogs(list: MemoryHog[]) {
    clear(hogs);
    hogs.append(h("div", { class: "panel-sub", text: t("Top apps by memory") }));
    for (const hog of list.slice(0, 4)) {
      hogs.append(h("div", { class: "panel-row" },
        h("span", { class: "panel-text", text: hog.name }),
        h("span", { class: "panel-due", text: bytes(hog.bytes) }),
        // close_app always asks first (pc.rs): the answer brings Allow / Deny.
        h("button", { class: "btn secondary", text: t("End"), onclick: () => act("end", null, hog.name) }),
      ));
    }
  }

  async function read() {
    const s = await Bridge.systemStatus();
    if (!s || timer === null) return;
    const now = performance.now();
    drawStats(s, speed(last, s, now - lastAt));
    last = s;
    lastAt = now;
    const list = await Bridge.memoryHogs();
    if (list && timer !== null) drawHogs(list);
  }

  return {
    el: h("div", { class: "panel-system" }, stats, h("div", { class: "panel-split" }, controls, hogs), answer),
    title: () => t("This PC"),
    shown(on) {
      if (timer !== null) window.clearInterval(timer);
      timer = null;
      if (!on) return;
      last = null;
      clear(answer);
      timer = window.setInterval(() => void read(), SYSTEM_EVERY_MS);
      void read();
    },
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

function mail(): Body {
  const list = h("div", { class: "panel-list" });
  const note = h("div", { class: "panel-hint" });
  const connect = h("div", { class: "panel-connect" },
    ...([["gmail", "Gmail"], ["outlook", "Outlook"], ["icloud", "iCloud"], ["imap", "IMAP"]] as const).map(([kind, name]) =>
      h("button", {
        class: "btn secondary",
        text: t("Connect {name}", { name }),
        onclick: () => {
          if (kind === "gmail" || kind === "outlook") note.textContent = t("Finish signing in in your browser…");
          void connectAccount(kind).then(() => {
            note.textContent = "";
            void Bridge.mailRefresh();
          }, (e) => (note.textContent = errorText(e)));
        },
      })),
  );
  const act = (m: MailMessage, action: "read" | "archive") =>
    void Bridge.mailAct(m.account, m.id, action).catch((e) => (note.textContent = errorText(e)));
  let drawn = "";

  function draw() {
    const key = JSON.stringify(Mail.inboxes.map((i) => [i.account.id, i.unread, i.error, i.messages.map((m) => [m.id, m.unread])])) + Mail.notes.length;
    if (key === drawn) return;
    drawn = key;
    clear(list);
    if (!Mail.inboxes.length) {
      // No account yet: the mail Windows showed, and the connect buttons, not Settings.
      if (!Mail.notes.length) list.append(h("div", { class: "panel-empty", text: t("No new mail. Connect an account to see your inbox here.") }));
      for (const n of Mail.notes) {
        list.append(h("button", { class: "panel-row panel-mail", onclick: () => void Bridge.mailOpen(n.appId).catch(showError) },
          h("b", { text: n.title || n.app }),
          h("span", { class: "panel-text", text: n.body }),
          h("span", { class: "panel-due", text: n.app }),
        ));
      }
      connect.style.display = "";
      return;
    }
    connect.style.display = "none";
    for (const inbox of Mail.inboxes) {
      list.append(h("div", { class: "panel-sub panel-account" },
        h("span", { text: inbox.account.email }),
        h("span", { class: "panel-due", text: inbox.unread ? t("{n} unread", { n: inbox.unread }) : t("All read") }),
        inbox.error ? h("span", { class: "panel-warn", text: inbox.error }) : null,
      ));
      for (const m of inbox.messages.slice(0, 5)) {
        list.append(h("div", { class: m.unread ? "panel-row panel-mail unread" : "panel-row panel-mail" },
          h("button", {
            class: "panel-text",
            title: m.snippet || m.subject,
            onclick: () => void Bridge.mailOpenMessage(m.account, m.id).catch((e) => (note.textContent = errorText(e))),
          }, h("b", { text: m.from }), h("span", { text: m.subject || t("(no subject)") })),
          h("span", { class: "panel-due", text: mailTime(m.at) }),
          m.unread && inbox.canModify ? h("button", { class: "panel-icon", "aria-label": t("Mark read"), title: t("Mark read"), onclick: () => act(m, "read") }, svg(ICONS.check, 14, { stroke: 2.2 })) : null,
          inbox.canModify ? h("button", { class: "panel-icon", "aria-label": t("Archive"), title: t("Archive"), onclick: () => act(m, "archive") }, svg(ICONS.xmark, 11)) : null,
        ));
      }
    }
  }
  return {
    el: h("div", { class: "panel-mailbox" }, list, connect, note),
    title: () => t("Mail"),
    shown(on) {
      drawn = "";
      note.textContent = "";
      if (on) {
        draw();
        // Fresh when it opens; the 3-minute check keeps it current after that.
        if (Mail.inboxes.length) void Bridge.mailRefresh();
      }
    },
    sync: draw,
  };
}

// ── Spotify ───────────────────────────────────────────────────────────────────

function spotify(): Body {
  const card = buildSpotifyCard();
  return {
    el: h("div", { class: "panel-spotify" }, card.el),
    title: () => t("Spotify"),
    shown() {},
    sync: () => card.sync(),
  };
}
