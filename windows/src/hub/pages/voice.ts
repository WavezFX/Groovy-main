// « OK Groovy »: say it, then ask anything. Turning it on, how Groovy answers
// out loud, and what leaves the computer. Port of the Mac's Settings → Voice.

import { needsSetup } from "./speechsetup";
import { h, clear } from "../../views/dom";
import { t } from "../../i18n/i18n";
import { Bridge, onEvent } from "../../core/bridge";
import { FlowBridge, type FlowPrefs, type OkTestStep, type VoiceOutput } from "../../core/flow";
import type { HubContext } from "../shell";
import { button, card, divider, label, row, segmented, toggle } from "../ui";
import { modelsControl } from "./flowsettings";

const ELEVEN = "elevenlabs-api-key";
/** Where secrets.rs keeps the keys on this OS (same words as Groovy → Chat). */
const KEY_STORE = navigator.userAgent.includes("Windows") ? "Windows Credential Manager" : "Secret Service (GNOME Keyring, KWallet)";

function elevenKey(): HTMLElement {
  return keyBox(ELEVEN, t("ElevenLabs key"));
}

/** A key saved in the OS store, or a field to paste one: never shown back. */
function keyBox(secret: string, placeholder: string, onChange?: () => void): HTMLElement {
  const box = h("div", { style: "display:flex;align-items:center;gap:8px" });
  const draw = async () => {
    const present = (await Bridge.secretPresent(secret)) ?? false;
    clear(box);
    if (present) {
      box.append(
        h("span", { class: "hub-note", style: "color:var(--hub-green)", text: `✓ ${t("Key saved in the {store}.", { store: KEY_STORE })}` }),
        button(t("Remove"), () => void Bridge.secretClear(secret).then(draw).then(() => onChange?.()), "", true),
      );
      return;
    }
    const input = h("input", { class: "hub-input", type: "password", placeholder, autocomplete: "off", spellcheck: "false", style: "width:220px" }) as HTMLInputElement;
    const save = button(t("Save key"), () => {
      const value = input.value.trim();
      if (!value) return;
      void Bridge.secretSet(secret, value).then(draw, () => input.classList.add("err")).then(() => onChange?.());
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

const FISH = "fish-audio-api-key";

/**
 * Groovy's own voice (round 3B item 8c): Fish Audio with the user's key and a
 * voice id, first whenever it is set up; Kokoro, offline, after it.
 */
function groovyVoiceCard(ctx: HubContext, prefs: FlowPrefs): HTMLElement {
  const body = card();
  const status = h("div", { class: "hub-note" });
  const kokoroBox = h("div", { style: "display:flex;align-items:center;gap:8px" });
  const usage = h("span", { class: "hub-note" });
  const voiceId = h("input", { class: "hub-input", placeholder: t("Voice ID from fish.audio"), spellcheck: "false", style: "width:240px", value: prefs.fishVoice }) as HTMLInputElement;
  voiceId.addEventListener("change", () => void ctx.setPrefs((p) => (p.fishVoice = voiceId.value.trim())).then(draw));
  const model = h("select", { class: "hub-input" }) as HTMLSelectElement;
  const testOut = h("span", { class: "hub-note" });
  const test = button(t("Test voice"), () => {
    testOut.textContent = "…";
    FlowBridge.fishTest().then(
      (a) => {
        testOut.textContent = "";
        void new Audio(`data:${a.mime};base64,${a.data}`).play();
      },
      (err: unknown) => (testOut.textContent = String(err)),
    );
  }, "", true);
  async function draw() {
    const s = await FlowBridge.ttsStatus();
    if (!s) return;
    if (model.options.length === 0) {
      for (const m of s.fishModels) model.append(h("option", { value: m, text: m }));
      model.value = s.fishModels.includes(prefs.fishModel) ? prefs.fishModel : s.fishModels[0];
    }
    const who = s.speaking === "fish" ? "Fish Audio" : s.speaking === "kokoro" ? "Kokoro" : t("the voice chosen below");
    status.textContent = s.fishNote ?? t("Groovy speaks with {who}.", { who });
    status.className = s.fishNote ? "hub-warn" : "hub-note";
    usage.textContent = s.fishKey ? t("About £{gbp} this month ({chars} characters sent)", { gbp: s.monthGbp.toFixed(2), chars: String(s.monthChars) }) : "";
    clear(kokoroBox);
    const k = s.kokoro;
    if (k.state === "ready") {
      kokoroBox.append(h("span", { class: "hub-note", style: "color:var(--hub-green)", text: `✓ ${t("Downloaded")}` }), button(t("Remove"), () => void FlowBridge.kokoroRemove(), "", true));
    } else if (k.state === "downloading") {
      kokoroBox.append(h("span", { class: "hub-note", text: t("Downloading… {pct}%", { pct: String(Math.round(k.fraction * 100)) }) }));
    } else {
      if (k.state === "failed") kokoroBox.append(h("span", { class: "hub-warn", text: k.error }));
      kokoroBox.append(button(t("Download ({mb} MB)", { mb: String(s.kokoroMb) }), () => void FlowBridge.kokoroDownload(), "primary", true));
    }
  }
  model.addEventListener("change", () => void ctx.setPrefs((p) => (p.fishModel = model.value)));
  void onEvent<null>("tts-status-changed", () => void draw());
  body.append(
    status,
    divider(),
    row(t("Fish Audio key"), t("Your own key, kept in the {store}. Fish Audio bills by text length; its terms apply.", { store: KEY_STORE }), keyBox(FISH, t("Fish Audio key"), () => void draw())),
    divider(),
    row(t("Voice ID"), t("The model ID on the voice's page at fish.audio."), voiceId),
    divider(),
    row(t("Model"), null, model, test),
    h("div", { style: "display:flex;justify-content:space-between;gap:8px" }, testOut, usage),
    divider(),
    row(t("Kokoro, on your computer"), t("Groovy's offline voice: used when Fish Audio isn't set up or doesn't answer. Downloaded only when you ask."), kokoroBox),
  );
  void draw();
  return body;
}

/** Voice → Listening (round 3D item 1): « OK Groovy », its check, and what leaves the PC. */
export async function listeningPage(page: HTMLElement, ctx: HubContext) {
  const status = await ctx.refresh();
  const prefs = status.prefs;
  page.append(ctx.hero("listening", t("Listening"), t("Say « OK Groovy », then ask anything: add a pill, play music, the weather, an email to prepare.")));

  const ready = status.models.parakeet.state === "ready";
  page.append(label(t("Listening")));
  const main = card(
    row(t("Listen for « OK Groovy »"), t("The microphone stays open; only what follows « OK Groovy » goes further."),
      toggle(prefs.voice, (on) => {
        if (on && needsSetup(ctx, "voice")) return;
        void ctx.setPrefs((p) => (p.voice = on)).then(() => ctx.navigate("listening"));
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

  page.append(label(t("Privacy")));
  page.append(privacyCard());
}

function privacyCard(): HTMLElement {
  return card(
    h("div", { class: "hub-note", text: t("Your voice never leaves this computer: Parakeet turns it into text here. What follows « OK Groovy » is answered by the local model; it goes to Anthropic only if you chose Claude as the brain, and to ElevenLabs only if you picked it for the answers.") }),
    divider(),
    h("div", { class: "hub-note", text: t("An email is never sent by Groovy: your mail app opens with it written, and you send it.") }),
  );
}

/** Voice → Speaking (round 3D item 1): his voice (Fish Audio, Kokoro, the test) and how he answers. */
export async function speakingPage(page: HTMLElement, ctx: HubContext) {
  const status = await ctx.refresh();
  const prefs = status.prefs;
  page.append(ctx.hero("speaking", t("Speaking"), t("How Groovy sounds when he answers out loud.")));
  page.append(label(t("Groovy's voice")));
  page.append(groovyVoiceCard(ctx, prefs));
  page.append(label(t("Answers")));
  const outputs: [VoiceOutput, string][] = [["system", t("System voice")], ["elevenlabs", "ElevenLabs"], ["off", t("Text only")]];
  const answers = card(
    row(t("Groovy answers out loud"), null, segmented(outputs, prefs.voiceOutput, (v) => void ctx.setPrefs((p) => (p.voiceOutput = v)).then(() => ctx.navigate("speaking")))),
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
}
