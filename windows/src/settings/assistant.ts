// Settings → Assistant: who Groovy is when he answers (assistant.rs turns
// this into the system prompt every provider gets) and where answers come
// from: local only, online only, or hybrid with the local model as fallback.

import { DEFAULT_ASSISTANT, type AssistantPrefs, type Settings } from "../core/state";
import { h } from "../views/dom";
import { N_, t } from "../i18n/i18n";

type Toggle = (on: boolean, onChange: (v: boolean) => void) => HTMLElement;

const PERSONALITIES: [AssistantPrefs["personality"], string][] = [
  ["friendly", N_("Friendly")],
  ["professional", N_("Professional")],
  ["sarcastic", N_("Sarcastic")],
  ["energetic", N_("Energetic")],
  ["quiet", N_("Quiet")],
  ["assistant", N_("Assistant mode")],
];

const LENGTHS: [AssistantPrefs["responseLength"], string][] = [
  ["short", N_("Short")],
  ["balanced", N_("Balanced")],
  ["detailed", N_("Detailed")],
];

const PROACTIVITY: [AssistantPrefs["proactivity"], string][] = [
  ["low", N_("Only what I ask")],
  ["normal", N_("Now and then")],
  ["high", N_("Suggest next steps")],
];

const MODES: [AssistantPrefs["aiMode"], string, string][] = [
  ["hybrid", N_("Hybrid"), N_("The provider picked in the chat; the model on this PC answers when it can't be reached, has no key, or today's limit is reached.")],
  ["local", N_("This PC only"), N_("Nothing you type is ever sent to an online provider. Works with no internet once a model is installed.")],
  ["online", N_("Online only"), N_("Always the provider picked in the chat, never a fallback.")],
];

const LOCALS: [AssistantPrefs["localProvider"], string][] = [
  ["llamacpp", N_("Groovy's engine (Settings → Models)")],
  ["ollama", "Ollama"],
  ["lmstudio", "LM Studio"],
  ["custom", N_("OpenAI-compatible server")],
];

/** Languages Groovy can be asked to always answer in. The model decides how well it does. */
const LANGUAGES = ["", "English", "Spanish", "German", "French", "Punjabi", "Hindi", "Portuguese", "Italian", "Dutch", "Polish", "Turkish", "Arabic", "Chinese", "Japanese", "Korean"];

function select<T extends string>(options: [T, string][], value: T, onChange: (v: T) => void): HTMLSelectElement {
  const el = h("select", {}) as HTMLSelectElement;
  for (const [v, label] of options) el.append(h("option", { value: v, text: t(label) }));
  el.value = value;
  el.addEventListener("change", () => onChange(el.value as T));
  return el;
}

export function prefs(settings: Settings): AssistantPrefs {
  return { ...DEFAULT_ASSISTANT, ...(settings.assistant ?? {}) };
}

export function assistantSection(settings: Settings, save: () => void, toggle: Toggle): HTMLElement {
  const p = prefs(settings);
  settings.assistant = p;
  const set = <K extends keyof AssistantPrefs>(key: K, value: AssistantPrefs[K]) => {
    settings.assistant = { ...settings.assistant, [key]: value };
    save();
  };

  const name = h("input", { type: "text", value: p.name, maxlength: 32, placeholder: "Groovy" }) as HTMLInputElement;
  name.addEventListener("change", () => set("name", name.value.trim() || "Groovy"));

  const humourValue = h("span", { class: "hint", text: String(p.humour) });
  const humour = h("input", { type: "range", min: 0, max: 10, step: 1, value: p.humour }) as HTMLInputElement;
  humour.addEventListener("input", () => (humourValue.textContent = humour.value));
  humour.addEventListener("change", () => set("humour", Number(humour.value)));

  const language = h("select", {}) as HTMLSelectElement;
  for (const l of LANGUAGES) language.append(h("option", { value: l, text: l || t("The language I write in") }));
  if (!LANGUAGES.includes(p.language)) language.append(h("option", { value: p.language, text: p.language }));
  language.value = p.language;
  language.addEventListener("change", () => set("language", language.value));

  const custom = h("textarea", {
    class: "textarea",
    rows: 3,
    maxlength: 1500,
    placeholder: t("Anything else Groovy should always keep in mind, e.g. “I'm learning Rust, explain simply.”"),
  }) as HTMLTextAreaElement;
  custom.value = p.customInstructions;
  custom.addEventListener("change", () => set("customInstructions", custom.value.trim()));

  const modeHint = h("div", { class: "hint" });
  const showMode = (m: AssistantPrefs["aiMode"]) => (modeHint.textContent = t(MODES.find((x) => x[0] === m)![2]));
  showMode(p.aiMode);
  const mode = select(MODES.map(([v, l]) => [v, l] as [AssistantPrefs["aiMode"], string]), p.aiMode, (v) => {
    showMode(v);
    set("aiMode", v);
  });

  const limit = h("input", { type: "number", min: 0, max: 10000, step: 1, value: p.dailyOnlineLimit, style: "width:90px" }) as HTMLInputElement;
  limit.addEventListener("change", () => set("dailyOnlineLimit", Math.max(0, Math.floor(Number(limit.value) || 0))));

  return h(
    "section",
    { id: "assistant" },
    h("h2", {}, h("span", { text: t("Assistant") })),
    h("div", { class: "row" }, h("label", { text: t("Name") }), name),
    h("div", { class: "row" }, h("label", { text: t("Personality") }), select(PERSONALITIES, p.personality, (v) => set("personality", v))),
    h("div", { class: "row" }, h("label", { text: t("Humour") }), humour, humourValue),
    h("div", { class: "row" }, h("label", { text: t("Answer length") }), select(LENGTHS, p.responseLength, (v) => set("responseLength", v))),
    h("div", { class: "row" }, h("label", { text: t("Proactivity") }), select(PROACTIVITY, p.proactivity, (v) => set("proactivity", v))),
    h("div", { class: "row" }, h("label", { text: t("Answer in") }), language),
    h("div", { class: "row column" }, h("label", { text: t("Your instructions") }), custom),
    h("div", { class: "hint", text: t("These change what the assistant actually says, with every provider.") }),
    h("div", { class: "subhead", text: t("Where answers come from") }),
    h("div", { class: "row" }, h("label", { text: t("AI mode") }), mode),
    modeHint,
    h("div", { class: "row" }, h("label", { text: t("Local model") }), select(LOCALS, p.localProvider, (v) => set("localProvider", v))),
    h("div", { class: "row" }, h("label", { text: t("Online messages per day") }), limit, h("span", { class: "hint", text: t("0 = no limit") })),
    h(
      "div",
      { class: "row" },
      h("label", { text: t("Remember things") }),
      toggle(p.memoryEnabled, (on) => set("memoryEnabled", on)),
      h("span", { class: "hint", text: t("Say “remember that…” in the chat. Managed in Memory below.") }),
    ),
  );
}
