// Brains, as approved (docs/design/settings-v2/Brains, round 3D item 3): on
// the left the tab's brains, each a row with its status and one action, and
// the router plugin's place; on the right the Auto order, "Switch when a limit
// is hit" and "Model per job". The detailed sections (models, servers, the
// assistant's personality) follow below the two columns.

import { Bridge } from "../core/bridge";
import type { Settings } from "../core/state";
import { N_, t } from "../i18n/i18n";
import { h, clear } from "../views/dom";
import { btn, secretField, statusPill, type Status } from "../hub/ui";

/** A brain the Auto order lists, by the id assistant.rs uses. */
interface Brain {
  id: string;
  name: string;
  kind: string;
}

export const BRAIN_STRINGS = {
  autoOrder: N_("Auto order"),
  cloud: N_("cloud"),
  free: N_("free"),
  subscription: N_("subscription"),
  thisPc: N_("this PC"),
  moveUp: N_("Move up"),
  switchName: N_("Switch when a limit is hit"),
  switchDetail: N_("Moves to the next brain and tells you"),
  perJob: N_("Model per job"),
  perJobDetail: N_("Chat, writing, commands"),
  edit: N_("Edit"),
  done: N_("Done"),
  router: N_("Router plugin"),
  routerDetail: N_("Let a plugin pick the model for each job. Groovy's switching stays as the backup."),
  addPlugin: N_("Add plugin"),
  routerLater: N_("The router plugin arrives with a later update."),
  manage: N_("Manage"),
  addKey: N_("Add key"),
  setUp: N_("Set up"),
  claudeDetail: N_("Your API key · the chat and quick jobs"),
  geminiDetail: N_("Free tier · resets daily"),
  openaiDetail: N_("Your API key"),
  openrouterDetail: N_("Many models with one key"),
  notSetUp: N_("Not set up"),
  claudeSub: N_("Claude subscription"),
  claudeSubDetail: N_("Used through Claude Code"),
  chatgptSub: N_("ChatGPT subscription"),
  chatgptSubDetail: N_("Used through Codex"),
  notInstalled: N_("Not installed"),
  ollamaDetail: N_("Found on this PC · unloads after 5 min idle"),
  ollamaMissing: N_("Not on this PC yet"),
  lmstudioDetail: N_("Runs its own server on this PC"),
  builtIn: N_("Built-in model"),
  builtInDetail: N_("Uses extra memory: over the 400 MB limit"),
  on: N_("On"),
  off: N_("Off"),
  ready: N_("Ready"),
  notFound: N_("Not found"),
  chatJob: N_("Chat"),
  writingJob: N_("Writing"),
  writingJobDetail: N_("Dictation clean-up, notes and call summaries"),
  commandsJob: N_("Commands"),
  commandsJobDetail: N_("« OK Groovy »"),
  auto: N_("Auto order"),
};

const CLOUDS: Brain[] = [
  { id: "anthropic", name: "Claude", kind: BRAIN_STRINGS.cloud },
  { id: "google", name: "Gemini", kind: BRAIN_STRINGS.free },
  { id: "openai", name: "OpenAI", kind: BRAIN_STRINGS.cloud },
  { id: "openrouter", name: "OpenRouter", kind: BRAIN_STRINGS.cloud },
  { id: "claude-code", name: "Claude Code", kind: BRAIN_STRINGS.subscription },
  { id: "codex", name: "Codex", kind: BRAIN_STRINGS.subscription },
];

const KEY_OF: Record<string, string> = {
  anthropic: "anthropic-api-key",
  google: "google-api-key",
  openai: "openai-api-key",
  openrouter: "openrouter-api-key",
};

/** The Auto order: the user's, then any brain not in it; this PC always last. */
export function autoOrder(saved: string[]): string[] {
  const ids = CLOUDS.map((b) => b.id);
  const known = saved.filter((id) => ids.includes(id));
  return [...known, ...ids.filter((id) => !known.includes(id))];
}

