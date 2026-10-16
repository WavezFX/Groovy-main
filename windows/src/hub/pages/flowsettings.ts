// Flow → Settings: the key, the microphone, the voice
// model, the Notetaker's summaries, the window and the history. Port of
// FlowHubSettings.swift.

import { needsSetup } from "./speechsetup";
import { h, clear } from "../../views/dom";
import { t } from "../../i18n/i18n";
import { Bridge, onEvent } from "../../core/bridge";
import { showError } from "../../views/toast";
import { FlowBridge, type FlowKey, type ModelsStatus } from "../../core/flow";
import type { HubContext } from "../shell";
import { button, card, divider, keyKeycap, keycap, label, number, row, segmented, select, toggle } from "../ui";

/** Download / progress / installed, for the voice model. Redraws itself. */
export function modelsControl(ctx: HubContext, onReady?: () => void): HTMLElement {
  const el = h("div", { style: "display:flex;align-items:center;gap:8px" });
  let confirming = false;
  const draw = (models: ModelsStatus) => {
    clear(el);
    const m = models.parakeet;
    if (m.state === "missing") {
      el.append(button(t("Download"), () => void FlowBridge.download(), "primary"));
    } else if (m.state === "downloading") {
      const bar = h("div", { class: "hub-progress" }, h("i", { style: `width:${Math.round(m.fraction * 100)}%` }));
      el.append(bar, h("span", { class: "hub-note", text: `${Math.round(m.fraction * 100)} %` }));
    } else if (m.state === "failed") {
      el.append(h("span", { class: "hub-note", text: m.error, title: m.error, style: "max-width:220px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap" }));
      el.append(button(t("Retry"), () => void FlowBridge.download()));
    } else if (confirming) {
      el.append(
        button(t("Remove the models"), () => {
          confirming = false;
          void FlowBridge.removeModels();
        }, "danger", true),
        button(t("Cancel"), () => {
          confirming = false;
          draw(ctx.status.models);
        }, "", true),
      );
    } else {
      const ok = h("button", { class: "hub-link", style: "color:var(--hub-green)", text: `✓ ${t("Installed")}`, title: t("Remove the models") });
      ok.addEventListener("click", () => {
        confirming = true;
        draw(ctx.status.models);
      });
      el.append(ok);
    }
  };
  draw(ctx.status.models);
  void onEvent<ModelsStatus>("speech-models", (models) => {
    if (!el.isConnected) return;
    const wasReady = ctx.status.models.parakeet.state === "ready";
    ctx.status.models = models;
    draw(models);
    if (!wasReady && models.parakeet.state === "ready") onReady?.();
  });
  return el;
}

