// Tiles that expand inside the island (round 3D item 8, approved design
// docs/design/settings-v2/IslandTiles): Spotify grows to 4 tiles, Mail, This
// PC and Reminders to 6, with the music strip beside them while something
// plays, and Mail to all 8. Each has a Back button; Esc or a click away folds
// it (island.ts). Nothing here reads or polls while its tile is folded: This
// PC reads the computer every 2 s only while it is open, and a message's text
// is fetched only when it is opened.

import { Bridge, type MailInbox, type MailMessage, type MemoryHog, type SystemStatus } from "../core/bridge";
import { Spotify, currentArtwork, isAd, spotifyPosition } from "../core/spotify";
import { State } from "../core/state";
import { t } from "../i18n/i18n";
import { clear, h, svg } from "./dom";
import { ICONS } from "./icons";
import { SYSTEM_EVERY_MS, bytes, connectAccount, mailTime, remindersBody, speed } from "./panel";
import { seek, setFlag, togglePlay } from "./spotify";
import { Mail } from "./tiles";
import { errorText, showError } from "./toast";

/** Groovy's own limit, shown against what he uses (round 3D item 7). */
export const GROOVY_LIMIT = 400 * 1024 * 1024;

export interface Grown {
  el: HTMLElement;
  /** It grew (true) or folded away (false). */
  shown(on: boolean): void;
  sync?(): void;
}

// ── Pieces ────────────────────────────────────────────────────────────────────

function backButton(label: string, onclick: () => void): HTMLElement {
  const b = h("button", { class: "grow-back", type: "button" }, svg(ICONS.chevronLeft, 13, { stroke: 2.4 }), h("span", { text: label }));
  b.addEventListener("click", (e) => {
    e.stopPropagation();
    onclick();
  });
  return b;
}

function roundButton(path: string, label: string, size: number, icon: number, onclick: () => void, cls = ""): HTMLButtonElement {
  const b = h("button", { class: `grow-round ${cls}`.trim(), type: "button", title: label, "aria-label": label, style: `width:${size}px;height:${size}px` }, svg(path, icon)) as HTMLButtonElement;
  b.addEventListener("click", (e) => {
    e.stopPropagation();
    onclick();
  });
  return b;
}

/** Play, or open Spotify when it isn't running (as the tile always did). */
export function playOrOpen() {
  if (!Spotify.state.running) {
    void Bridge.spotifyOpen();
    return;
  }
  togglePlay();
}

function artInto(box: HTMLElement) {
  const cover = currentArtwork();
  clear(box);
  if (cover) box.append(h("img", { src: cover, alt: "" }));
  else box.append(svg(ICONS.musicNote, 18));
}

// ── Spotify: 2 × 2 ────────────────────────────────────────────────────────────

/** Art, song and artist, a bar you can scrub, ⏮ 40 px, ⏯ 52 px, ⏭ 40 px, shuffle and repeat. */
export function musicGrown(back: () => void): Grown {
  const art = h("div", { class: "grow-art" });
  const title = h("b", {});
  const artist = h("span", {});
  const fill = h("i", {});
  const bar = h("div", { class: "grow-scrub", role: "slider", tabindex: "0", "aria-label": t("Position") }, fill);
  const play = roundButton(ICONS.play, t("Play"), 52, 22, playOrOpen, "main");
  const shuffle = roundButton(ICONS.shuffle, t("Shuffle"), 28, 13, () => setFlag("shuffle", !Spotify.state.shuffle), "flag");
  const repeat = roundButton(ICONS.repeat, t("Repeat"), 28, 13, () => setFlag("repeat", !Spotify.state.repeat), "flag");
  const scrubTo = (x: number) => {
    const d = Spotify.state.track?.duration ?? 0;
    const r = bar.getBoundingClientRect();
    if (d > 0 && r.width > 0) seek(Math.min(1, Math.max(0, (x - r.left) / r.width)) * d);
  };
  bar.addEventListener("pointerdown", (e) => {
    e.stopPropagation();
    scrubTo(e.clientX);
    const move = (m: PointerEvent) => scrubTo(m.clientX);
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  });
  bar.addEventListener("keydown", (e) => {
    const s = Spotify.state;
    const step = e.key === "ArrowRight" ? 5 : e.key === "ArrowLeft" ? -5 : 0;
    if (step) seek(spotifyPosition(s, Date.now()) + step);
  });
  const el = h("div", { class: "grow-panel grow-music" },
    h("div", { class: "grow-now" }, art, h("div", { class: "grow-song" }, title, artist)),
    bar,
    h("div", { class: "grow-controls" },
      roundButton(ICONS.backward, t("Previous"), 40, 18, () => void Bridge.spotifyControl("previous")),
      play,
      roundButton(ICONS.forward, t("Next"), 40, 18, () => void Bridge.spotifyControl("next")),
    ),
    h("div", { class: "grow-foot" }, backButton(t("Back"), back), h("span", { class: "grow-gap" }), shuffle, repeat),
  );
  let drawn = "";
  return {
    el,
    shown() {
      drawn = "";
    },
    sync() {
      const s = Spotify.state;
      const key = `${s.track?.id}|${s.playing}|${s.shuffle}|${s.repeat}|${currentArtwork() ? 1 : 0}|${s.running}`;
      if (key !== drawn) {
        drawn = key;
        artInto(art);
        title.textContent = s.track ? (isAd(s.track) ? t("Advertisement") : s.track.title) : s.running ? t("Nothing playing") : "Spotify";
        artist.textContent = s.track && !isAd(s.track) ? s.track.artist : s.running ? "" : t("Open Spotify");
        play.replaceChildren(svg(s.playing ? ICONS.pause : ICONS.play, 22));
        play.title = s.playing ? t("Pause") : t("Play");
        play.setAttribute("aria-label", play.title);
        shuffle.classList.toggle("on", !!s.shuffle);
        repeat.classList.toggle("on", !!s.repeat);
      }
      const d = s.track?.duration ?? 0;
      fill.style.transform = `scaleX(${d > 0 ? Math.min(1, spotifyPosition(s, Date.now()) / d) : 0})`;
    },
  };
}

