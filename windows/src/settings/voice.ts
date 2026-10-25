// Settings → Voice: the wake words, where speech is turned into text, and the
// optional LiveKit conversation mode.

import { FlowBridge, type FlowPrefs } from "../core/flow";
import { voiceTestBlock } from "./voicetest";
import { Bridge, onEvent, type VoiceStatus } from "../core/bridge";
import type { Settings } from "../core/state";
import { h, clear } from "../views/dom";
import { statusDot } from "./parts";
import { t } from "../i18n/i18n";
import { speak, voices } from "../voice/speak";
import { PARAKEET_LABEL, PARAKEET_OVER_LIMIT } from "./speechlabels";

/** Round 3D item 7: Parakeet is the accurate choice, not the default. */

/** Whisper models Groovy can download (ggml format from whisper.cpp; pinned in voice.rs). */
const MODELS: Array<[id: string, label: string]> = [
  ["base-q5_1", "Base · 99 languages · light · 57 MB"],
  ["base.en-q5_1", "Base · English · light · 57 MB"],
  ["tiny.en", "Tiny · English · fastest · 75 MB"],
  ["base.en", "Base · English · more accurate · 142 MB"],
  ["tiny", "Tiny · 99 languages · 74 MB"],
  ["base", "Base · 99 languages · 141 MB"],
  ["small", "Small · 99 languages · most accurate · 465 MB"],
];

