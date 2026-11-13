// Brains → On this PC (round 3D item 5): Ollama with a Qwen model sized for
// this PC, set up in one click. Groovy only calls Ollama's HTTP API; Ollama
// loads the model and lets it go after five quiet minutes. On this PC the
// order is Ollama, then LM Studio, then Groovy's own model.

import { Bridge, onEvent, type OllamaStatus, type OllamaStep } from "../core/bridge";
import type { Settings } from "../core/state";
import { clear, h } from "../views/dom";
import { N_, t } from "../i18n/i18n";
import { btn, setting, statusPill } from "../hub/ui";

const rows = (...r: (HTMLElement | null)[]) => h("div", { class: "hub-t-rows" }, ...r);
import { showError } from "../views/toast";

export const OLLAMA_STRINGS = {
  title: N_("Ollama + Qwen"),
  hint: N_("The smoothest way to run AI on this PC. Groovy asks Ollama for answers and never loads the model itself."),
  looking: N_("Looking for Ollama…"),
  missing: N_("Ollama isn't on this PC yet."),
  get: N_("Get Ollama"),
  again: N_("Check again"),
  stopped: N_("Ollama is installed but not running. Use it and Groovy starts it."),
  running: N_("Ollama is running."),
  model: N_("Model"),
  modelDetail: N_("Every model Ollama has. Qwen is picked for this PC."),
  download: N_("{model} (downloads, about {size})"),
  use: N_("Use Ollama + Qwen"),
  inUse: N_("In use: {model}"),
  starting: N_("Starting Ollama…"),
  pulling: N_("Downloading {model}…"),
  ready: N_("Ready: {model} answers on this PC."),
  order: N_("On this PC: Ollama first, then LM Studio, then Groovy's own model."),
};

/** About how much each recommended Qwen downloads. */
const SIZES: Record<string, string> = { "qwen2.5:1.5b": "1 GB", "qwen2.5:3b": "2 GB", "qwen2.5:7b": "4.7 GB" };

export function ollamaSection(settings: Settings, save: () => Promise<void> | void): HTMLElement {
  const body = h("div", {});
  const section = h("section", {}, h("h2", {}, h("span", { text: t(OLLAMA_STRINGS.title) })), h("div", { class: "hint", text: t(OLLAMA_STRINGS.hint) }), body);
  let busy = false;

  const draw = (s: OllamaStatus | null, note?: HTMLElement) => {
    clear(body);
    if (!s) {
      body.append(rows(setting(t(OLLAMA_STRINGS.looking), null, null)));
      return;
    }
    const again = btn(t(OLLAMA_STRINGS.again), () => void refresh(), "subtle");
    if (!s.installed) {
      const get = btn(t(OLLAMA_STRINGS.get), () => void Bridge.openUrl(s.downloadPage), "primary");
      body.append(rows(setting(t(OLLAMA_STRINGS.missing), t(OLLAMA_STRINGS.order), h("div", { class: "hub-t-actions" }, get, again))));
      return;
    }
    // Every installed model, and the recommended Qwen when it isn't there yet.
    const choices = [...s.models];
    if (!choices.includes(s.recommended)) choices.unshift(s.recommended);
    const current = settings.chatModels.ollama || (s.models.find((m) => m === s.recommended) ?? s.models.find((m) => m.includes("qwen")) ?? s.recommended);
    const pick = h("select", {}) as HTMLSelectElement;
    for (const m of choices) {
      const label = s.models.includes(m) ? m : t(OLLAMA_STRINGS.download, { model: m, size: SIZES[m] ?? "?" });
      pick.append(h("option", { value: m, text: label }));
    }
    pick.value = choices.includes(current) ? current : choices[0];
    const inUse = settings.chatProvider === "ollama" && !!settings.chatModels.ollama;
    const state = s.running ? statusPill("connected", t(OLLAMA_STRINGS.running)) : statusPill("unset", t(OLLAMA_STRINGS.stopped));
    const use = btn(inUse && pick.value === settings.chatModels.ollama ? t(OLLAMA_STRINGS.inUse, { model: settings.chatModels.ollama }) : t(OLLAMA_STRINGS.use), () => void setUp(pick.value), "primary");
    use.disabled = busy || (inUse && pick.value === settings.chatModels.ollama);
    pick.onchange = () => draw(s);
    body.append(rows(
      setting("Ollama", t(OLLAMA_STRINGS.order), state),
      setting(t(OLLAMA_STRINGS.model), t(OLLAMA_STRINGS.modelDetail), pick),
      setting("", null, h("div", { class: "hub-t-actions" }, use, again)),
      note ?? null,
    ));
  };

  /** Ollama's state, or "not on this PC" when Groovy couldn't ask. */
  const status = async (): Promise<OllamaStatus> =>
    (await Bridge.ollamaStatus()) ?? { running: false, installed: false, models: [], canInstall: false, recommended: "qwen2.5:3b", downloadPage: "https://ollama.com/download/windows" };

  const refresh = async () => {
    draw(null);
    draw(await status());
  };

  const setUp = async (model: string) => {
    if (busy) return;
    busy = true;
    const progress = h("div", {});
    const show = (pill: HTMLElement) => {
      clear(progress);
      progress.append(setting("", null, pill));
    };
    show(statusPill("downloading", t(OLLAMA_STRINGS.starting)));
    body.append(progress);
    const stop = await onEvent<OllamaStep>("ollama-setup", (step) => {
      if (step.stage === "model") {
        show(statusPill("downloading", t(OLLAMA_STRINGS.pulling, { model }), step.total > 0 ? step.done / step.total : undefined));
      } else if (step.stage === "start") {
        show(statusPill("downloading", t(OLLAMA_STRINGS.starting)));
      }
    });
    try {
      const used = await Bridge.ollamaSetup(model);
      // The backend saved it; the page's copy follows.
      settings.chatProvider = "ollama";
      settings.chatModels = { ...settings.chatModels, ollama: used };
      settings.assistant = { ...settings.assistant, localProvider: "auto" };
      await save();
      busy = false;
      draw(await status(), setting("", null, statusPill("connected", t(OLLAMA_STRINGS.ready, { model: used }))));
    } catch (e) {
      busy = false;
      showError(e);
      draw(await status(), setting("", null, statusPill("error", String(e))));
    } finally {
      stop?.();
    }
  };

  void refresh();
  return section;
}
