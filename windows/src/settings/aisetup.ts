// "Groovy's AI": the first question, before anything loads a model. Cloud and
// subscription first, local last (FIXES_ROUND1 item 2: on an 8 GB PC the local
// model took 2 GB and the PC to 91 %). Shown at the top of the Assistant page,
// opened at the first start, and again from General → Run setup again.

import { Bridge } from "../core/bridge";
import { providerDef } from "../core/providers";
import type { AssistantPrefs, Settings } from "../core/state";
import type { ProviderId } from "../core/providers";
import { h, clear } from "../views/dom";
import { showError } from "../views/toast";
import { t } from "../i18n/i18n";

const GB = 1024 ** 3;

type CloudId = "google" | "anthropic" | "openai" | "openrouter";

const CLOUDS: [CloudId, string, string][] = [
  ["google", "Gemini", "aistudio.google.com/apikey"],
  ["anthropic", "Claude", "console.anthropic.com"],
  ["openai", "OpenAI", "platform.openai.com/api-keys"],
  ["openrouter", "OpenRouter", "openrouter.ai/keys"],
];

/** What the local option says on a PC with `total` bytes of memory. */
export function localAdvice(total: number): { recommended: boolean; warning: string | null } {
  if (total >= 16 * GB) return { recommended: true, warning: null };
  if (total <= 9 * GB) return { recommended: false, warning: "slow" };
  return { recommended: false, warning: "tight" };
}

/** The settings a choice leads to. Pure, tested. */
export function applyChoice(
  a: AssistantPrefs,
  choice: { kind: "cloud"; provider: CloudId } | { kind: "subscription"; tool: "claude-code" | "codex" } | { kind: "local" },
): { assistant: AssistantPrefs; chatProvider: ProviderId | null } {
  if (choice.kind === "cloud") {
    return { assistant: { ...a, aiSetup: "cloud", brain: "auto", aiMode: a.aiMode === "local" ? "hybrid" : a.aiMode }, chatProvider: choice.provider };
  }
  if (choice.kind === "subscription") {
    return { assistant: { ...a, aiSetup: "subscription", brain: choice.tool, aiMode: a.aiMode === "local" ? "hybrid" : a.aiMode }, chatProvider: null };
  }
  return { assistant: { ...a, aiSetup: "local", brain: "auto" }, chatProvider: "llamacpp" };
}

export function aiSetupSection(settings: Settings, save: () => void, prefs: () => AssistantPrefs): HTMLElement {
  const section = h("section", { id: "ai-setup" });
  const draw = async () => {
    clear(section);
    const p = prefs();
    const memory = (await Bridge.modelsStatus())?.memory.total ?? 0;
    const clis = (await Bridge.aiClis()) ?? { claudeCode: false, codex: false };
    const choose = (c: Parameters<typeof applyChoice>[1]) => {
      const next = applyChoice(p, c);
      settings.assistant = next.assistant;
      if (next.chatProvider) settings.chatProvider = next.chatProvider;
      save();
      void draw();
    };
    const current = p.aiSetup === "cloud" ? t("A cloud AI with your key")
      : p.aiSetup === "subscription" ? t("Your subscription ({tool})", { tool: p.brain === "codex" ? "Codex" : "Claude Code" })
      : p.aiSetup === "local" ? t("The model on this PC")
      : null;
    section.append(
      h("h2", {}, h("span", { text: t("Groovy's AI") })),
      current
        ? h("div", { class: "notice ok", text: t("In use: {what}.", { what: current }) })
        : h("div", { class: "notice warn", text: t("Choose the AI Groovy uses for the chat, « OK Groovy », dictation and notes. Nothing loads until you choose.") }),
    );

    // 1. Gemini: free.
    const gemini = h("input", { type: "password", placeholder: "AIza…", autocomplete: "off", style: "flex:1" }) as HTMLInputElement;
    const geminiSave = h("button", { class: "primary", text: t("Use Gemini") }) as HTMLButtonElement;
    geminiSave.addEventListener("click", () => {
      const key = gemini.value.trim();
      if (!key) {
        showError(t("Paste your Gemini key first."));
        return;
      }
      void Bridge.secretSet(providerDef("google").key!, key).then(() => choose({ kind: "cloud", provider: "google" }), showError);
    });
    section.append(
      h("div", { class: "subhead", text: t("Gemini — free (recommended)") }),
      h("div", { class: "hint", text: t("Get a free key in a minute at aistudio.google.com/apikey (a Google account is enough), then paste it here.") }),
      h("div", { class: "row" }, gemini, geminiSave, h("button", { text: t("Get a key"), onclick: () => void Bridge.openUrl("https://aistudio.google.com/apikey") })),
    );

    // 2. Another key.
    const which = h("select", {}) as HTMLSelectElement;
    for (const [id, name] of CLOUDS.slice(1)) which.append(h("option", { value: id, text: name }));
    const other = h("input", { type: "password", placeholder: t("API key"), autocomplete: "off", style: "flex:1" }) as HTMLInputElement;
    const otherSave = h("button", { text: t("Use this key") }) as HTMLButtonElement;
    otherSave.addEventListener("click", () => {
      const key = other.value.trim();
      const id = which.value as CloudId;
      if (!key) {
        showError(t("Paste the key first."));
        return;
      }
      void Bridge.secretSet(providerDef(id).key!, key).then(() => choose({ kind: "cloud", provider: id }), showError);
    });
    section.append(
      h("div", { class: "subhead", text: t("Claude, OpenAI or OpenRouter key") }),
      h("div", { class: "row" }, which, other, otherSave),
      h("div", { class: "hint", text: t("Keys are kept in the Windows credential store, never in a file.") }),
    );

    // 3. A subscription through its CLI.
    const sub = h("div", { class: "row" });
    for (const [tool, name, found] of [["claude-code", "Claude Code", clis.claudeCode], ["codex", "Codex", clis.codex]] as const) {
      const b = h("button", { text: found ? t("Use {name}", { name }) : t("{name} not installed", { name }) }) as HTMLButtonElement;
      b.addEventListener("click", () => {
        if (!found) {
          showError(t("Install {name} and sign in first, then come back here.", { name }));
          return;
        }
        choose({ kind: "subscription", tool });
      });
      sub.append(b);
    }
    section.append(
      h("div", { class: "subhead", text: t("Claude Code or Codex subscription") }),
      sub,
      h("div", { class: "hint", text: t("Uses the command-line tool you already signed in to, for « OK Groovy », dictation and notes (answers without tools). The chat still needs a key.") }),
    );

    // 4. Local, last.
    // Memory unknown (no answer): no warning rather than a wrong one.
    const advice = memory > 0 ? localAdvice(memory) : { recommended: false, warning: null };
    const local = h("button", { text: t("Use local AI") }) as HTMLButtonElement;
    local.addEventListener("click", () => choose({ kind: "local" }));
    section.append(
      h("div", { class: "subhead", text: t("Local (needs a strong PC)") }),
      h("div", {
        class: advice.warning === "slow" ? "notice err" : advice.warning ? "notice warn" : "hint",
        text: advice.warning === "slow"
          ? t("This PC has {total}: a local model takes about 2 GB and will slow everything down. A cloud AI is better here.", { total: `${(memory / GB).toFixed(0)} GB` })
          : advice.warning
            ? t("This PC has {total}: small local models work, but slowly. 16 GB or more is recommended.", { total: `${(memory / GB).toFixed(0)} GB` })
            : t("Runs on this computer, offline and private. Download a model in Groovy → Models afterwards."),
      }),
      h("div", { class: "row" }, local),
    );
  };
  void draw();
  return section;
}
