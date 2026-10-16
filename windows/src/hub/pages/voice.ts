// « OK Groovy »: say it, then ask anything. Turning it on, how Groovy answers
// out loud, and what leaves the computer. Port of the Mac's Settings → Voice.

import { needsSetup } from "./speechsetup";
import { h, clear } from "../../views/dom";
import { t } from "../../i18n/i18n";
import { Bridge, onEvent } from "../../core/bridge";
import { FlowBridge, type OkTestStep, type VoiceOutput } from "../../core/flow";
import type { HubContext } from "../shell";
import { button, card, divider, label, row, segmented, toggle } from "../ui";
import { modelsControl } from "./flowsettings";

const ELEVEN = "elevenlabs-api-key";
/** Where secrets.rs keeps the keys on this OS (same words as Groovy → Chat). */
const KEY_STORE = navigator.userAgent.includes("Windows") ? "Windows Credential Manager" : "Secret Service (GNOME Keyring, KWallet)";

function elevenKey(): HTMLElement {
  const box = h("div", { style: "display:flex;align-items:center;gap:8px" });
  const draw = async () => {
    const present = (await Bridge.secretPresent(ELEVEN)) ?? false;
    clear(box);
    if (present) {
      box.append(
        h("span", { class: "hub-note", style: "color:var(--hub-green)", text: `✓ ${t("Key saved in the {store}.", { store: KEY_STORE })}` }),
        button(t("Remove"), () => void Bridge.secretClear(ELEVEN).then(draw), "", true),
      );
      return;
    }
    const input = h("input", { class: "hub-input", type: "password", placeholder: t("ElevenLabs key"), autocomplete: "off", spellcheck: "false", style: "width:220px" }) as HTMLInputElement;
    const save = button(t("Save key"), () => {
      const value = input.value.trim();
      if (!value) return;
      void Bridge.secretSet(ELEVEN, value).then(draw, () => input.classList.add("err"));
    }, "primary", true);
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") save.click();
    });
    box.append(input, save);
  };
  void draw();
  return box;
}

/** ✓/✗ for the microphone, the speech model and the AI, and "Test OK Groovy" (round 4 item 1). */
function checkCard(): HTMLElement {
  const list = h("div", { class: "ok-checks" });
  const mark = (ok: boolean, text: string) => h("div", { class: ok ? "ok-check ok" : "ok-check bad" }, h("span", { text: ok ? "✓" : "✗" }), h("span", { text }));
  const draw = async () => {
    const s = await FlowBridge.okStatus();
    clear(list);
    if (!s) return;
    list.append(
      mark(s.on, s.on ? (s.listening ? t("« OK Groovy » is on and listening") : t("« OK Groovy » is on, not listening yet")) : t("« OK Groovy » is off")),
      mark(!s.micError && !!s.mic, s.micError ? t("Microphone “{mic}” didn't open: {error}", { mic: s.mic, error: s.micError }) : t("Listening to “{mic}”", { mic: s.mic || t("the default microphone") })),
      mark(s.speech, s.speech ? t("Speech model ready") : t("Speech model not downloaded")),
      mark(!!s.ai, s.ai ? t("AI: {name}", { name: s.ai }) : t("No AI: music, volume, the time, timers and opening apps still work")),
    );
  };
  const steps = h("ol", { class: "ok-steps" });
  const STEPS: [OkTestStep["step"], string][] = [
    ["wake", t("Heard « OK Groovy »")],
    ["words", t("Understood the words")],
    ["answer", t("Got an answer")],
    ["spoken", t("Said it out loud")],
  ];
  const test = button(t("Test OK Groovy"), () => {
    clear(steps);
    for (const [id, text] of STEPS) steps.append(h("li", { class: "ok-step", "data-step": id, text: `… ${text}` }));
    hint.textContent = t("Say « OK Groovy, what time is it? »");
    FlowBridge.okTest().catch((err: unknown) => {
      clear(steps);
      hint.textContent = String(err);
    });
  }, "primary", true);
  const hint = h("div", { class: "hub-note" });
  void onEvent<OkTestStep>("okgroovy-test", (step) => {
    const li = steps.querySelector<HTMLElement>(`[data-step="${step.step}"]`);
    const text = STEPS.find(([id]) => id === step.step)?.[1] ?? step.step;
    if (li) {
      li.textContent = `${step.ok ? "✓" : "✗"} ${text}${step.detail ? `: ${step.detail}` : ""}`;
      li.className = step.ok ? "ok-step ok" : "ok-step bad";
    }
    if (step.step === "spoken" || !step.ok) void draw();
  });
  void draw();
  return card(list, divider(), row(t("Test OK Groovy"), t("Shows each step of the next « OK Groovy »."), test), steps, hint);
}

export async function voicePage(page: HTMLElement, ctx: HubContext) {
  const status = await ctx.refresh();
  const prefs = status.prefs;
  page.append(ctx.hero("voice", "OK Groovy", t("Say « OK Groovy », then ask anything: add a pill, play music, the weather, an email to prepare.")));

  const ready = status.models.parakeet.state === "ready";
  page.append(label(t("Listening")));
  const main = card(
    row(t("Listen for « OK Groovy »"), t("The microphone stays open; only what follows « OK Groovy » goes further."),
      toggle(prefs.voice, (on) => {
        if (on && needsSetup(ctx, "voice")) return;
        void ctx.setPrefs((p) => (p.voice = on)).then(() => ctx.navigate("voice"));
      })),
    divider(),
    row(t("Parakeet, on your computer"), t("Hears « OK Groovy » here, without the internet."), modelsControl(ctx, () => ctx.navigate("voice"))),
  );
  if (prefs.voice && !ready) {
    main.append(h("div", { class: "hub-warn", text: t("Download the voice model so Groovy can hear you.") }));
  }
  page.append(main);
  page.append(label(t("Check")));
  page.append(checkCard());

  page.append(label(t("Answers")));
  const outputs: [VoiceOutput, string][] = [["system", t("System voice")], ["elevenlabs", "ElevenLabs"], ["off", t("Text only")]];
  const answers = card(
    row(t("Groovy answers out loud"), null, segmented(outputs, prefs.voiceOutput, (v) => void ctx.setPrefs((p) => (p.voiceOutput = v)).then(() => ctx.navigate("voice")))),
  );
  if (prefs.voiceOutput === "elevenlabs") {
    answers.append(
      divider(),
      row(t("Voice"), null, segmented([["female", t("Female")], ["male", t("Male")]], prefs.voiceGender, (g) => void ctx.setPrefs((p) => (p.voiceGender = g)))),
      divider(),
      row(t("ElevenLabs key"), null, elevenKey()),
    );
  }
  page.append(answers);

  page.append(label(t("Privacy")));
  page.append(card(
    h("div", { class: "hub-note", text: t("Your voice never leaves this computer: Parakeet turns it into text here. What follows « OK Groovy » is answered by the local model; it goes to Anthropic only if you chose Claude as the brain, and to ElevenLabs only if you picked it for the answers.") }),
    divider(),
    h("div", { class: "hub-note", text: t("An email is never sent by Groovy: your mail app opens with it written, and you send it.") }),
  ));
}