/** The 2-tile strip in the fourth column: art, title, ⏯ 44 px, ⏮ ⏭ 34 px. */
export function musicStrip(): Grown {
  const art = h("div", { class: "grow-art small" });
  const title = h("span", { class: "grow-strip-title" });
  const play = roundButton(ICONS.play, t("Play"), 44, 18, playOrOpen, "main");
  const el = h("div", { class: "grow-panel grow-strip" },
    art,
    title,
    play,
    h("div", { class: "grow-strip-ctl" },
      roundButton(ICONS.backward, t("Previous"), 34, 15, () => void Bridge.spotifyControl("previous")),
      roundButton(ICONS.forward, t("Next"), 34, 15, () => void Bridge.spotifyControl("next")),
    ),
  );
  let drawn = "";
  return {
    el,
    shown() {
      drawn = "";
    },
    sync() {
      const s = Spotify.state;
      const key = `${s.track?.id}|${s.playing}|${currentArtwork() ? 1 : 0}`;
      if (key === drawn) return;
      drawn = key;
      artInto(art);
      title.textContent = s.track ? (isAd(s.track) ? t("Advertisement") : s.track.title) : "Spotify";
      play.replaceChildren(svg(s.playing ? ICONS.pause : ICONS.play, 18));
      play.title = s.playing ? t("Pause") : t("Play");
      play.setAttribute("aria-label", play.title);
    },
  };
}

// ── Mail: inbox → message → reply ─────────────────────────────────────────────

const AVATAR = ["#F5B84B", "#C9D1CD", "#7FC8FF", "#B9A2FF", "#7FF0A4", "#FF9C8A"];

function avatarColor(name: string): string {
  let n = 0;
  for (const c of name) n = (n * 31 + c.charCodeAt(0)) >>> 0;
  return AVATAR[n % AVATAR.length];
}

/** The web mail a message opens in, by account. */
export function mailApp(kind: string): string {
  return kind === "gmail" ? "Gmail" : kind === "outlook" ? "Outlook" : kind === "icloud" ? "iCloud Mail" : t("the browser");
}

/** Every connected inbox's newest messages, newest first. */
export function inboxRows(inboxes: MailInbox[]): { m: MailMessage; inbox: MailInbox }[] {
  return inboxes.flatMap((inbox) => inbox.messages.map((m) => ({ m, inbox }))).sort((a, b) => b.m.at - a.m.at);
}

