// What the island says at each stage of a voice call (FIXES_ROUND2 item 4):
// Listening → Heard "…" → Thinking (who answers) → Speaking, and when a stage
// fails, why and the one button that fixes it.

import { providerDef } from "../core/providers";
import type { Settings } from "../core/state";
import type { CallState } from "./call-state";
import { N_, t } from "../i18n/i18n";

/** The fix for a failed stage: the speech model, the AI, or a voice to speak with. */
export type CallFix = "speech" | "ai" | "voice";

export const FIX_LABELS: Record<CallFix, string> = {
  speech: N_("Download speech model"),
  ai: N_("Choose an AI"),
  voice: N_("Pick a voice"),
};

/** Where each fix is done: a Control Centre page. */
export const FIX_PAGES: Record<CallFix, string> = {
  speech: "speechsetup",
  ai: "assistant",
  voice: "calls",
};

/** Who answers the call, as a person reads it: "Gemini", "Local Gemma 3 1B it". */
export function brainName(settings: Pick<Settings, "chatProvider" | "chatModels" | "assistant">): string {
  const id = settings.chatProvider;
  if (id === "llamacpp") {
    const file = settings.assistant?.localModel ?? "";
    const pretty = file.replace(/\.gguf$/i, "").replace(/[-_](Q\d[\w]*|q\d[\w]*)$/, "").replace(/[-_]+/g, " ").trim();
    return pretty ? t("Local {model}", { model: pretty }) : t("the local model");
  }
  // Google's chat models are Gemini: the name people know.
  if (id === "google") return "Gemini";
  return t(providerDef(id).name);
}

/** The call bar's text for `state`. */
export function stageText(state: CallState, heard: string | null, brain: string, reason: string | null): string {
  switch (state) {
    case "listening": return t("Listening…");
    case "processing": {
      const thinking = t("Thinking ({brain})…", { brain });
      return heard ? `${t("Heard “{text}”", { text: clip(heard, 40) })} · ${thinking}` : thinking;
    }
    case "speaking": return t("Speaking…");
    case "muted": return t("Muted");
    case "error": return reason ?? t("Something went wrong");
    default: return "";
  }
}

function clip(text: string, max: number): string {
  return text.length > max ? `${text.slice(0, max - 1)}…` : text;
}