export async function flowSettingsPage(page: HTMLElement, ctx: HubContext) {
  const status = await ctx.refresh();
  const prefs = status.prefs;
  page.append(ctx.hero("flowsettings", t("Settings"), null));

  // ── Keys ──
  const keys: [FlowKey, string][] = [
    ["right_ctrl", t("Right {0}", { 0: "Ctrl" })],
    ["right_alt", t("Right {0}", { 0: "Alt" })],
    ["right_shift", t("Right {0}", { 0: "Shift" })],
    ["left_ctrl_super", status.platform === "windows" ? "Ctrl + Win" : "Ctrl + Super"],
  ];
  page.append(label(t("Keys")));
  const keysCard = card(
    row(t("Hold a key to dictate anywhere"), null, toggle(prefs.enabled, (on) => {
      if (on && needsSetup(ctx, "dictation")) return;
      void ctx.setPrefs((p) => (p.enabled = on));
    })),
    divider(),
    status.keys === "wayland"
      ? row(t("Hold to dictate"), null, keyKeycap("wayland", status.platform))
      : row(t("Hold to dictate"), prefs.key === "right_alt" ? t("On a keyboard with AltGr, the right Alt types characters: pick another key.") : null,
        select(keys, prefs.key, (key) => void ctx.setPrefs((p) => (p.key = key)).then(() => ctx.navigate("flowsettings")))),
    divider(),
    row(t("Hands-free"), t("Tap twice, speak as long as you like, tap once to paste."), keyKeycap(status.keys === "wayland" ? "wayland" : prefs.key, status.platform, true)),
    divider(),
    row(t("Paste the last dictation again"), t("When the words went to the wrong place."), button(t("Paste again"), () => void FlowBridge.pasteAgain(), "", true)),
    divider(),
    row(t("Cancel a dictation"), null, keycap(t("Esc"))),
  );
  if (status.keys === "wayland") {
    keysCard.append(h("div", {
      class: "hub-warn",
      text: t("On Wayland, the desktop holds the dictation key for Groovy: Ctrl+Alt+V, or the keys you picked when it asked. The first paste asks once to let Groovy type for you."),
    }));
  } else if (status.keys !== "ok") {
    keysCard.append(h("div", { class: "hub-warn", text: t("Groovy can't watch the keyboard here: use the Dictate button in the island.") }));
  }
  page.append(keysCard);

  // ── Microphone ──
  page.append(label(t("Microphone")));
  const mics: [string, string][] = [["", t("System default")], ...status.mics.map((m): [string, string] => [m.name, m.name])];
  if (prefs.mic && !mics.some(([id]) => id === prefs.mic)) mics.push([prefs.mic, prefs.mic]);
  page.append(card(row(t("Microphone"), t("The one Groovy listens to for dictation and calls."), select(mics, prefs.mic, (mic) => void ctx.setPrefs((p) => (p.mic = mic))))));

  // ── Recognition ──
  page.append(label(t("Recognition")));
  const cloudLine = h("div", { class: "hub-note" });
  if (prefs.cloudSpeech) {
    void FlowBridge.cloudServices().then((services) => {
      cloudLine.textContent = services?.length
        ? t("In use: {0}, then this computer.", { 0: services.join(" → ") })
        : t("No key yet: this computer's engine does it all.");
    });
  }
  const engines: ["parakeet" | "whisper", string][] = [["parakeet", "Parakeet"], ["whisper", "Whisper"]];
  page.append(card(
    row(t("Speech engine"), t("Parakeet knows 25 European languages. Whisper knows about 99 (Punjabi, Hindi, Arabic, Japanese, Chinese…) and uses the model and language picked in Groovy → Voice calls."),
      segmented(engines, prefs.engine, (engine) => void ctx.setPrefs((p) => (p.engine = engine)).then(() => ctx.navigate("flowsettings")))),
    prefs.engine === "whisper" && !status.engineReady
      ? h("div", { class: "hub-warn", text: t("Download a multilingual Whisper model (base or small) in Groovy → Voice calls.") })
      : null,
    divider(),
    row(t("Parakeet, on your computer"), t("The most accurate, offline, 25 languages. Downloaded once (about {0} MB).", { 0: number(status.models.downloadMb) }), modelsControl(ctx)),
    divider(),
    // Round 3B item 6: names, brands and model numbers right.
    row(t("Cloud speech first"), t("With a Deepgram, OpenAI, ElevenLabs or Google AI key, dictation goes to that service first: it hears names and model numbers better. This computer's engine takes over without a key or a network."),
      toggle(prefs.cloudSpeech, (on) => void ctx.setPrefs((p) => (p.cloudSpeech = on)).then(() => ctx.navigate("flowsettings")))),
    cloudLine,
    keyRow("deepgram-api-key", t("Deepgram key"), "https://console.deepgram.com/"),
    divider(),
    row(t("Learn names from my corrections"), t("Words you fix within a minute of dictating (a brand, a model, a name) join the Dictionary, and every engine is told them."),
      toggle(prefs.learnWords, (on) => void ctx.setPrefs((p) => (p.learnWords = on)))),
    h("div", { class: "hub-note", text: prefs.cloudSpeech
      ? t("With a key, the recording goes to that service only, to be turned into text, then is dropped. Without one, it never leaves this computer.")
      : t("Your voice never leaves this computer: Parakeet or Whisper turns it into text here, and the recording is dropped as soon as it is pasted.") }),
  ));

  // ── Notetaker ──
  page.append(label(t("Notetaker")));
  page.append(card(
    row(t("Summaries"), null, select([["ai", t("Groovy's AI (local by default)")], ["off", t("No summary")]], prefs.summaryEngine, (engine) => void ctx.setPrefs((p) => (p.summaryEngine = engine)))),
    h("div", { class: "hub-note", text: t("Summaries are written by the local model on this computer. Only if you chose Claude as the brain does the transcript (never the audio) go to Anthropic.") }),
    divider(),
    h("div", { class: "hub-note", text: t("Everything stays on this computer. Tell the people in the call that you are recording.") }),
  ));

  // ── App ──
  page.append(label(t("App")));
  page.append(card(
    row(t("Open this window when you open Groovy"), t("Not when Groovy starts with the computer."), toggle(prefs.openAtLaunch, (on) => void ctx.setPrefs((p) => (p.openAtLaunch = on)))),
  ));

  // ── Privacy ──
  page.append(label(t("Privacy")));
  const keep: [string, string][] = [["0", t("Don't keep")], ["7", t("7 days")], ["30", t("30 days")], ["365", t("1 year")], ["-1", t("Forever")]];
  const confirmRow = h("div", { class: "hub-row" });
  const erase = () => {
    clear(confirmRow);
    confirmRow.append(
      button(t("Erase the history"), () => void FlowBridge.clear(false), "", true),
      button(t("Erase everything"), () => {
        clear(confirmRow);
        confirmRow.append(
          h("span", { class: "hub-note", style: "flex:1", text: t("Erase the history and the statistics?") }),
          button(t("Erase everything"), () => {
            void FlowBridge.clear(true);
            erase();
          }, "danger", true),
          button(t("Cancel"), erase, "", true),
        );
      }, "", true),
    );
  };
  erase();
  page.append(card(
    row(t("Keep the history"), null, select(keep, String(prefs.keepDays), (v) => void ctx.setPrefs((p) => (p.keepDays = Number(v))))),
    divider(),
    confirmRow,
    h("div", { class: "hub-note", text: t("The history, the statistics and your notes stay on this computer, in Groovy's folder. Dictations are never saved as audio; a call's audio is deleted as soon as its transcript is written.") }),
  ));
}

/** A key field for a speech service, with where to get one: kept in Windows' credential store. */
function keyRow(key: string, name: string, page: string): HTMLElement {
  const input = h("input", { type: "password", placeholder: name, autocomplete: "off", spellcheck: "false", style: "flex:1" }) as HTMLInputElement;
  const save = button(t("Save"), () => {
    const value = input.value.trim();
    if (!value) return;
    void Bridge.secretSet(key, value).then(() => {
      input.value = "";
      input.placeholder = t("{0} saved", { 0: name });
    }, showError);
  });
  void Bridge.secretPresent(key).then((present) => {
    if (present) input.placeholder = t("{0} saved", { 0: name });
  });
  const get = h("a", { href: "#", text: t("Get a key ↗"), onclick: (e: Event) => { e.preventDefault(); void Bridge.openUrl(page); } });
  return h("div", { class: "hub-row" }, input, save, get);
}
