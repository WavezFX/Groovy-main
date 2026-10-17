// Settings → Memory: what Groovy remembers (memory.rs), kept apart from the
// chat history. Every entry can be read, edited, pinned or deleted; each store
// is cleared on its own; everything can be exported to a JSON file.

import { Bridge, type ConversationInfo, type MemoryEntry, type MemoryKind } from "../core/bridge";
import { DEFAULT_ASSISTANT, type AssistantPrefs, type Settings } from "../core/state";
import { clear, h } from "../views/dom";
import { N_, t } from "../i18n/i18n";
import { confirmClick } from "./parts";

type Toggle = (on: boolean, onChange: (v: boolean) => void) => HTMLElement;

export const KINDS: [MemoryKind, string][] = [
  ["preference", N_("Preference")],
  ["fact", N_("Fact")],
  ["project", N_("Project")],
  ["task", N_("Task")],
];

const RETENTION = [7, 30, 90, 365, 0];

function when(secs: number): string {
  return new Date(secs * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

function kindSelect(value: MemoryKind): HTMLSelectElement {
  const s = h("select", {}) as HTMLSelectElement;
  for (const [k, label] of KINDS) s.append(h("option", { value: k, text: t(label) }));
  s.value = value;
  return s;
}

export function memorySection(settings: Settings, save: () => void, toggle: Toggle): HTMLElement {
  const prefs = (): AssistantPrefs => ({ ...DEFAULT_ASSISTANT, ...(settings.assistant ?? {}) });
  const set = <K extends keyof AssistantPrefs>(key: K, value: AssistantPrefs[K]) => {
    settings.assistant = { ...prefs(), [key]: value };
    save();
  };
  const message = h("div", {});
  const say = (text: string, kind: "ok" | "err" = "err") => {
    clear(message);
    message.append(h("div", { class: `notice ${kind}`, text }));
  };
  const memList = h("div", { class: "stack" });
  const convList = h("div", { class: "stack" });

  // ── Long-term memory ──
  const addKind = kindSelect("fact");
  const addText = h("input", { type: "text", maxlength: 2000, placeholder: t("e.g. I prefer TypeScript over JavaScript"), style: "flex:1" }) as HTMLInputElement;
  const addBtn = h("button", { class: "primary", text: t("Remember") }) as HTMLButtonElement;
  const add = async () => {
    if (!addText.value.trim()) return;
    try {
      await Bridge.memoryAdd(addKind.value as MemoryKind, addText.value);
      addText.value = "";
      clear(message);
      await drawMemories();
    } catch (e) {
      say(String(e).replace(/^Error:\s*/, ""));
    }
  };
  addBtn.addEventListener("click", () => void add());
  addText.addEventListener("keydown", (e) => {
    if (e.key === "Enter") void add();
  });

  function memoryRow(m: MemoryEntry): HTMLElement {
    const kind = kindSelect(m.kind);
    const text = h("input", { type: "text", value: m.content, maxlength: 2000, style: "flex:1" }) as HTMLInputElement;
    const pin = h("button", { class: m.pinned ? "primary" : "", text: m.pinned ? t("Pinned") : t("Pin"), title: t("Pinned memories always go into the prompt first.") }) as HTMLButtonElement;
    const update = async (pinned: boolean) => {
      try {
        await Bridge.memoryUpdate(m.id, kind.value as MemoryKind, text.value, pinned);
        await drawMemories();
      } catch (e) {
        say(String(e).replace(/^Error:\s*/, ""));
      }
    };
    kind.addEventListener("change", () => void update(m.pinned));
    text.addEventListener("change", () => void update(m.pinned));
    pin.addEventListener("click", () => void update(!m.pinned));
    const del = h("button", { class: "danger", text: t("Delete") }) as HTMLButtonElement;
    confirmClick(del, t("Click again to delete"), async () => {
      await Bridge.memoryDelete(m.id).catch((e) => say(String(e)));
      await drawMemories();
    });
    return h("div", { class: "row memory-row", title: t("Saved {date}", { date: when(m.updatedAt) }) }, kind, text, pin, del);
  }

  async function drawMemories() {
    clear(memList);
    try {
      const all = await Bridge.memoryList();
      if (all.length === 0) memList.append(h("div", { class: "hint", text: t("Nothing remembered yet.") }));
      for (const m of all) memList.append(memoryRow(m));
    } catch (e) {
      memList.append(h("div", { class: "notice err", text: String(e).replace(/^Error:\s*/, "") }));
    }
  }

  // ── Chat history ──
  function conversationRow(c: ConversationInfo): HTMLElement {
    const transcript = h("div", { class: "transcript" });
    const open = h("button", { text: t("Show") }) as HTMLButtonElement;
    open.addEventListener("click", async () => {
      if (transcript.childElementCount) {
        clear(transcript);
        open.textContent = t("Show");
        return;
      }
      open.textContent = t("Hide");
      for (const m of await Bridge.historyMessages(c.id).catch(() => [])) {
        transcript.append(h("div", { class: `line ${m.role}` }, h("b", { text: m.role === "user" ? t("You") : prefs().name }), h("span", { text: m.content })));
      }
    });
    const del = h("button", { class: "danger", text: t("Delete") }) as HTMLButtonElement;
    confirmClick(del, t("Click again to delete"), async () => {
      await Bridge.historyDelete(c.id).catch((e) => say(String(e)));
      await drawConversations();
    });
    return h(
      "div",
      { class: "stack" },
      h(
        "div",
        { class: "pill-row full" },
        h("div", { class: "name" }, h("div", { text: c.title }), h("div", { class: "hint", text: `${when(c.updatedAt)} · ${t("{count} messages", { count: c.messages })}` })),
        open,
        del,
      ),
      transcript,
    );
  }

  async function drawConversations() {
    clear(convList);
    try {
      const all = await Bridge.historyList();
      if (all.length === 0) convList.append(h("div", { class: "hint", text: t("No saved conversations.") }));
      for (const c of all.slice(0, 50)) convList.append(conversationRow(c));
    } catch (e) {
      convList.append(h("div", { class: "notice err", text: String(e).replace(/^Error:\s*/, "") }));
    }
  }

  const retention = h("select", {}) as HTMLSelectElement;
  for (const d of RETENTION) retention.append(h("option", { value: String(d), text: d === 0 ? t("Forever") : t("{days} days", { days: d }) }));
  if (!RETENTION.includes(prefs().historyRetentionDays)) {
    retention.append(h("option", { value: String(prefs().historyRetentionDays), text: t("{days} days", { days: prefs().historyRetentionDays }) }));
  }
  retention.value = String(prefs().historyRetentionDays);
  retention.addEventListener("change", () => set("historyRetentionDays", Number(retention.value)));

  const clearMem = h("button", { class: "danger", text: t("Clear all memories") }) as HTMLButtonElement;
  confirmClick(clearMem, t("Click again: chats are kept"), async () => {
    const n = await Bridge.memoryClear().catch((e) => (say(String(e)), 0));
    say(t("{count} memories deleted. Chat history was not touched.", { count: n }), "ok");
    await drawMemories();
  });
  const clearChats = h("button", { class: "danger", text: t("Clear chat history") }) as HTMLButtonElement;
  confirmClick(clearChats, t("Click again: memories are kept"), async () => {
    const n = await Bridge.historyClear().catch((e) => (say(String(e)), 0));
    say(t("{count} conversations deleted. Memories were not touched.", { count: n }), "ok");
    await drawConversations();
  });
  const exportBtn = h("button", { text: t("Export everything") }) as HTMLButtonElement;
  exportBtn.addEventListener("click", async () => {
    try {
      const path = await Bridge.memoryExport();
      say(t("Saved to {path}", { path }), "ok");
    } catch (e) {
      say(String(e).replace(/^Error:\s*/, ""));
    }
  });

  void drawMemories();
  void drawConversations();
  return h(
    "section",
    { id: "memory" },
    h("h2", {}, h("span", { text: t("Memory") })),
    message,
    h("div", { class: "row" }, h("label", { text: t("Remember things") }), toggle(prefs().memoryEnabled, (on) => set("memoryEnabled", on))),
    h("div", { class: "hint", text: t("What Groovy remembers goes into every chat, with any provider, so he can use it. Things that look like passwords or keys are refused. It is stored only on this computer.") }),
    h("div", { class: "row" }, addKind, addText, addBtn),
    memList,
    h("div", { class: "row" }, clearMem),
    h("div", { class: "subhead", text: t("Chat history") }),
    h("div", { class: "row" }, h("label", { text: t("Save conversations") }), toggle(prefs().historyEnabled, (on) => set("historyEnabled", on))),
    h("div", { class: "row" }, h("label", { text: t("Keep them for") }), retention),
    convList,
    h("div", { class: "row" }, clearChats, h("span", { class: "spacer" }), exportBtn),
  );
}