/** `id` one place earlier. */
export function moveUp(order: string[], id: string): string[] {
  const i = order.indexOf(id);
  if (i <= 0) return order;
  const o = order.slice();
  [o[i - 1], o[i]] = [o[i], o[i - 1]];
  return o;
}

function mono(letter: string, tint: string): HTMLElement {
  return h("div", { class: "hub-b-mono", style: `background:${tint}`, text: letter });
}

/** One brain: its letter, name and line, status and action; the action can open a panel under it. */
function brainRow(letter: string, tint: string, name: string, detail: string, status: Status, statusText: string, action: HTMLElement | null, panel?: HTMLElement): HTMLElement {
  const row = h("div", { class: "hub-b-row" },
    mono(letter, tint),
    h("div", { class: "hub-b-text" }, h("span", { class: "hub-b-name", text: name }), h("span", { class: "hub-b-detail", text: detail })),
    statusPill(status, statusText),
    action,
  );
  return panel ? h("div", { class: "hub-b-item" }, row, panel) : row;
}

/** `changed` runs after a key is stored (the island's tiles and the chat follow it). */
export async function brainsPage(settings: Settings, save: () => Promise<void> | void, tab: string, keys: Record<string, boolean>, changed: () => void): Promise<HTMLElement> {
  const a = () => settings.assistant;
  const setA = (change: Partial<Settings["assistant"]>) => {
    settings.assistant = { ...settings.assistant, ...change };
    void save();
  };

  // ── Left: the tab's brains ──
  const list = h("div", { class: "hub-card hub-b-list" });
  const keyRow = (id: string, letter: string, tint: string, name: string, detail: string) => {
    const has = !!keys[KEY_OF[id]];
    const panel = h("div", { class: "hub-b-panel", hidden: true });
    const field = secretField({
      label: name,
      present: has,
      placeholder: t("API key"),
      save: async (value) => {
        await Bridge.secretSet(KEY_OF[id], value);
        keys[KEY_OF[id]] = true;
        changed();
        void redraw();
      },
    });
    panel.append(field);
    const action = btn(has ? t(BRAIN_STRINGS.manage) : t(BRAIN_STRINGS.addKey), () => (panel.hidden = !panel.hidden));
    return brainRow(letter, tint, name, has ? detail : t(BRAIN_STRINGS.notSetUp), has ? "connected" : "unset", has ? t("Connected") : t(BRAIN_STRINGS.notSetUp), action, panel);
  };

  async function redraw() {
    clear(list);
    if (tab === "subscriptions") {
      const clis = (await Bridge.aiClis()) ?? { claudeCode: false, codex: false };
      const more = () => document.querySelector(".hub-b-more")?.scrollIntoView({ behavior: "smooth" });
      list.append(
        brainRow("C", "#E8A27C", t(BRAIN_STRINGS.claudeSub), clis.claudeCode ? t(BRAIN_STRINGS.claudeSubDetail) : t(BRAIN_STRINGS.notInstalled), clis.claudeCode ? "connected" : "unset", clis.claudeCode ? t("Connected") : t(BRAIN_STRINGS.notSetUp), btn(clis.claudeCode ? t(BRAIN_STRINGS.manage) : t(BRAIN_STRINGS.setUp), more)),
        brainRow("X", "#C9D1CD", t(BRAIN_STRINGS.chatgptSub), clis.codex ? t(BRAIN_STRINGS.chatgptSubDetail) : t(BRAIN_STRINGS.notInstalled), clis.codex ? "connected" : "unset", clis.codex ? t("Connected") : t(BRAIN_STRINGS.notSetUp), btn(clis.codex ? t(BRAIN_STRINGS.manage) : t(BRAIN_STRINGS.setUp), more)),
      );
    } else if (tab === "local") {
      const ollama = await Bridge.ollamaStatus();
      const more = () => document.querySelector(".hub-b-more")?.scrollIntoView({ behavior: "smooth" });
      const model = settings.chatModels.ollama || ollama?.models.find((m) => m.includes("qwen")) || ollama?.models[0] || "";
      const builtIn = a().aiSetup === "local" && a().localProvider === "llamacpp";
      list.append(
        brainRow("Q", "#7FD4FF", model ? `Ollama · ${model}` : "Ollama", ollama?.running ? t(BRAIN_STRINGS.ollamaDetail) : t(BRAIN_STRINGS.ollamaMissing), ollama?.running ? "connected" : "unset", ollama?.running ? t(BRAIN_STRINGS.ready) : t(BRAIN_STRINGS.notFound), btn(t("Models"), more)),
        brainRow("L", "#C9D1CD", "LM Studio", settings.lmstudioUrl ? t(BRAIN_STRINGS.lmstudioDetail) : t(BRAIN_STRINGS.notInstalled), settings.lmstudioUrl ? "connected" : "unset", settings.lmstudioUrl ? t("Connected") : t(BRAIN_STRINGS.notFound), btn(t(BRAIN_STRINGS.setUp), more)),
        brainRow("B", "#F5B84B", t(BRAIN_STRINGS.builtIn), t(BRAIN_STRINGS.builtInDetail), "warn", builtIn ? t(BRAIN_STRINGS.on) : t(BRAIN_STRINGS.off), btn(t(BRAIN_STRINGS.setUp), more)),
      );
    } else {
      list.append(
        keyRow("anthropic", "C", "#E8A27C", "Claude", t(BRAIN_STRINGS.claudeDetail)),
        keyRow("google", "G", "#8AB4F8", "Gemini", t(BRAIN_STRINGS.geminiDetail)),
        keyRow("openai", "O", "#C9D1CD", "OpenAI", t(BRAIN_STRINGS.openaiDetail)),
        keyRow("openrouter", "R", "#C7A7FF", "OpenRouter", t(BRAIN_STRINGS.openrouterDetail)),
      );
    }
  }
  await redraw();

  const addPlugin = btn(t(BRAIN_STRINGS.addPlugin), () => {});
  addPlugin.disabled = true;
  addPlugin.title = t(BRAIN_STRINGS.routerLater);
  const router = h("div", { class: "hub-card hub-b-router" },
    h("div", { class: "hub-b-text" }, h("span", { class: "hub-b-name", text: t(BRAIN_STRINGS.router) }), h("span", { class: "hub-b-detail", text: t(BRAIN_STRINGS.routerDetail) })),
    addPlugin,
  );
  const left = h("div", { class: "hub-b-col" }, h("div", { class: "hub-tab-slot" }), list, router);

  // ── Right: the Auto order, switching, and who does each job ──
  const orderCard = h("div", { class: "hub-card hub-b-order" });
  // A subscription is in the order once its command-line tool is on this PC.
  const clis = (await Bridge.aiClis()) ?? { claudeCode: false, codex: false };
  const shown = (id: string) => (id === "claude-code" ? clis.claudeCode : id === "codex" ? clis.codex : true);
  const drawOrder = () => {
    clear(orderCard);
    const order = autoOrder(a().brainOrder ?? []);
    const entries = [...order.filter(shown).map((id) => CLOUDS.find((b) => b.id === id)!), { id: "local", name: t("On this PC"), kind: BRAIN_STRINGS.thisPc }];
    entries.forEach((b, i) => {
      const up = h("button", { class: "hub-icon", type: "button", "aria-label": t(BRAIN_STRINGS.moveUp), title: t(BRAIN_STRINGS.moveUp) });
      up.innerHTML = '<svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path d="M6 14l6-6 6 6" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>';
      const fixed = b.id === "local" || i === 0;
      (up as HTMLButtonElement).disabled = fixed;
      up.addEventListener("click", () => {
        // One place up among those shown: past the hidden ones in between.
        const visible = order.filter(shown);
        const before = visible[visible.indexOf(b.id) - 1];
        let next = order;
        while (before && next.indexOf(b.id) > next.indexOf(before)) next = moveUp(next, b.id);
        setA({ brainOrder: next });
        drawOrder();
      });
      orderCard.append(h("div", { class: "hub-b-step" },
        h("span", { class: "hub-b-n", text: String(i + 1) }),
        h("span", { class: "hub-b-name", text: b.name }),
        h("span", { class: "hub-b-kind", text: t(b.kind) }),
        up,
      ));
    });
  };
  drawOrder();

  const sw = h("button", { class: a().switchOnLimit !== false ? "switch on" : "switch", type: "button", role: "switch", "aria-label": t(BRAIN_STRINGS.switchName), "aria-checked": String(a().switchOnLimit !== false) });
  sw.addEventListener("click", () => {
    const on = !sw.classList.contains("on");
    sw.classList.toggle("on", on);
    sw.setAttribute("aria-checked", String(on));
    setA({ switchOnLimit: on });
  });

  const jobs = h("div", { class: "hub-b-jobs", hidden: true });
  const brainChoices = (): [string, string][] => [["auto", t(BRAIN_STRINGS.auto)], ...CLOUDS.map((b): [string, string] => [b.id, b.name]), ["local", t("On this PC")]];
  const jobRow = (job: string, name: string, detail: string) => {
    const pick = h("select", { "aria-label": name }) as HTMLSelectElement;
    for (const [id, label] of brainChoices()) pick.append(h("option", { value: id, text: label }));
    pick.value = a().jobBrains?.[job] ?? "auto";
    pick.addEventListener("change", () => setA({ jobBrains: { ...(a().jobBrains ?? {}), [job]: pick.value } }));
    return h("div", { class: "hub-b-row" }, h("div", { class: "hub-b-text" }, h("span", { class: "hub-b-name", text: name }), h("span", { class: "hub-b-detail", text: detail })), pick);
  };
  // The chat's brain is the chat's own picker (settings.chatProvider).
  const chatPick = h("select", { "aria-label": t(BRAIN_STRINGS.chatJob) }) as HTMLSelectElement;
  for (const [id, label] of [["anthropic", "Claude"], ["google", "Gemini"], ["openai", "OpenAI"], ["openrouter", "OpenRouter"], ["llamacpp", t("On this PC")]] as [string, string][]) chatPick.append(h("option", { value: id, text: label }));
  chatPick.value = ["anthropic", "google", "openai", "openrouter"].includes(settings.chatProvider) ? settings.chatProvider : "llamacpp";
  chatPick.addEventListener("change", () => {
    settings.chatProvider = chatPick.value as Settings["chatProvider"];
    void save();
  });
  jobs.append(
    h("div", { class: "hub-b-row" }, h("div", { class: "hub-b-text" }, h("span", { class: "hub-b-name", text: t(BRAIN_STRINGS.chatJob) })), chatPick),
    jobRow("writing", t(BRAIN_STRINGS.writingJob), t(BRAIN_STRINGS.writingJobDetail)),
    jobRow("commands", t(BRAIN_STRINGS.commandsJob), t(BRAIN_STRINGS.commandsJobDetail)),
  );
  const edit = btn(t(BRAIN_STRINGS.edit), () => {
    jobs.hidden = !jobs.hidden;
    edit.textContent = jobs.hidden ? t(BRAIN_STRINGS.edit) : t(BRAIN_STRINGS.done);
  });

  const switches = h("div", { class: "hub-card hub-b-list" },
    h("div", { class: "hub-b-row" }, h("div", { class: "hub-b-text" }, h("span", { class: "hub-b-name", text: t(BRAIN_STRINGS.switchName) }), h("span", { class: "hub-b-detail", text: t(BRAIN_STRINGS.switchDetail) })), sw),
    h("div", { class: "hub-b-row" }, h("div", { class: "hub-b-text" }, h("span", { class: "hub-b-name", text: t(BRAIN_STRINGS.perJob) }), h("span", { class: "hub-b-detail", text: t(BRAIN_STRINGS.perJobDetail) })), edit),
    jobs,
  );
  const right = h("aside", { class: "hub-b-col", "aria-label": t(BRAIN_STRINGS.autoOrder) },
    h("span", { class: "hub-label hub-b-aside-label", text: t(BRAIN_STRINGS.autoOrder) }),
    orderCard,
    switches,
  );
  return h("div", { class: "hub-b-grid" }, left, right);
}
