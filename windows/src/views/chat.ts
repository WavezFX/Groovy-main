// Chat view — DOM port of PromptView / ChatBubble / TypingDotsView /
// ModelPickerView from IslandViewContent.swift.
//
// Answers are rendered as Markdown (markdown.ts). A local model streams its
// answer: Rust sends `chat-delta` events with the text visible so far. The
// model name above the text field opens the picker: provider chips, then the
// models of the chosen provider, asked for only once it is picked.

import { FIX_LABELS, FIX_PAGES, stageText } from "../voice/stages";
import { h, svg, clear } from "./dom";
import { ICONS } from "./icons";
import { renderMarkdown } from "./markdown";
import { Bridge, onEvent, type ChatContext, type ModelInfo } from "../core/bridge";
import {
  activeModel, pickModel, providerDef, visibleProviders, withModel, type ProviderDef,
} from "../core/providers";
import { Sound } from "../core/sound";
import { CallControl, MicLevel } from "../voice/call";
import type { CallState } from "../voice/call-state";
import { brainReady, offerBrain } from "../island/brain";
import { State, type ChatMessage } from "../core/state";
import type { ViewHost } from "./views";
import { N_, t, tl } from "../i18n/i18n";

const STRINGS = {
  placeholderFirst: N_("Ask me anything…"),
  placeholderNext: N_("Continue…"),
  send: N_("Send"),
  switchModel: N_("Switch provider or model"),
  noModel: N_("Choose a model"),
  loading: N_("Loading models…"),
  noKey: N_("No API key — add it in Settings."),
  openSettings: N_("Open Settings"),
  listening: N_("Listening…"),
  speaking: N_("Speaking…"),
  call: N_("Talk to Groovy"),
  hangUp: N_("End the call"),
  dictate: N_("Dictate a message"),
  stopDictating: N_("Stop dictating"),
  mute: N_("Mute the microphone"),
  unmute: N_("Unmute the microphone"),
  interrupt: N_("Stop talking"),
  thinking: N_("Thinking…"),
  muted: N_("Muted"),
  callError: N_("Something went wrong"),
  allowOnce: N_("Allow once"),
  allowAlways: N_("Always allow"),
  deny: N_("Don't"),
};

/** What the call bar says in each state of the call. */
export function callLabel(state: CallState): string {
  switch (state) {
    case "listening": return STRINGS.listening;
    case "processing": return STRINGS.thinking;
    case "speaking": return STRINGS.speaking;
    case "muted": return STRINGS.muted;
    case "error": return STRINGS.callError;
    default: return "";
  }
}

let nextId = 1;

function bubble(message: ChatMessage, onChange: () => void): HTMLElement {
  if (message.role === "user") {
    return h(
      "div",
      { class: "chat-row user" },
      h("div", { class: "bubble", text: message.content }),
    );
  }
  const reply = h("div", { class: "reply" });
  renderMarkdown(reply, message.content);
  const action = message.action;
  if (action) {
    // A desktop tool asks first (tools.rs): nothing runs until a button is clicked.
    const decide = async (decision: "once" | "always" | "deny") => {
      message.action = undefined;
      try {
        const result = await Bridge.toolsDecide(action.id, decision);
        State.chatHistory.push({ id: nextId++, role: "assistant", content: result });
      } catch (err) {
        State.chatHistory.push({ id: nextId++, role: "assistant", content: String(err).replace(/^Error:\s*/, "") });
      }
      onChange();
    };
    const row = h(
      "div",
      { class: "tool-ask" },
      h("button", { class: "tool-btn primary", text: tl(STRINGS.allowOnce), onclick: () => void decide("once") }),
      h("button", { class: "tool-btn", text: tl(STRINGS.allowAlways), onclick: () => void decide("always") }),
      h("button", { class: "tool-btn", text: tl(STRINGS.deny), onclick: () => void decide("deny") }),
    );
    reply.append(row);
  }
  return h("div", { class: "chat-row" }, reply);
}

function typingDots(): HTMLElement {
  return h(
    "div",
    { class: "chat-row" },
    h("div", { class: "typing" }, h("i"), h("i"), h("i")),
  );
}

/** The coloured chip showing what the question is about (a dropped file). */
function contextChip(label: string): HTMLElement {
  const chip = h("div", { class: "chip" }, h("i", { class: "chip-dot" }), h("span", { text: label }));
  requestAnimationFrame(() => chip.classList.add("settled"));
  return chip;
}

function saveSettings() {
  void Bridge.saveSettings(State.settings);
}

