// Voice in the island: opens the microphone while Settings → Voice is on (or a
// call is going), and turns what the VoiceController hears into island actions.
//
//   one question   "Hey Groovy" opens the chat and listens; what you say is
//                  asked like a typed question, and the answer is read aloud.
//   voice call     the same, then he keeps listening after each answer, like a
//                  phone call, until you say "bye" or go quiet. Started by the
//                  wake word (Settings → Voice) or the phone button in the chat.
//   LiveKit        the wake word joins your own LiveKit room instead.

import { Bridge, onEvent } from "../core/bridge";
import { Sound } from "../core/sound";
import { State } from "../core/state";
import type { BotEngine } from "../mochi/engine";
import { VoiceController, toInt16, type VoicePhase } from "../voice/controller";
import { CallControl } from "../voice/call";
import { openMic, type Mic } from "../voice/mic";
import { startLiveKit, type LiveKitSession } from "../voice/livekit";
import { speak, stopSpeaking } from "../voice/speak";
import { t } from "../i18n/i18n";

/** In a call, how long he waits for you to start speaking before hanging up. */
const CALL_TURN_WAIT = 12;

export interface VoiceHostActions {
  /** Opens the island on the chat. */
  openChat(): void;
}

export class VoiceHost {
  private controller: VoiceController;
  private mic: Mic | null = null;
  private opening: Promise<boolean> | null = null;
  private live: LiveKitSession | null = null;
  /** Whether the microphone is meant to be open. */
  private wanted = false;
  /** Opening failed (no model, no microphone): no retrying on every change. */
  private gaveUp = false;
  /** A spoken question is waiting for its answer: the chat's length when asked. */
  private awaiting: number | null = null;

  private engine: BotEngine;
  private actions: VoiceHostActions;

  constructor(engine: BotEngine, actions: VoiceHostActions) {
    this.engine = engine;
    this.actions = actions;
    this.controller = new VoiceController(
      (pcm, purpose) => Bridge.voiceTranscribe(toInt16(pcm), purpose === "wake"),
      {
        onWake: () => this.heard(),
        onCommand: (text) => this.command(text),
        onCancel: () => this.cancelled(),
        onPhase: (p) => this.phase(p),
      },
    );
    CallControl.toggle = () => {
      if (State.voiceCall || this.live) this.endCall();
      else void this.startCall();
    };
  }

  /** Called whenever the state changes: the microphone, and answers to read out. */
  sync() {
    const want = (!!State.settings.voiceEnabled || State.voiceCall) && !State.paused;
    if (want !== this.wanted) {
      this.wanted = want;
      // Turned off and on again: try again after a failure.
      if (!want) this.gaveUp = false;
      if (want && !this.gaveUp) void this.open();
      else if (!want) this.close();
    }
    this.watchAnswer();
  }

  /** Opens the microphone; false when it can't be (no model, no permission). */
  private open(): Promise<boolean> {
    if (this.mic) return Promise.resolve(true);
    if (this.opening) return this.opening;
    this.opening = (async () => {
      try {
        const status = await Bridge.voiceStatus();
        if (!status?.available) {
          State.voiceProblem = t("This build of Groovy has no speech engine.");
          this.gaveUp = true;
          return false;
        }
        if (!status.ready) {
          // First time: fetch the speech model now instead of sending you to Settings.
          State.voiceProblem = t("Getting my ears ready… {0}%", { 0: 0 });
          State.notify();
          const stop = await onEvent<{ model: string; done: number; total: number }>("voice-model-progress", (p) => {
            const pct = p.total > 0 ? Math.floor((p.done / p.total) * 100) : 0;
            State.voiceProblem = t("Getting my ears ready… {0}%", { 0: pct });
            State.notify();
          });
          try {
            await Bridge.voiceDownloadModel(State.settings.voiceModel || "base.en");
          } catch (e) {
            State.voiceProblem = String(e).replace(/^Error:\s*/, "");
            this.gaveUp = true;
            return false;
          } finally {
            stop?.();
          }
        }
        this.mic = await openMic((frame) => this.controller.feed(frame));
        State.voiceProblem = null;
        this.controller.start();
        return true;
      } catch (e) {
        State.voiceProblem = t("Groovy can't use the microphone: {0}", { 0: String(e).replace(/^Error:\s*/, "") });
        this.gaveUp = true;
        return false;
      } finally {
        this.opening = null;
        State.notify();
      }
    })();
    return this.opening;
  }

  private close() {
    this.live?.hangUp();
    this.live = null;
    stopSpeaking();
    this.controller.stop();
    this.mic?.stop();
    this.mic = null;
    this.awaiting = null;
    State.voiceListening = false;
    State.voiceCall = false;
    State.notify();
  }