export function mailGrown(back: () => void, toggleFull: () => void): Grown {
  type Step = { at: "inbox" } | { at: "read"; m: MailMessage; inbox: MailInbox } | { at: "reply"; m: MailMessage; inbox: MailInbox };
  let step: Step = { at: "inbox" };
  const head = h("div", { class: "grow-head" });
  const body = h("div", { class: "grow-body" });
  const note = h("div", { class: "grow-note" });
  const el = h("div", { class: "grow-panel grow-mail" }, head, body, note);
  let opened: { key: string; from: string; subject: string; text: string } | null = null;
  let draft = "";
  let drawn = "";

  const go = (next: Step) => {
    step = next;
    note.textContent = "";
    drawn = "";
    draw();
  };
  const unread = () => Mail.inboxes.reduce((n, i) => n + i.unread, 0);

  function headFor(title: string, backTo: () => void, backLabel: string) {
    clear(head);
    const size = h("button", { class: "grow-back grow-size", type: "button", text: State.mailFull ? t("6 tiles") : t("All 8"), "aria-label": t("Change mail size") });
    size.addEventListener("click", (e) => {
      e.stopPropagation();
      toggleFull();
    });
    head.append(backButton(backLabel, backTo), h("b", { class: "grow-title", text: title }), size);
  }

  async function open(m: MailMessage, inbox: MailInbox) {
    go({ at: "read", m, inbox });
    const key = `${m.account}:${m.id}`;
    if (opened?.key === key) return;
    opened = { key, from: m.from, subject: m.subject, text: m.snippet };
    draw();
    try {
      const r = await Bridge.mailRead(m.account, m.id);
      if (opened?.key !== key) return;
      opened = { key, from: r.from || m.from, subject: r.subject || m.subject, text: r.text || m.snippet };
      drawn = "";
      draw();
      // Read here, so read there too.
      if (m.unread && inbox.canModify) void Bridge.mailAct(m.account, m.id, "read").catch(() => {});
    } catch (e) {
      note.textContent = errorText(e);
    }
  }

  function inbox() {
    headFor(unread() ? t("Inbox · {n} unread", { n: unread() }) : t("Inbox"), back, t("Back"));
    if (!Mail.inboxes.length) {
      // No account yet: the mail Windows showed, and the connect buttons.
      for (const n of Mail.notes.slice(0, 3)) {
        body.append(h("button", { class: "grow-row", type: "button", onclick: () => void Bridge.mailOpen(n.appId).catch(showError) },
          h("span", { class: "grow-avatar", style: `background:${avatarColor(n.app)}`, text: (n.title || n.app || "?")[0].toUpperCase() }),
          h("span", { class: "grow-row-text" }, h("b", { text: n.title || n.app }), h("span", { text: n.body }))));
      }
      if (!Mail.notes.length) body.append(h("div", { class: "grow-empty", text: t("No new mail. Connect an account to see your inbox here.") }));
      body.append(h("div", { class: "grow-actions" },
        ...([["gmail", "Gmail"], ["outlook", "Outlook"], ["icloud", "iCloud"], ["imap", "IMAP"]] as const).map(([kind, name]) =>
          h("button", { class: "grow-btn", type: "button", text: name, title: t("Connect {name}", { name }),
            onclick: () => {
              if (kind === "gmail" || kind === "outlook") note.textContent = t("Finish signing in in your browser…");
              void connectAccount(kind).then(() => {
                note.textContent = "";
                void Bridge.mailRefresh();
              }, (e) => (note.textContent = errorText(e)));
            } }))));
      return;
    }
    const failing = Mail.inboxes.find((i) => i.error);
    if (failing) note.textContent = `${failing.account.email}: ${failing.error}`;
    for (const { m, inbox: box } of inboxRows(Mail.inboxes).slice(0, 8)) {
      body.append(h("button", { class: m.unread ? "grow-row unread" : "grow-row", type: "button", title: m.snippet || m.subject, onclick: () => void open(m, box) },
        h("span", { class: "grow-avatar", style: `background:${avatarColor(m.from)}`, text: (m.from || "?")[0].toUpperCase() }),
        h("span", { class: "grow-row-text" },
          h("span", { class: "grow-row-top" }, h("b", { text: m.from }), h("span", { class: "grow-time", text: mailTime(m.at) })),
          h("span", { text: m.subject || t("(no subject)") }))));
    }
  }

  function read(m: MailMessage, box: MailInbox) {
    headFor(opened?.from ?? m.from, () => go({ at: "inbox" }), t("Inbox"));
    const actions = h("div", { class: "grow-actions" },
      h("button", { class: "grow-btn primary", type: "button", text: t("Reply"), onclick: () => go({ at: "reply", m, inbox: box }) }),
      box.canModify
        ? h("button", { class: "grow-btn", type: "button", text: t("Archive"),
          onclick: () => void Bridge.mailAct(m.account, m.id, "archive").then(() => go({ at: "inbox" }), (e) => (note.textContent = errorText(e))) })
        : null,
      m.link || box.account.kind !== "imap"
        ? h("button", { class: "grow-btn", type: "button", text: t("Open in {app}", { app: mailApp(box.account.kind) }),
          onclick: () => void Bridge.mailOpenMessage(m.account, m.id).catch((e) => (note.textContent = errorText(e))) })
        : null,
    );
    body.append(
      h("b", { class: "grow-subject", text: opened?.subject || m.subject || t("(no subject)") }),
      h("div", { class: "grow-text", text: opened?.text ?? m.snippet }),
      actions,
    );
  }

  function reply(m: MailMessage, box: MailInbox) {
    headFor(t("Reply"), () => go({ at: "read", m, inbox: box }), t("Inbox"));
    const area = h("textarea", { class: "grow-reply", "aria-label": t("Reply to {name}", { name: m.from }) }) as HTMLTextAreaElement;
    area.value = draft;
    area.addEventListener("input", () => (draft = area.value));
    area.addEventListener("keydown", (e) => e.stopPropagation());
    const send = h("button", { class: "grow-btn send", type: "button", text: t("Send") }) as HTMLButtonElement;
    const polish = h("button", { class: "grow-btn", type: "button", text: t("Groovy, polish it") }) as HTMLButtonElement;
    send.addEventListener("click", async () => {
      send.disabled = true;
      note.textContent = t("Sending…");
      try {
        await Bridge.mailReply(m.account, m.id, area.value);
        draft = "";
        go({ at: "inbox" });
        note.textContent = t("Sent");
      } catch (e) {
        note.textContent = errorText(e);
        send.disabled = false;
      }
    });
    polish.addEventListener("click", async () => {
      if (!area.value.trim()) return;
      polish.disabled = true;
      note.textContent = t("Polishing…");
      try {
        area.value = draft = await Bridge.mailPolish(area.value);
        note.textContent = "";
      } catch (e) {
        note.textContent = errorText(e);
      }
      polish.disabled = false;
    });
    body.append(h("label", { class: "grow-label", text: t("Reply to {name}", { name: m.from }) }), area, h("div", { class: "grow-actions" }, send, polish));
    queueMicrotask(() => area.focus());
  }

  function draw() {
    const key = `${step.at}|${State.mailFull}|${opened?.key}|${opened?.text.length}|` + JSON.stringify(Mail.inboxes.map((i) => [i.account.id, i.unread, i.error, i.messages.map((m) => [m.id, m.unread])])) + Mail.notes.length;
    if (key === drawn) return;
    drawn = key;
    // The reply box keeps its focus and text while the inbox refreshes behind it.
    if (step.at === "reply" && body.querySelector("textarea")) return;
    clear(body);
    el.dataset.step = step.at;
    if (step.at === "inbox") inbox();
    else if (step.at === "read") read(step.m, step.inbox);
    else reply(step.m, step.inbox);
  }

  return {
    el,
    shown(on) {
      step = { at: "inbox" };
      opened = null;
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

// ── This PC ───────────────────────────────────────────────────────────────────

export function meterRows(s: SystemStatus): { name: string; value: string; fill: number; color: string }[] {
  const used = s.memoryTotal - s.memoryAvailable;
  const disk = s.disks[0];
  const rows = [
    { name: t("Memory"), value: `${bytes(used)} / ${bytes(s.memoryTotal)}`, fill: s.memoryTotal ? used / s.memoryTotal : 0, color: "#7FC8FF" },
    { name: t("CPU"), value: `${Math.round(s.cpuUsage)}%`, fill: s.cpuUsage / 100, color: "#46EB78" },
  ];
  if (disk) rows.push({ name: t("Disk"), value: `${bytes(disk.total - disk.available)} / ${bytes(disk.total)}`, fill: disk.total ? (disk.total - disk.available) / disk.total : 0, color: "#F5B84B" });
  rows.push({ name: "Groovy", value: `${bytes(s.groovyMemory)} / ${bytes(GROOVY_LIMIT)}`, fill: s.groovyMemory / GROOVY_LIMIT, color: s.groovyMemory > GROOVY_LIMIT * 0.95 ? "#F0506E" : "#46EB78" });
  return rows.map((r) => ({ ...r, fill: Math.min(1, Math.max(0, r.fill)) }));
}

export function systemGrown(back: () => void): Grown {
  const status = h("span", { class: "grow-status" });
  const meters = h("div", { class: "grow-meters" });
  const hogs = h("div", { class: "grow-hogs" });
  const answer = h("div", { class: "grow-note" });
  const controlsBtn = h("button", { class: "grow-back grow-size", type: "button", text: t("Controls") });
  const head = h("div", { class: "grow-head" }, backButton(t("Back"), back), h("b", { class: "grow-title", text: t("This PC") }), status, controlsBtn);
  const stats = h("div", { class: "grow-body" }, meters, h("span", { class: "grow-sub", text: t("Top apps by memory") }), hogs);
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
      h("button", { class: "grow-btn primary", type: "button", text: t("Allow once"), onclick: () => decide("once") }),
      h("button", { class: "grow-btn", type: "button", text: t("Always"), onclick: () => decide("always") }),
      h("button", { class: "grow-btn", type: "button", text: t("Deny"), onclick: () => decide("deny") }),
    );
  };
  const act = (action: Parameters<typeof Bridge.pcDo>[0], value: number | null, app: string | null = null) =>
    void Bridge.pcDo(action, value, app).then((r) => say(r.answer, r.pending), (e) => say(errorText(e), null));

  // Volume, brightness, Wi-Fi, Bluetooth and Do not disturb (round 4 item 5) behind "Controls".
  const slider = (label: string, action: "volume" | "brightness") => {
    const input = h("input", { type: "range", min: "0", max: "100", step: "5", value: "50", "aria-label": label }) as HTMLInputElement;
    input.addEventListener("change", () => act(action, Number(input.value)));
    return h("label", { class: "grow-slider" }, h("span", { text: label }), input);
  };
  const onOff = (label: string, action: "wifi" | "bluetooth") =>
    h("div", { class: "grow-onoff" }, h("span", { text: label }),
      h("button", { class: "grow-btn", type: "button", text: t("On"), onclick: () => act(action, 1) }),
      h("button", { class: "grow-btn", type: "button", text: t("Off"), onclick: () => act(action, 0) }));
  const controls = h("div", { class: "grow-body grow-pc-controls" },
    slider(t("Volume"), "volume"),
    slider(t("Brightness"), "brightness"),
    onOff(t("Wi-Fi"), "wifi"),
    onOff(t("Bluetooth"), "bluetooth"),
    h("button", { class: "grow-btn", type: "button", text: t("Do not disturb"), onclick: () => act("dnd", null) }),
  );
  controls.hidden = true;
  controlsBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    controls.hidden = !controls.hidden;
    stats.hidden = !controls.hidden;
    controlsBtn.textContent = controls.hidden ? t("Controls") : t("Status");
  });

  function drawStats(s: SystemStatus, rate: { down: number; up: number } | null) {
    clear(meters);
    for (const r of meterRows(s)) {
      meters.append(h("div", { class: "grow-meter" },
        h("div", { class: "grow-meter-top" }, h("b", { text: r.name }), h("span", { text: r.value })),
        h("div", { class: "grow-bar" }, h("i", { style: `transform:scaleX(${r.fill.toFixed(3)});background:${r.color}` }))));
    }
    const parts = [
      s.battery ? t("Battery {pct}%", { pct: s.battery.percent }) + (s.battery.charging ? " ⚡" : "") : null,
      rate ? `↓ ${bytes(rate.down)}/s` : null,
    ].filter(Boolean);
    status.textContent = parts.join(" · ");
  }

  function drawHogs(list: MemoryHog[]) {
    clear(hogs);
    for (const hog of list.slice(0, 3)) {
      hogs.append(h("div", { class: "grow-hog" },
        h("b", { text: hog.name }),
        h("span", { text: bytes(hog.bytes) }),
        // close_app always asks first (pc.rs): the answer brings Allow / Deny.
        h("button", { class: "grow-btn", type: "button", text: t("End"), onclick: () => act("end", null, hog.name) }),
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
    el: h("div", { class: "grow-panel grow-pc" }, head, stats, controls, answer),
    shown(on) {
      if (timer !== null) window.clearInterval(timer);
      timer = null;
      if (!on) return;
      last = null;
      clear(answer);
      controls.hidden = true;
      stats.hidden = false;
      controlsBtn.textContent = t("Controls");
      timer = window.setInterval(() => void read(), SYSTEM_EVERY_MS);
      void read();
    },
  };
}

// ── Reminders ─────────────────────────────────────────────────────────────────

export function remindersGrown(back: () => void): Grown {
  const inner = remindersBody(back);
  const head = h("div", { class: "grow-head" }, backButton(t("Back"), back), h("b", { class: "grow-title", text: t("Reminders") }));
  return {
    el: h("div", { class: "grow-panel grow-reminders" }, head, h("div", { class: "grow-body" }, inner.el)),
    shown: (on) => inner.shown(on),
  };
}
