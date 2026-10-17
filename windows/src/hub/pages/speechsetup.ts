// The guided speech setup (FIXES_ROUND1 item 3), shown the first time
// dictation, the Notetaker or « OK Groovy » is turned on, and from the pill
// when the model is missing: 1. the microphone, with a live level meter;
// 2. the speech model, with its size and a progress bar; 3. a five-second test
// that shows the words heard. Finishing turns on what was asked for.

import { invoke } from "@tauri-apps/api/core";
import { h } from "../../views/dom";
import { t } from "../../i18n/i18n";
import { IS_TAURI, onEvent } from "../../core/bridge";
import type { FlowPrefs } from "../../core/flow";
import { showError, errorText } from "../../views/toast";
import type { HubContext } from "../shell";
import { button, card, divider, label, number, row, select } from "../ui";
import { modelsControl } from "./flowsettings";

/** What the setup turns on when done ("" when opened by itself). */
let pending: "" | "dictation" | "voice" = "";

/** Opens the setup instead of turning `what` on, when speech isn't set up yet. Returns true when it did. */
export function needsSetup(ctx: HubContext, what: "dictation" | "voice"): boolean {
  if (ctx.status.prefs.speechSetupDone && ctx.status.engineReady) return false;
  pending = what;
  ctx.navigate("speechsetup");
  return true;
}

const call = <T>(cmd: string, args?: Record<string, unknown>) => (IS_TAURI ? invoke<T>(cmd, args) : Promise.reject(new Error("not running inside Groovy")));

export async function speechSetupPage(page: HTMLElement, ctx: HubContext) {
  const status = await ctx.refresh();
  const prefs = status.prefs;
  page.append(ctx.hero("dictation", t("Set up speech"), t("Three steps, about two minutes: the microphone, the speech model, and a test.")));

  // ── 1. Microphone ──
  const meter = h("div", { class: "hub-progress", style: "flex:1;min-width:120px" }, h("i", { style: "width:0%;transition:width 60ms linear" }));
  const fill = meter.firstElementChild as HTMLElement;
  const mics: [string, string][] = [["", t("System default")], ...status.mics.map((m): [string, string] => [m.name, m.name])];
  if (prefs.mic && !mics.some(([id]) => id === prefs.mic)) mics.push([prefs.mic, prefs.mic]);
  const micNote = h("div", { class: "hub-note", text: t("Speak: the bar should move. If it doesn't, pick another microphone.") });
  const open = (mic: string) =>
    call<void>("speech_check_start", { mic }).then(
      () => (micNote.className = "hub-note"),
      (e) => {
        micNote.className = "hub-warn";
        micNote.textContent = errorText(e);
      },
    );
  page.append(label(t("1. Microphone")), card(
    row(t("Microphone"), null, select(mics, prefs.mic, (mic) => {
      void ctx.setPrefs((p) => (p.mic = mic)).then(() => open(mic));
    })),
    h("div", { class: "hub-row" }, meter),
    micNote,
  ));
  const offLevel = await onEvent<number>("speech-level", (v) => (fill.style.width = `${Math.round(v * 100)}%`));
  void open(prefs.mic);

  // ── 2. Speech model ──
  const engines: [FlowPrefs["engine"], string][] = [["parakeet", t("Parakeet (25 European languages)")], ["whisper", t("Whisper (99 languages, chosen in Voice calls)")]];
  const modelCard = card(
    row(t("Speech engine"), null, select(engines, prefs.engine, (engine) => void ctx.setPrefs((p) => (p.engine = engine)).then(() => ctx.navigate("speechsetup")))),
    divider(),
  );
  if (prefs.engine === "whisper") {
    modelCard.append(
      status.engineReady
        ? h("div", { class: "hub-ok", text: t("The Whisper model is ready.") })
        : h("div", { class: "hub-warn", text: t("Download a multilingual Whisper model (base or small) in Groovy → Voice calls.") }),
      button(t("Open Voice calls"), () => ctx.navigate("calls")),
    );
  } else {
    modelCard.append(row(
      t("Parakeet, on your computer"),
      t("The most accurate, offline, 25 languages. Downloaded once (about {0} MB).", { 0: number(status.models.downloadMb) }),
      modelsControl(ctx, () => ctx.navigate("speechsetup")),
    ));
  }
  page.append(label(t("2. Speech model")), modelCard);

  // ── 3. Test ──
  const result = h("div", { class: "hub-note", text: t("Press Test, then say a sentence. Five seconds.") });
  let testing = false;
  const test = button(t("Test (5 s)"), () => {
    if (testing) return;
    testing = true;
    test.disabled = true;
    let left = 5;
    result.className = "hub-note";
    result.textContent = t("Listening… {0}", { 0: left });
    void call<void>("speech_check_reset").catch(() => {});
    const tick = setInterval(() => {
      left -= 1;
      if (left > 0) {
        result.textContent = t("Listening… {0}", { 0: left });
        return;
      }
      clearInterval(tick);
      result.textContent = t("Writing it down…");
      void call<string>("speech_check_stop").then(
        (heard) => {
          result.className = "hub-ok";
          result.textContent = t("Heard: “{0}”", { 0: heard });
          finish.disabled = false;
        },
        (e) => {
          result.className = "hub-warn";
          result.textContent = errorText(e);
        },
      ).finally(() => {
        testing = false;
        test.disabled = false;
        void open(ctx.status.prefs.mic);
      });
    }, 1000);
  }, "primary");
  const finish = button(pending === "voice" ? t("Done: turn on « OK Groovy »") : pending === "dictation" ? t("Done: turn on dictation") : t("Done"), () => {
    const what = pending;
    pending = "";
    void ctx.setPrefs((p) => {
      p.speechSetupDone = true;
      if (what === "dictation") p.enabled = true;
      if (what === "voice") p.voice = true;
    }).then(() => ctx.navigate(what === "voice" ? "voice" : "dictation"), showError);
  }, "primary");
  finish.disabled = !(status.prefs.speechSetupDone && status.engineReady);
  page.append(label(t("3. Test")), card(h("div", { class: "hub-row" }, test, finish), result));

  ctx.onLeave = () => {
    offLevel();
    void call<void>("speech_check_cancel").catch(() => {});
  };
}