  /** The island was closed by hand: stop listening, hang up a call. */
  dismiss() {
    if (State.voiceCall || this.live) {
      this.endCall();
      return;
    }
    if (State.voiceListening) {
      State.voiceListening = false;
      this.controller.resume();
      State.notify();
    }
  }

  // ── Calls ──────────────────────────────────────────────────────────────────

  private async startCall() {
    if (State.settings.voiceEngine === "livekit") {
      void this.startConversation();
      return;
    }
    this.gaveUp = false;
    State.voiceCall = true;
    State.notify(); // opens the microphone through sync()
    if (!(await this.open())) {
      State.voiceCall = false;
      this.actions.openChat();
      State.notify();
      return;
    }
    Sound.play("open");
    this.engine.triggerEmote("happy", 1.2);
    this.listenForTurn();
  }

  private endCall() {
    this.live?.hangUp();
    const was = State.voiceCall;
    State.voiceCall = false;
    State.voiceListening = false;
    State.voiceSpeaking = false;
    this.awaiting = null;
    stopSpeaking();
    this.controller.resume();
    if (was) Sound.play("close");
    State.notify();
  }

  private listenForTurn() {
    State.voiceListening = true;
    State.voiceSpeaking = false;
    this.actions.openChat();
    this.controller.resume();
    this.controller.listen(CALL_TURN_WAIT);
    State.notify();
  }

  // ── What the controller heard ─────────────────────────────────────────────

  private heard() {
    if (State.settings.voiceEngine === "livekit") {
      void this.startConversation();
      return;
    }
    Sound.play("peek");
    this.engine.triggerEmote("surprised", 0.9);
    if (State.settings.voiceCall) State.voiceCall = true;
    State.voiceListening = true;
    this.actions.openChat();
    State.notify();
  }

  private command(text: string) {
    if (State.settings.voiceEngine === "livekit") {
      // "Hey Groovy, …" in one breath: the conversation starts and the agent
      // hears the rest from you live.
      void this.startConversation();
      return;
    }
    if (!State.voiceCall && !State.voiceListening && State.settings.voiceCall) State.voiceCall = true;
    State.voiceListening = false;
    // Not listening while he thinks and answers: he'd hear himself.
    this.controller.pause();
    this.awaiting = State.chatHistory.length;
    State.voiceQuery = text;
    this.actions.openChat();
    State.notify();
  }

  private cancelled() {
    if (State.voiceCall) {
      this.endCall();
      return;
    }
    if (!State.voiceListening) return;
    State.voiceListening = false;
    Sound.play("close");
    State.notify();
  }

  private phase(p: VoicePhase) {
    State.voicePhase = p;
    State.notify();
  }

  /** The answer to a spoken question arrived: read it out, then listen again in a call. */
  private watchAnswer() {
    if (this.awaiting == null) return;
    const answer = State.chatHistory.slice(this.awaiting).find((m) => m.role === "assistant");
    const failed = !answer && State.view === "note" && State.stateOverride !== "thinking";
    if (!answer && !failed) return;
    this.awaiting = null;
    if (failed) {
      if (State.voiceCall) this.endCall();
      else this.controller.resume();
      return;
    }
    void this.say(answer!.content);
  }

  private async say(text: string) {
    if (State.settings.voiceSpeak) {
      State.voiceSpeaking = true;
      State.notify();
      this.engine.triggerEmote("happy", 0.8);
      await speak(text, State.settings.voiceTts);
      State.voiceSpeaking = false;
    }
    if (State.voiceCall) this.listenForTurn();
    else {
      this.controller.resume();
      State.notify();
    }
  }

  // ── LiveKit ───────────────────────────────────────────────────────────────

  private async startConversation() {
    if (this.live) return;
    this.controller.pause();
    Sound.play("open");
    this.engine.triggerEmote("happy", 1.2);
    State.voiceCall = true;
    State.voiceListening = true;
    State.voiceCaption = t("Connecting…");
    this.actions.openChat();
    State.notify();
    this.live = await startLiveKit({
      onLive: () => {
        State.voiceCaption = t("Listening…");
        State.notify();
      },
      onCaption: (who, text) => {
        State.voiceCaption = who === "you" ? `“${text}”` : text;
        State.notify();
      },
      onEnd: (error) => {
        this.live = null;
        State.voiceCall = false;
        State.voiceListening = false;
        State.voiceCaption = null;
        if (error) {
          State.noteMessage = error;
          State.view = "note";
          Sound.play("error");
        } else {
          Sound.play("close");
        }
        this.controller.resume();
        State.notify();
      },
    });
  }
}