/** Languages to listen for with a multilingual model, by their own names. "" lets Whisper detect it. */
export const SPEECH_LANGUAGES: Array<[code: string, name: string]> = [
  ["en", "English"], ["es", "Español"], ["de", "Deutsch"], ["pa", "ਪੰਜਾਬੀ"], ["hi", "हिन्दी"],
  ["fr", "Français"], ["pt", "Português"], ["it", "Italiano"], ["nl", "Nederlands"], ["pl", "Polski"],
  ["tr", "Türkçe"], ["ar", "العربية"], ["ur", "اردو"], ["bn", "বাংলা"], ["ru", "Русский"],
  ["uk", "Українська"], ["zh", "中文"], ["ja", "日本語"], ["ko", "한국어"], ["id", "Bahasa Indonesia"],
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
    h("option", { value: "local", text: t("On this computer") }),
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
  if (![...model.options].some((o) => o.value === settings.voiceModel)) model.value = "base-q5_1";
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

  // ── What he listens for, and with which microphone ──
  const language = h("select", {}) as HTMLSelectElement;
  language.append(h("option", { value: "", text: t("Detect automatically") }));
  for (const [code, name] of SPEECH_LANGUAGES) language.append(h("option", { value: code, text: name }));
  language.value = settings.voiceLanguage ?? "";
  language.addEventListener("change", () => {
    settings.voiceLanguage = language.value;
    save();
  });
  const languageRow = h("div", { class: "row" }, h("label", { text: t("I speak") }), language);
  const languageHint = h("div", { class: "hint", text: t("English-only models always hear English. The wake word is always “Hey Groovy”.") });

  const mic = h("select", { style: "flex:1 1 auto;min-width:0" }) as HTMLSelectElement;
  const listMics = async () => {
    clear(mic);
    mic.append(h("option", { value: "", text: t("System default") }));
    try {
      const devices = await navigator.mediaDevices.enumerateDevices();
      for (const d of devices.filter((d) => d.kind === "audioinput" && d.deviceId && d.deviceId !== "default")) {
        mic.append(h("option", { value: d.deviceId, text: d.label || t("Microphone {n}", { n: mic.options.length }) }));
      }
    } catch {
      // No media devices here (a plain browser page): the default is all there is.
    }
    mic.value = settings.voiceMic ?? "";
    if (mic.value !== (settings.voiceMic ?? "")) mic.value = "";
  };
  void listMics();
  navigator.mediaDevices?.addEventListener?.("devicechange", () => void listMics());
  mic.addEventListener("change", () => {
    settings.voiceMic = mic.value;
    save();
  });
  const micRow = h("div", { class: "row" }, h("label", { text: t("Microphone") }), mic);
  const interruptRow = h("div", { class: "row" },
    h("label", { text: t("Let me interrupt him") }),
    toggle(settings.voiceInterrupt ?? true, (v) => { settings.voiceInterrupt = v; save(); }),
    h("span", { class: "hint", text: t("Speak over him and he stops to listen. Use headphones, or he may hear himself.") }),
  );

  // Talk to Groovy (round 4 item 3): a real-time voice model, offered when its key is there.
  const instant = h("select", {}) as HTMLSelectElement;
  for (const [value, label] of [["off", t("Off: Groovy's own voice")], ["openai", "OpenAI Realtime"], ["gemini", "Gemini Live"]] as const) {
    instant.append(h("option", { value, text: label }));
  }
  instant.value = settings.instantVoice ?? "off";
  instant.addEventListener("change", () => {
    settings.instantVoice = instant.value as typeof settings.instantVoice;
    save();
  });
  void Bridge.instantVoiceOptions().then((have) => {
    for (const o of Array.from(instant.options)) {
      if (o.value !== "off") o.disabled = !(have ?? []).includes(o.value as "openai" | "gemini");
    }
  });
  const instantRow = h("div", { class: "row" }, h("label", { text: t("Instant voice") }), instant);
  const instantHint = h("div", { class: "hint", text: t("With an OpenAI or Gemini key, a real-time voice model answers in a conversation: faster and more natural. If it can't connect, Groovy's own voice takes over.") });
  const summaryRow = h("div", { class: "row" },
    h("label", { text: t("Save a summary of each conversation") }),
    toggle(settings.callSummary !== false, (v) => { settings.callSummary = v; save(); }),
  );

  const modelRow = h("div", { class: "row" }, h("label", { text: t("Speech model") }), model, download, remove, bar);
  const modelHint = h("div", {
    class: "hint",
    text: t("Speech is turned into text on this computer: no audio leaves it. The model is downloaded once from Hugging Face (whisper.cpp)."),
  });

  // ── Recognition: the one speech engine calls, dictation, notes and « OK Groovy » share ──
  const recognition = h("select", {},
    h("option", { value: "whisper", text: t("Whisper (light, 99 languages)") }),
    h("option", { value: "parakeet", text: t(PARAKEET_LABEL) }),
  ) as HTMLSelectElement;
  let flowPrefs: FlowPrefs | null = null;
  let parakeetReady = false;
  recognition.addEventListener("change", () => {
    if (!flowPrefs) return;
    flowPrefs = { ...flowPrefs, engine: recognition.value === "whisper" ? "whisper" : "parakeet" };
    void FlowBridge.setPrefs(flowPrefs).then(() => refresh());
  });
  const setupSpeech = h("button", { text: t("Set up speech"), onclick: () => void Bridge.openSettingsWindow("speechsetup") });
  const parakeetState = h("span", { class: "hint" });
  const recognitionRow = h("div", { class: "row" }, h("label", { text: t("Recognition") }), recognition, parakeetState, setupSpeech);
  const recognitionHint = h("div", { class: "hint", text: t("Downloaded once and used everywhere: calls, dictation, the Notetaker and « OK Groovy ».") });
  const parakeetWarning = h("div", { class: "notice warn", text: t(PARAKEET_OVER_LIMIT) });

  function draw() {
    const local = settings.voiceEngine !== "livekit";
    const whisper = flowPrefs?.engine === "whisper";
    const available = status?.available ?? true;
    const installed = status?.installed.includes(model.value) ?? false;
    // Whisper's own models only matter when Whisper is the engine.
    modelRow.style.display = whisper ? "" : "none";
    modelHint.style.display = local && whisper ? "" : "none";
    parakeetState.textContent = whisper ? "" : parakeetReady ? t("Ready") : t("Not downloaded yet");
    setupSpeech.style.display = !whisper && !parakeetReady ? "" : "none";
    parakeetWarning.style.display = flowPrefs && !whisper ? "" : "none";
    livekitRows.style.display = local ? "none" : "";
    speakRows.style.display = local ? "" : "none";
    const multilingual = !model.value.endsWith(".en") && !model.value.includes(".en-");
    languageRow.style.display = local && whisper && multilingual ? "" : "none";
    languageHint.style.display = local && whisper ? "" : "none";
    interruptRow.style.display = local ? "" : "none";
    download.style.display = installed || !available ? "none" : "";
    download.disabled = downloading != null;
    download.textContent = downloading ? t("Downloading…") : t("Download");
    remove.style.display = installed && !downloading ? "" : "none";
    if (!downloading) bar.style.display = "none";
    const ready = status?.ready ?? (available && installed);
    dot.style.background = ready ? "#22c55e" : "#f4505e";
    state.textContent = !available
      ? t("This build of Groovy has no speech engine.")
      : ready
        ? t("Say “Groovy”, “Hey Groovy” or “Yo Groovy”, then what you want.")
        : t("Set up the speech model to use your voice.");
  }

  async function refresh() {
    status = await Bridge.voiceStatus();
    const flow = await FlowBridge.status();
    if (flow) {
      flowPrefs = flow.prefs;
      parakeetReady = flow.models.parakeet.state === "ready";
      recognition.value = flow.prefs.engine;
    }
    draw();
  }
  void refresh();

  // Round 3: what the island shows while Groovy listens (dictation, a call, a recording).
  const listening = h("select", {}) as HTMLSelectElement;
  for (const [value, label] of [["badge", t("Badge")], ["strip", t("Live strip")]] as const) {
    listening.append(h("option", { value, text: label }));
  }
  listening.value = settings.listeningView === "strip" ? "strip" : "badge";
  listening.addEventListener("change", () => {
    settings.listeningView = listening.value === "strip" ? "strip" : "badge";
    save();
  });
  const listeningRow = h("div", { class: "row" }, h("label", { text: t("While listening, show") }), listening);
  const listeningHint = h("div", { class: "hint", text: t("Badge keeps the island as it is with a small timer; Live strip shrinks it to Groovy, your voice and Stop.") });

  // Round 3B item 10: speech models and the local AI load when needed and go after this long unused.
  const unload = h("select", {}) as HTMLSelectElement;
  for (const [value, label] of [["30s", t("30 seconds")], ["2m", t("2 minutes")], ["5m", t("5 minutes")], ["keep", t("Keep loaded")]] as const) {
    unload.append(h("option", { value, text: label }));
  }
  unload.value = ["30s", "5m", "keep"].includes(settings.unloadAfter) ? settings.unloadAfter : "2m";
  unload.addEventListener("change", () => {
    settings.unloadAfter = unload.value as typeof settings.unloadAfter;
    save();
  });
  const unloadRow = h("div", { class: "row" }, h("label", { text: t("Free memory after") }), unload);
  const unloadHint = h("div", { class: "hint", text: t("Speech models and the local AI load when you use them and give their memory back once unused for this long.") });

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
    recognitionRow,
    recognitionHint,
    parakeetWarning,
    speakRows,
    interruptRow,
    instantRow,
    instantHint,
    summaryRow,
    micRow,
    modelRow,
    languageRow,
    languageHint,
    modelHint,
    livekitRows,
    listeningRow,
    listeningHint,
    unloadRow,
    unloadHint,
    h("div", { class: "subhead", text: t("Check") }),
    voiceTestBlock(settings),
    feedback,
  );
}