// ── Model picker ──────────────────────────────────────────────────────────────

interface Picker {
  el: HTMLElement;
  open(): void;
  close(): void;
  readonly isOpen: boolean;
}

/** Provider chips, then the chosen provider's models — ModelPickerView. */
function buildPicker(onChange: () => void): Picker {
  const chips = h("div", { class: "picker-chips" });
  const list = h("div", { class: "picker-list" });
  const el = h("div", { class: "picker" }, chips, h("div", { class: "picker-rule" }), list);

  /** Models already asked for, by provider; a model server is asked again each time. */
  const cache = new Map<string, ModelInfo[]>();
  let isOpen = false;
  let request = 0;

  function drawChips() {
    clear(chips);
    for (const p of visibleProviders(State.settings)) {
      const on = p.id === State.settings.chatProvider;
      const chip = h(
        "button",
        { class: on ? "picker-chip on" : "picker-chip", style: `--accent:${p.accent}` },
        h("i", { class: "picker-dot" }),
        h("span", { text: t(p.name) }),
      );
      chip.addEventListener("click", () => {
        if (p.id === State.settings.chatProvider) return;
        State.settings = { ...State.settings, chatProvider: p.id };
        saveSettings();
        Sound.play("pop");
        drawChips();
        void loadModels();
        onChange();
      });
      chips.append(chip);
    }
  }

  function status(text: string, withSettings = false) {
    clear(list);
    const line = h("div", { class: "picker-status", text });
    if (withSettings) {
      line.append(
        h("button", {
          class: "picker-link",
          text: tl(STRINGS.openSettings),
          onclick: () => void Bridge.openSettingsWindow(),
        }),
      );
    }
    list.append(line);
  }

  function drawModels(p: ProviderDef, models: ModelInfo[]) {
    clear(list);
    const current = activeModel(State.settings);
    for (const m of models) {
      const on = m.id === current;
      const row = h(
        "button",
        { class: on ? "picker-model on" : "picker-model", style: `--accent:${p.accent}`, title: m.id },
        h("span", { class: "picker-model-name", text: m.label }),
        on ? svg(ICONS.check, 11, { stroke: 2.2 }) : null,
      );
      row.addEventListener("click", () => {
        State.settings = withModel(State.settings, p.id, m.id);
        saveSettings();
        Sound.play("blip");
        close();
      });
      list.append(row);
    }
    list.querySelector(".picker-model.on")?.scrollIntoView({ block: "nearest" });
  }

  async function loadModels() {
    const p = providerDef(State.settings.chatProvider);
    const ticket = ++request;
    // Model servers and the models folder can change any time: asked afresh.
    const live = p.urlField !== null || p.id === "llamacpp";
    const cached = live ? undefined : cache.get(p.id);
    if (cached) {
      drawModels(p, cached);
      return;
    }
    // Nothing is asked of a provider that has no key yet.
    if (p.key && !(await Bridge.secretPresent(p.key))) {
      if (ticket === request) status(t(STRINGS.noKey), true);
      return;
    }
    if (ticket !== request) return;
    status(t(STRINGS.loading));
    try {
      const models = await Bridge.chatModels(p.id);
      if (ticket !== request) return;
      if (!live) cache.set(p.id, models);
      const keep = pickModel(p, models.map((m) => m.id), activeModel(State.settings));
      if (keep && keep !== activeModel(State.settings)) {
        State.settings = withModel(State.settings, p.id, keep);
        saveSettings();
        onChange();
      }
      drawModels(p, models);
    } catch (err) {
      if (ticket === request) status(String(err).replace(/^Error:\s*/, ""), Boolean(p.key));
    }
  }

  function open() {
    isOpen = true;
    el.classList.add("on");
    drawChips();
    onChange();
    void loadModels();
  }

  function close() {
    isOpen = false;
    request++;
    el.classList.remove("on");
    onChange();
  }

  return {
    el,
    open,
    close,
    get isOpen() {
      return isOpen;
    },
  };
}

// ── View ──────────────────────────────────────────────────────────────────────

