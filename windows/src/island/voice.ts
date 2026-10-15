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
import { CallControl, MicLevel } from "../voice/call";
import { BargeIn, LevelMeter, nextCallState, type CallEvent } from "../voice/call-state";
import { power } from "../voice/vad";
import { openMic, type Mic } from "../voice/mic";
import { startLiveKit, type LiveKitSession } from "../voice/livekit";
import { speak, stopSpeaking } from "../voice/speak";
import { SentenceSpeaker } from "../voice/sentences";
import { brainName, type CallFix } from "../voice/stages";
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
  private bargeIn = new BargeIn();
  private meter = new LevelMeter();
  private lastLevel = 0;
  /** Reads the answer sentence by sentence as it streams in. */
  private speaker: SentenceSpeaker | null = null;
  /** The last transcription failed (not just silence): why. */
  private transcribeError: string | null = null;
  /** When the current turn was asked, for the log. */
  private askedAt = 0;

  private engine: BotEngine;
  private actions: VoiceHostActions;

  constructor(engine: BotEngine, actions: VoiceHostActions) {
    this.engine = engine;
    this.actions = actions;
    this.controller = new VoiceController(
      async (pcm, purpose) => {
        try {
          const text = await Bridge.voiceTranscribe(toInt16(pcm), purpose === "wake");
          if (purpose === "command") this.transcribeError = null;
          return text;
        } catch (e) {
          if (purpose === "command") {
            this.transcribeError = String(e).replace(/^Error:\s*/, "");
            log(`transcription failed: ${this.transcribeError}`);
          }
          throw e;
        }
      },
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
    CallControl.mute = () => this.toggleMute();
    CallControl.interrupt = () => this.interrupt();
    CallControl.dictate = () => void this.dictate();
    // The answer streams in: each finished sentence is read out at once.
    void onEvent<string>("chat-delta", (text) => {
      if (this.awaiting != null && this.speaker && text) this.speaker.feed(text);
    });
  }

  /** A stage failed: the call stays up showing why, with the button that fixes it. */
  private failCall(reason: string, fix: CallFix | null) {
    State.callReason = reason;
    State.callFix = fix;
    log(`call failed (${fix ?? "no fix"}): ${reason}`);
    this.call("fail");
    State.voiceListening = false;
    State.voiceSpeaking = false;
    State.notify();
  }

  /** Called whenever the state changes: the microphone, and answers to read out. */
  sync() {
    const want = (!!State.settings.voiceEnabled || State.voiceCall || State.dictating) && !State.paused;
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
          // One speech model for calls, dictation, notes and « OK Groovy »:
          // set up once in Voice and dictation, not a second download here.
          State.voiceProblem = t("Calls need the speech model. It's downloaded once and used for dictation too.");
          State.callFix = "speech";
          log("speech model missing");
          this.gaveUp = true;
          return false;
        }
        this.mic = await openMic((frame) => this.frame(frame), 1024, State.settings.voiceMic ?? "", () => this.micLost());
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

  /** Every microphone frame: the level meter, barge-in while he speaks, else the controller. */
  private frame(frame: Float32Array) {
    const p = power(frame);
    const level = this.meter.feed(p);
    if (Math.abs(level - this.lastLevel) > 0.02) {
      this.lastLevel = level;
      MicLevel.set(State.callState === "muted" ? 0 : level);
    }
    if (State.callState === "muted") return;
    if (State.voiceSpeaking) {
      if (State.voiceCall && State.settings.voiceInterrupt && this.bargeIn.feed(p, this.controller.noiseFloor)) this.interrupt();
      return;
    }
    this.controller.feed(frame);
  }

  private call(event: CallEvent) {
    const next = nextCallState(State.callState, event);
    if (next === State.callState) return;
    State.callState = next;
    State.notify();
  }

  /** The microphone went away (unplugged, disabled): say so, and stop using it. */
  private micLost() {
    State.voiceProblem = t("The microphone was disconnected. Plug it back in, or pick another one in Settings → Voice.");
    this.call("fail");
    const inCall = State.voiceCall;
    this.mic?.stop();
    this.mic = null;
    this.controller.stop();
    this.gaveUp = true;
    State.voiceListening = false;
    State.dictating = false;
    if (inCall) {
      State.voiceCall = false;
      Sound.play("error");
    }
    State.notify();
  }

  private toggleMute() {
    if (!State.voiceCall) return;
    if (State.callState === "muted") {
      this.call("unmute");
      this.listenForTurn();
    } else {
      this.call("mute");
      this.controller.pause();
      State.voiceListening = false;
      MicLevel.set(0);
      State.notify();
    }
  }

  /** Stop talking and listen: the call button while he speaks, or speaking over him. */
  private interrupt() {
    if (!State.voiceSpeaking) return;
    this.speaker?.stop();
    stopSpeaking();
    State.voiceSpeaking = false;
    this.bargeIn.reset();
    this.call("interrupt");
    if (State.voiceCall) this.listenForTurn();
    else this.controller.resume();
  }

  /** Mode A: listen for one message and put it in the text field, unsent. */
  private async dictate() {
    if (State.dictating) {
      State.dictating = false;
      this.controller.resume();
      State.notify();
      return;
    }
    if (State.voiceCall) return;
    this.gaveUp = false;
    State.dictating = true;
    State.voiceProblem = null;
    State.notify(); // opens the microphone through sync()
    if (!(await this.open())) {
      State.dictating = false;
      State.notify();
      return;
    }
    Sound.play("peek");
    this.controller.resume();
    this.controller.listen(8);
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
    State.dictating = false;
    this.call("end");
    MicLevel.set(0);
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
    State.dictating = false;
    State.callReason = null;
    State.callFix = null;
    State.callHeard = null;
    State.callBrain = brainName(State.settings);
    this.call("start");
    log(`call started (brain ${State.callBrain}, speech ${State.settings.voiceSpeak ? "on" : "off"})`);
    State.notify(); // opens the microphone through sync()
    if (!(await this.open())) {
      // The call bar stays, saying why and how to fix it.
      this.actions.openChat();
      this.failCall(State.voiceProblem ?? t("Groovy can't use the microphone."), State.callFix);
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
    this.speaker?.stop();
    this.speaker = null;
    stopSpeaking();
    this.controller.resume();
    this.call("end");
    MicLevel.set(0);
    if (was) {
      Sound.play("close");
      log("call ended");
    }
    State.notify();
  }

  private listenForTurn() {
    State.callReason = null;
    State.callFix = null;
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
    if (State.settings.voiceCall) {
      State.voiceCall = true;
      this.call("start");
    }
    State.voiceListening = true;
    this.actions.openChat();
    State.notify();
  }

  private command(text: string) {
    if (State.dictating) {
      // Dictation: the words go in the text field, to edit and send by hand.
      State.dictating = false;
      State.dictation = text;
      this.controller.resume();
      this.actions.openChat();
      State.notify();
      return;
    }
    if (State.settings.voiceEngine === "livekit") {
      // "Hey Groovy, …" in one breath: the conversation starts and the agent
      // hears the rest from you live.
      void this.startConversation();
      return;
    }
    if (!State.voiceCall && !State.voiceListening && State.settings.voiceCall) {
      State.voiceCall = true;
      this.call("start");
    }
    if (State.voiceCall) this.call("heard");
    State.voiceListening = false;
    // Not listening while he thinks and answers: he'd hear himself.
    this.controller.pause();
    this.awaiting = State.chatHistory.length;
    State.callHeard = text;
    State.callBrain = brainName(State.settings);
    this.askedAt = performance.now();
    log(`heard ${text.length} chars; asking ${State.callBrain}`);
    this.speaker = State.settings.voiceSpeak ? this.newSpeaker() : null;
    State.voiceQuery = text;
    this.actions.openChat();
    State.notify();
  }

  private cancelled() {
    if (State.dictating) {
      State.dictating = false;
      State.notify();
      return;
    }
    if (State.voiceCall) {
      // Not silence: the speech engine failed. Say why instead of hanging up.
      if (this.transcribeError) {
        const why = this.transcribeError;
        this.transcribeError = null;
        this.failCall(why, "speech");
        return;
      }
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

  private newSpeaker(): SentenceSpeaker {
    return new SentenceSpeaker(
      (sentence) => speak(sentence, State.settings.voiceTts, 1, true),
      () => {
        // The first sentence is ready: he starts talking before the answer is done.
        log(`first sentence after ${((performance.now() - this.askedAt) / 1000).toFixed(1)}s`);
        this.call("answered");
        State.voiceSpeaking = true;
        this.bargeIn.reset();
        this.engine.triggerEmote("happy", 0.8);
        State.notify();
      },
    );
  }

  /** The answer to a spoken question arrived: read it out, then listen again in a call. */
  private watchAnswer() {
    if (this.awaiting == null) return;
    const answer = State.chatHistory.slice(this.awaiting).find((m) => m.role === "assistant");
    const failed = !answer && State.view === "note" && State.stateOverride !== "thinking";
    if (!answer && !failed) return;
    this.awaiting = null;
    if (failed) {
      this.speaker?.stop();
      this.speaker = null;
      if (State.voiceCall) {
        // The brain didn't answer: the chat shows the call bar with why.
        const why = State.noteMessage ?? t("The AI didn't answer.");
        State.view = "prompt";
        this.failCall(why, "ai");
      } else this.controller.resume();
      return;
    }
    log(`answered in ${((performance.now() - this.askedAt) / 1000).toFixed(1)}s (${answer!.content.length} chars)`);
    void this.say(answer!.content);
  }

  private async say(text: string) {
    const speaker = this.speaker;
    if (speaker) {
      // Kept until it's done, so interrupting stops what's still queued.
      await speaker.finish(text);
      if (this.speaker === speaker) this.speaker = null;
      if (speaker.failure) {
        if (State.voiceCall) this.failCall(speaker.failure, "voice");
        else this.controller.resume();
        return;
      }
      if (!State.voiceSpeaking) return; // interrupted: already listening again
      State.voiceSpeaking = false;
      log("spoken");
      this.call("spoke");
    } else {
      this.call("silent");
    }
    if (State.voiceCall && State.callState === "muted") {
      State.notify();
      return;
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
    this.call("start");
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
        this.call(error ? "fail" : "end");
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

/** One line in voice.log: stages and timings, never what was said. */
function log(line: string) {
  void Bridge.voiceLog(line);
}
