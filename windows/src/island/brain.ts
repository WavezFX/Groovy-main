// The offline brain's one-click setup, shown on the island's note card: offers
// to install Ollama and a model, shows the progress, then asks the question
// that was waiting.

import { Bridge, onEvent, type OllamaStep } from "../core/bridge";
import { Sound } from "../core/sound";
import { State } from "../core/state";
import { N_, t } from "../i18n/i18n";

export const BRAIN_STRINGS = {
  offer: N_("Groovy's offline brain isn't set up yet."),
  offerDetail: N_("One click installs Ollama and a small AI model (about 2 GB). After that, chat and voice calls work without the internet or API keys."),
  setUp: N_("Set it up"),
  notNow: N_("Not now"),
  install: N_("Downloading Ollama… {0}%"),
  start: N_("Starting Ollama…"),
  model: N_("Downloading the brain… {0}%"),
  modelWait: N_("Downloading the brain…"),
  done: N_("Ready! Asking your question now."),
};

/** Is Ollama there with a model? Asked before a chat goes to it. */
export async function brainReady(): Promise<boolean> {
  const s = await Bridge.ollamaStatus();
  return !!s && s.running && s.models.length > 0;
}

let progressHooked = false;

function hookProgress() {
  if (progressHooked) return;
  progressHooked = true;
  void onEvent<OllamaStep>("ollama-setup", (step) => {
    if (State.brainSetup !== "working") return;
    const pct = step.total > 0 ? Math.floor((step.done / step.total) * 100) : -1;
    State.noteMessage =
      step.stage === "install"
        ? t(BRAIN_STRINGS.install, { 0: Math.max(0, pct) })
        : step.stage === "start"
          ? t(BRAIN_STRINGS.start)
          : step.stage === "model"
            ? pct >= 0 ? t(BRAIN_STRINGS.model, { 0: pct }) : t(BRAIN_STRINGS.modelWait)
            : t(BRAIN_STRINGS.done);
    State.notify();
  });
}

/** Shows the offer on the note card; `then` asks the waiting question once ready. */
export function offerBrain(then: () => void) {
  hookProgress();
  State.brainSetup = "offer";
  State.brainThen = then;
  State.noteMessage = t(BRAIN_STRINGS.offer);
  State.view = "note";
  State.notify();
}

export async function setUpBrain() {
  State.brainSetup = "working";
  State.noteMessage = t(BRAIN_STRINGS.start);
  State.notify();
  try {
    await Bridge.ollamaSetup();
    State.noteMessage = t(BRAIN_STRINGS.done);
    Sound.play("finish");
    State.brainSetup = null;
    const then = State.brainThen;
    State.brainThen = null;
    State.view = "prompt";
    State.notify();
    then?.();
  } catch (e) {
    State.brainSetup = "offer";
    State.noteMessage = String(e).replace(/^Error:\s*/, "");
    Sound.play("error");
    State.notify();
  }
}

export function dismissBrain() {
  State.brainSetup = null;
  State.brainThen = null;
  State.view = "prompt";
  State.notify();
}
