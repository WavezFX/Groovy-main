// "Test my voice setup" (FIXES_ROUND2 item 4): each stage of a call checked in
// turn — the microphone's level, a 3-second recording turned into text, "say
// hello" sent to the brain the calls use, the answer spoken — with ✓ or ✗ and
// the reason for each.

import { invoke } from "@tauri-apps/api/core";
import { Bridge, onEvent } from "../core/bridge";
import { FlowBridge } from "../core/flow";
import { speak } from "../voice/speak";
import { h, clear } from "../views/dom";
import { t } from "../i18n/i18n";
import type { Settings } from "../core/state";

export type TestStage = "mic" | "speech" | "brain" | "voice";

export interface StageResult {
  stage: TestStage;
  ok: boolean | null; // null: skipped, an earlier stage it needs failed
  detail: string;
}

export interface TestSteps {
  /** The loudest level (0…1) heard while the microphone is open, then it keeps recording. */
  micLevel(): Promise<number>;
  /** The next 3 seconds, as text. */
  record(): Promise<string>;
  probe(): Promise<{ answer: string; brain: string }>;
  /** null when spoken, else why not. */
  say(text: string): Promise<string | null>;
}

/** Below this the microphone is too quiet to have heard anything. */
export const MIN_LEVEL = 0.15;

const why = (e: unknown) => String(e).replace(/^Error:\s*/, "");

/** Runs every stage; a stage that needs a failed one is skipped, the others still run. */
export async function runVoiceTest(steps: TestSteps, onStage: (r: StageResult) => void = () => {}): Promise<StageResult[]> {
  const out: StageResult[] = [];
  const done = (r: StageResult) => {
    out.push(r);
    onStage(r);
  };
  let micOk = false;
  try {
    const level = await steps.micLevel();
    micOk = level >= MIN_LEVEL;
    done({ stage: "mic", ok: micOk, detail: micOk ? t("Level {0} %", { 0: Math.round(level * 100) }) : t("Too quiet ({0} %): check the microphone, or speak closer.", { 0: Math.round(level * 100) }) });
  } catch (e) {
    done({ stage: "mic", ok: false, detail: why(e) });
  }
  if (micOk) {
    try {
      const text = (await steps.record()).trim();
      done({ stage: "speech", ok: text.length > 0, detail: text ? `“${text}”` : t("Nothing was understood.") });
    } catch (e) {
      done({ stage: "speech", ok: false, detail: why(e) });
    }
  } else {
    done({ stage: "speech", ok: null, detail: t("Skipped: the microphone didn't hear you.") });
  }
  let answer = "";
  try {
    const r = await steps.probe();
    answer = r.answer.trim();
    done({ stage: "brain", ok: answer.length > 0, detail: answer ? t("{brain}: “{answer}”", { brain: r.brain, answer: answer.slice(0, 120) }) : t("{brain} gave an empty answer.", { brain: r.brain }) });
  } catch (e) {
    done({ stage: "brain", ok: false, detail: why(e) });
  }
  const spoken = await steps.say(answer || t("Hello, I'm Groovy.")).catch((e) => why(e));
  done({ stage: "voice", ok: spoken === null, detail: spoken ?? t("Spoken aloud.") });
  return out;
}

const STAGE_NAMES: Record<TestStage, () => string> = {
  mic: () => t("1. Microphone"),
  speech: () => t("2. Speech to text (3 s)"),
  brain: () => t("3. The AI answers"),
  voice: () => t("4. The answer is spoken"),
};

/** The button and its report, for Settings → Voice. */
export function voiceTestBlock(settings: Settings): HTMLElement {
  const report = h("div", { class: "stack" });
  const run = h("button", { text: t("Test my voice setup") }) as HTMLButtonElement;
  run.addEventListener("click", () => {
    run.disabled = true;
    clear(report);
    const prompt = h("div", { class: "hint", text: t("Say something when asked: first anything, then a short sentence.") });
    report.append(prompt);
    let peak = 0;
    const steps: TestSteps = {
      async micLevel() {
        const mic = (await FlowBridge.status())?.prefs.mic ?? "";
        const off = await onEvent<number>("speech-level", (v) => (peak = Math.max(peak, v)));
        try {
          await invoke<void>("speech_check_start", { mic });
          prompt.textContent = t("Say anything now…");
          await new Promise((r) => setTimeout(r, 2000));
        } finally {
          off?.();
        }
        return peak;
      },
      async record() {
        prompt.textContent = t("Now say a short sentence (3 seconds)…");
        await invoke<void>("speech_check_reset");
        await new Promise((r) => setTimeout(r, 3000));
        prompt.textContent = t("Writing it down…");
        return await invoke<string>("speech_check_stop");
      },
      async probe() {
        prompt.textContent = t("Asking the AI…");
        return await Bridge.voiceProbe();
      },
      async say(text) {
        prompt.textContent = t("Speaking…");
        return await speak(text, settings.voiceTts);
      },
    };
    void runVoiceTest(steps, (r) => {
      report.append(h("div", { class: r.ok ? "notice ok" : r.ok === null ? "hint" : "notice err" },
        h("b", { text: `${r.ok ? "✓" : r.ok === null ? "–" : "✗"} ${STAGE_NAMES[r.stage]()}` }),
        h("span", { text: ` ${r.detail}` }),
      ));
    }).finally(() => {
      void invoke<void>("speech_check_cancel").catch(() => {});
      prompt.remove();
      run.disabled = false;
    });
  });
  return h("div", { class: "stack" },
    h("div", { class: "row" }, run),
    h("div", { class: "hint", text: t("Checks each stage of a call in turn. Details go to %LOCALAPPDATA%\\Groovy\\logs\\voice.log.") }),
    report,
  );
}
