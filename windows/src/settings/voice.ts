// Settings → Voice: the wake words, where speech is turned into text, and the
// optional LiveKit conversation mode.

import { Bridge, onEvent, type VoiceStatus } from "../core/bridge";
import type { Settings } from "../core/state";
import { h, clear } from "../views/dom";
import { statusDot } from "./parts";
import { t } from "../i18n/i18n";
import { speak, voices } from "../voice/speak";

/** Whisper models Groovy can download (English, ggml format from whisper.cpp). */
const MODELS: Array<[id: string, label: string]> = [
  ["tiny.en", "Tiny · fastest · 75 MB"],
  ["base.en", "Base · more accurate · 142 MB"],
];

interface Progress {
  model: string;
  done: number;
  total: number;
}

export function voiceSection(
  settings: Settings,
  save: () => void,
  toggle: (on: boolean, onChange: (v: boolean) => void) => HTMLElement,
  keyStore: string,
): HTMLElement {
  const dot = statusDot(false);
  const state = h("span", { class: "hint" });
  const feedback = h("div", {});
  let status: VoiceStatus | null = null;
  let downloading: string | null = null;

  // ── Engine ──
  const engine = h("select", {}) as HTMLSelectElement;
  engine.append(
    h("option", { value: "local", text: t("On this computer (Whisper)") }),
    h("option", { value: "livekit", text: t("LiveKit conversation") }),
  );
  engine.value = settings.voiceEngine;
  engine.addEventListener("change", () => {
    settings.voiceEngine = engine.value === "livekit" ? "livekit" : "local";
    save();
    draw();
  });

  // ── Whisper model ──
  const model = h("select", {}) as HTMLSelectElement;
  for (const [id, label] of MODELS) model.append(h("option", { value: id, text: label }));
  model.value = settings.voiceModel;
  model.addEventListener("change", () => {
    settings.voiceModel = model.value;
    save();
    void refresh();
  });
  const download = h("button", { class: "primary", text: t("Download") });
  const remove = h("button", { class: "danger", text: t("Remove") });
  const bar = h("progress", { max: "1", value: "0", style: "flex:1 1 auto;display:none" }) as HTMLProgressElement;

  download.addEventListener("click", async () => {
    if (downloading) return;
    downloading = model.value;
    clear(feedback);
    draw();
    try {
      await Bridge.voiceDownloadModel(model.value);
      feedback.append(h("div", { class: "notice ok", text: t("Model ready. Say “Hey Groovy”.") }));
    } catch (err) {
      feedback.append(h("div", { class: "notice err", text: t("Could not download: {error}", { error: String(err) }) }));
    } finally {
      downloading = null;
      await refresh();
    }
  });
  remove.addEventListener("click", async () => {
    clear(feedback);
    try {
      await Bridge.voiceDeleteModel(model.value);
    } catch (err) {
      feedback.append(h("div", { class: "notice err", text: t("Could not remove: {error}", { error: String(err) }) }));
    }
    await refresh();
  });
  void onEvent<Progress>("voice-model-progress", (p) => {
    if (p.model !== downloading) return;
    bar.style.display = "";
    bar.value = p.total > 0 ? p.done / p.total : 0;
  });

  // ── LiveKit ──
  const url = h("input", {
    type: "text", placeholder: "wss://your-project.livekit.cloud", value: settings.livekitUrl,
    style: "flex:1 1 auto;min-width:0", spellcheck: "false",
  }) as HTMLInputElement;
  url.addEventListener("change", () => {
    settings.livekitUrl = url.value.trim();
    save();
  });
  const room = h("input", {
    type: "text", value: settings.livekitRoom, style: "width:160px", spellcheck: "false",
  }) as HTMLInputElement;
  room.addEventListener("change", () => {
    settings.livekitRoom = room.value.trim() || "groovy";
    room.value = settings.livekitRoom;
    save();
  });
  const keyField = secretField("livekit-api-key", "API…");
  const secretFieldEl = secretField("livekit-api-secret", "••••");
  const livekitRows = h(
    "div",
    { class: "stack" },
    h("div", { class: "row" }, h("label", { text: t("Server") }), url),
    h("div", { class: "row" }, h("label", { text: t("Room") }), room),
    h("div", { class: "row" }, h("label", { text: t("API key") }), ...keyField.parts),
    h("div", { class: "row" }, h("label", { text: t("API secret") }), ...secretFieldEl.parts),
    h("div", {
      class: "hint",
      text: t("Groovy joins this room after the wake word and talks with the voice agent you run there (LiveKit Agents). The key and secret stay in the {store}; Groovy only uses them to sign its own short-lived token.", { store: keyStore }),
    }),
  );

  function secretField(key: string, placeholder: string) {
    const field = h("input", {
      type: "password", placeholder, autocomplete: "off", spellcheck: "false",
      style: "flex:1 1 auto;min-width:0",
    }) as HTMLInputElement;
    const saveBtn = h("button", { text: t("Save") });
    const note = h("span", { class: "hint" });
    const refreshKey = async () => {
      const present = (await Bridge.secretPresent(key)) ?? false;
      note.textContent = present ? t("(stored)") : "";
    };
    saveBtn.addEventListener("click", async () => {
      const value = field.value.trim();
      if (!value) return;
      try {
        await Bridge.secretSet(key, value);
        field.value = "";
      } catch (err) {
        feedback.append(h("div", { class: "notice err", text: t("Could not save: {error}", { error: String(err) }) }));
      }
      await refreshKey();
    });
    void refreshKey();
    return { parts: [field, saveBtn, note] };
  }

  // ── His voice ──
  const ttsVoice = h("select", { style: "flex:1 1 auto;min-width:0" }) as HTMLSelectElement;
  ttsVoice.append(h("option", { value: "", text: t("Automatic (offline English)") }));
  void voices().then((list) => {
    for (const v of list) {
      const label = v.localService ? `${v.name} · ${v.lang} · ${t("offline")}` : `${v.name} · ${v.lang}`;
      ttsVoice.append(h("option", { value: v.name, text: label }));
    }
    ttsVoice.value = settings.voiceTts;
    if (ttsVoice.value !== settings.voiceTts) ttsVoice.value = "";
  });
  ttsVoice.addEventListener("change", () => {
    settings.voiceTts = ttsVoice.value;
    save();
  });
  const tryVoice = h("button", { text: t("Try it") });
  tryVoice.addEventListener("click", () => void speak(t("Hey, I'm Groovy. What can I do for you?"), ttsVoice.value));
  const speakRows = h(
    "div",
    { class: "stack" },
    h("div", { class: "row" },
      h("label", { text: t("Keep talking, like a call") }),
      toggle(settings.voiceCall, (v) => { settings.voiceCall = v; save(); }),
    ),
    h("div", { class: "hint", text: t("After “Hey Groovy” he keeps listening after each answer until you say “bye” or go quiet. The microphone button in the chat starts a call too.") }),
    h("div", { class: "row" },
      h("label", { text: t("Read answers aloud") }),
      toggle(settings.voiceSpeak, (v) => { settings.voiceSpeak = v; save(); }),
    ),
    h("div", { class: "row" }, h("label", { text: t("His voice") }), ttsVoice, tryVoice),
  );

  const modelRow = h("div", { class: "row" }, h("label", { text: t("Speech model") }), model, download, remove, bar);
  const modelHint = h("div", {
    class: "hint",
    text: t("Speech is turned into text on this computer: no audio leaves it. The model is downloaded once from Hugging Face (whisper.cpp)."),
  });

  function draw() {
    const local = settings.voiceEngine !== "livekit";
    const available = status?.available ?? true;
    const installed = status?.installed.includes(model.value) ?? false;
    // The wake word is always heard by Whisper, LiveKit or not.
    modelRow.style.display = "";
    modelHint.style.display = local ? "" : "none";
    livekitRows.style.display = local ? "none" : "";
    speakRows.style.display = local ? "" : "none";
    download.style.display = installed || !available ? "none" : "";
    download.disabled = downloading != null;
    download.textContent = downloading ? t("Downloading…") : t("Download");
    remove.style.display = installed && !downloading ? "" : "none";
    if (!downloading) bar.style.display = "none";
    const ready = available && installed;
    dot.style.background = ready ? "#22c55e" : "#f4505e";
    state.textContent = !available
      ? t("This build of Groovy has no speech engine.")
      : ready
        ? t("Say “Groovy”, “Hey Groovy” or “Yo Groovy”, then what you want.")
        : t("Download a speech model to use your voice.");
  }

  async function refresh() {
    status = await Bridge.voiceStatus();
    draw();
  }
  void refresh();

  return h(
    "section",
    {},
    h("h2", {}, dot, h("span", { text: t("Voice") })),
    state,
    h("div", { class: "row" },
      h("label", { text: t("Listen for “Hey Groovy”") }),
      toggle(settings.voiceEnabled, (v) => { settings.voiceEnabled = v; save(); }),
    ),
    h("div", { class: "row" }, h("label", { text: t("Voice engine") }), engine),
    speakRows,
    modelRow,
    modelHint,
    livekitRows,
    feedback,
  );
}