export function buildPrompt(onHeightChange: () => void): ViewHost {
  const chipRow = h("div", { class: "chip-row" });
  const log = h("div", { class: "chat-log" });
  const input = h("input", {
    type: "text",
    class: "chat-input",
    placeholder: t(STRINGS.placeholderFirst),
    spellcheck: "false",
  }) as HTMLInputElement;
  const send = h("button", { class: "send-btn", title: tl(STRINGS.send) }, svg(ICONS.arrowUp, 11));
  const callBtn = h("button", { class: "call-btn", title: tl(STRINGS.call) }, svg(ICONS.phone, 12));
  callBtn.addEventListener("click", () => CallControl.toggle());
  const dictateBtn = h("button", { class: "call-btn", title: tl(STRINGS.dictate) }, svg(ICONS.mic, 12));
  dictateBtn.addEventListener("click", () => CallControl.dictate());
  const bar = h("div", { class: "chat-bar" }, input, dictateBtn, callBtn, send);

  // ── The call bar: what the call is doing, the level, mute / interrupt / end ──
  const viz = h("div", { class: "call-viz", "aria-hidden": "true" }, h("i"), h("i"), h("i"), h("i"), h("i"));
  const callText = h("span", { class: "call-state" });
  const muteBtn = h("button", { class: "call-btn small", title: tl(STRINGS.mute) }, svg(ICONS.micOff, 11));
  muteBtn.addEventListener("click", () => CallControl.mute());
  const interruptBtn = h("button", { class: "call-chip", text: tl(STRINGS.interrupt) });
  interruptBtn.addEventListener("click", () => CallControl.interrupt());
  const endBtn = h("button", { class: "call-btn small end", title: tl(STRINGS.hangUp) }, svg(ICONS.xmark, 10));
  endBtn.addEventListener("click", () => CallControl.toggle());
  // A failed stage's one fix (Download speech model, Choose an AI, Pick a voice).
  const fixBtn = h("button", { class: "call-chip", onclick: () => {
    if (State.callFix) void Bridge.openSettingsWindow(FIX_PAGES[State.callFix]);
  } });
  const callBar = h("div", { class: "call-bar", role: "status" }, viz, callText, h("div", { class: "grow" }), fixBtn, interruptBtn, muteBtn, endBtn);
  const bars = Array.from(viz.children) as HTMLElement[];
  MicLevel.listeners.add((level) => {
    if (!callBar.isConnected || callBar.style.display === "none") return;
    bars.forEach((b, i) => {
      const shape = [0.55, 0.8, 1, 0.8, 0.55][i];
      b.style.transform = `scaleY(${(0.18 + level * shape * 0.82).toFixed(3)})`;
    });
  });

  const modelDot = h("i", { class: "model-dot" });
  const modelName = h("span", { class: "model-name" });
  const modelBtn = h(
    "button",
    { class: "model-btn", title: tl(STRINGS.switchModel) },
    modelDot,
    modelName,
    svg(ICONS.chevronUpDown, 9, { stroke: 2 }),
  );
  const modelRow = h("div", { class: "model-row" }, modelBtn);

  const body = h("div", { class: "chat-body" });
  const picker = buildPicker(() => {
    body.classList.toggle("picking", picker.isOpen);
    drawModelButton();
  });
  body.append(chipRow, log, picker.el, modelRow, callBar, bar);

  const el = h("div", { class: "view" }, h("div", { class: "card wash chat-card" }, body));
  (el.querySelector(".card") as HTMLElement).style.setProperty("--wash", "rgba(99,102,241,0.5)");

  let sending = false;
  let renderedCount = -1;
  // A local model answers token by token: where its text so far is shown.
  let live: HTMLElement | null = null;

  function drawModelButton() {
    const p = providerDef(State.settings.chatProvider);
    modelDot.style.background = p.accent;
    modelName.textContent = activeModel(State.settings) || t(STRINGS.noModel);
    modelBtn.classList.toggle("open", picker.isOpen);
    modelBtn.disabled = sending;
  }

  modelBtn.addEventListener("click", () => {
    if (sending) return;
    if (picker.isOpen) picker.close();
    else picker.open();
  });

  void onEvent<string>("chat-delta", (text) => {
    if (!sending || !text) return; // nothing visible yet: the dots stay
    if (!live) {
      live = h("div", { class: "reply" });
      log.querySelector(".typing")?.parentElement?.remove();
      log.append(h("div", { class: "chat-row" }, live));
    }
    renderMarkdown(live, text);
    log.scrollTop = log.scrollHeight;
  });

  async function submit(spoken = false) {
    const query = input.value.trim();
    if (!query || sending) return;
    if (picker.isOpen) picker.close();
    // The offline brain first needs Ollama and a model: offer to set them up,
    // then ask this question.
    if (State.settings.chatProvider === "ollama" && !(await brainReady())) {
      offerBrain(() => {
        input.value = query;
        void submit();
      });
      return;
    }
    input.value = "";
    sending = true;
    drawModelButton();
    Sound.play("send");

    State.chatHistory.push({ id: nextId++, role: "user", content: query });
    State.stateOverride = "thinking";
    State.notify();
    onHeightChange();

    const file = State.droppedFile;
    const context: ChatContext | null =
      State.chatHistory.length === 1 && file ? { kind: "file", name: file.name, path: file.path } : null;

    try {
      const reply = await Bridge.chatSend(query, context, spoken);
      State.chatHistory.push({ id: nextId++, role: "assistant", content: reply.text, action: reply.action });
      State.stateOverride = null;
      Sound.play("finish");
    } catch (err) {
      State.stateOverride = null;
      State.noteMessage = String(err).replace(/^Error:\s*/, "");
      State.view = "note";
      Sound.play("error");
    } finally {
      sending = false;
      live = null;
      renderedCount = -1; // the finished answer replaces the streamed one
      drawModelButton();
      State.notify();
      onHeightChange();
      input.focus();
    }
  }

  send.addEventListener("click", () => void submit());
  input.addEventListener("keydown", (e) => {
    const key = (e as KeyboardEvent).key;
    if (key === "Enter") {
      e.preventDefault();
      void submit();
    } else if (key === "Escape" && picker.isOpen) {
      e.preventDefault();
      picker.close();
    }
    e.stopPropagation(); // Escape closes the island, not the chat
  });

  return {
    el,
    sync() {
      const file = State.droppedFile;
      const wantChip = file?.name ?? "";
      if (chipRow.dataset.label !== wantChip) {
        chipRow.dataset.label = wantChip;
        clear(chipRow);
        if (wantChip) chipRow.append(contextChip(wantChip));
      }

      const thinking = State.stateOverride === "thinking";
      const count = State.chatHistory.length + (thinking ? 0.5 : 0);
      if (count !== renderedCount && !live) {
        renderedCount = count;
        clear(log);
        const changed = () => {
          renderedCount = -1;
          State.notify();
          onHeightChange();
        };
        for (const m of State.chatHistory) log.append(bubble(m, changed));
        if (thinking) log.append(typingDots());
        log.scrollTop = log.scrollHeight;
      }

      // Leaving the chat folds the picker away.
      if (State.view !== "prompt" && picker.isOpen) picker.close();
      drawModelButton();

      // Voice: "Groovy" heard — the field says he's listening; a spoken command
      // is asked exactly like a typed one.
      const listening = State.voiceListening;
      bar.classList.toggle("listening", listening);
      bar.classList.toggle("speaking", State.voiceSpeaking);
      callBtn.classList.toggle("on", State.voiceCall);
      callBtn.title = t(State.voiceCall ? STRINGS.hangUp : STRINGS.call);
      dictateBtn.classList.toggle("on", State.dictating);
      dictateBtn.title = t(State.dictating ? STRINGS.stopDictating : STRINGS.dictate);
      dictateBtn.style.display = State.voiceCall ? "none" : "";
      callBar.style.display = State.voiceCall ? "" : "none";
      callBar.dataset.state = State.callState;
      callText.textContent = State.voiceCall
        ? State.voiceCaption && State.callState === "listening" ? State.voiceCaption : stageText(State.callState, State.callHeard, State.callBrain, State.callReason)
        : "";
      const fix = State.callState === "error" ? State.callFix : null;
      fixBtn.style.display = fix ? "" : "none";
      if (fix) fixBtn.textContent = t(FIX_LABELS[fix]);
      muteBtn.classList.toggle("on", State.callState === "muted");
      muteBtn.title = t(State.callState === "muted" ? STRINGS.unmute : STRINGS.mute);
      interruptBtn.style.display = State.voiceSpeaking ? "" : "none";
      if (State.dictation !== null && !sending) {
        input.value = State.dictation;
        State.dictation = null;
        input.focus();
      }
      input.placeholder = State.voiceSpeaking
        ? t(STRINGS.speaking)
        : listening
        ? State.voiceCaption ?? t(STRINGS.listening)
        : State.voiceProblem ?? t(State.chatHistory.length === 0 ? STRINGS.placeholderFirst : STRINGS.placeholderNext);
      input.disabled = sending;
      if (State.voiceQuery && !sending && State.view === "prompt") {
        input.value = State.voiceQuery;
        State.voiceQuery = null;
        void submit(true);
      }
    },
    focus() {
      input.focus();
      input.select();
    },
  };
}
