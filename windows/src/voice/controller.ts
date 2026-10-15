// Groovy's ears: listens for "Groovy" / "Hey Groovy" / "Yo Groovy", then for
// what you ask, and hands the words to the island.
//
// Everything here is pure logic fed 16 kHz mono frames, so it runs the same in
// the tests as with the microphone (./mic.ts). Speech is turned into text by
// whoever the controller is given — Whisper on this computer (Rust, voice.rs).
//
//   wake      the VAD cuts speech into segments; each short one is transcribed
//             and checked for the wake phrase. "Hey Groovy, what's on today?"
//             in one breath goes straight through as a command.
//   command   after a bare "Groovy": the next thing said, ended by a longer
//             pause, is the command. Nothing said within a few seconds, or
//             "never mind", goes back to waiting.
//   paused    a LiveKit conversation (or anything else) has the microphone.

import { EnergyVAD, power } from "./vad";
import { isEndPhrase, splitWake } from "./wake";

export const SAMPLE_RATE = 16000;

export type VoicePhase = "off" | "wake" | "command" | "transcribing" | "paused";

/** Turns speech into text. `wake` asks for a quick pass, only to spot the name. */
export type Transcriber = (pcm: Float32Array, purpose: "wake" | "command") => Promise<string>;

export interface VoiceCallbacks {
  /** "Groovy" heard on its own: he is listening for the command. */
  onWake(): void;
  /** The words to act on. */
  onCommand(text: string): void;
  /** Listening ended without a command (silence, "never mind"). */
  onCancel(): void;
  onPhase?(phase: VoicePhase): void;
}

/** Seconds of audio kept from just before speech starts, so the first word isn't clipped. */
const PRE_ROLL = 0.35;
/** A wake segment is only checked on its first seconds: the name comes first. */
const WAKE_CHECK = 3.0;
/** Shorter than this is a click or a cough. */
const MIN_SEGMENT = 0.3;
/** Pauses that end a segment: short while waiting for the name, longer for a command. */
const WAKE_SILENCE = 0.7;
const COMMAND_SILENCE = 1.6;
/** How long to wait for a command to start after a bare "Groovy". */
const COMMAND_WAIT = 6.0;
/** Safety limit for one command. */
const COMMAND_MAX = 15.0;

export class VoiceController {
  phase: VoicePhase = "off";
  private vad: EnergyVAD;
  private preRoll: Float32Array[] = [];
  private segment: Float32Array[] = [];
  private segmentSamples = 0;
  /** Seconds of audio fed since the command window opened (for COMMAND_WAIT). */
  private commandClock = 0;
  private commandStarted = false;
  /** How long this command window waits for speech to start. */
  private commandWait = COMMAND_WAIT;
  /** Bumped on every reset so a late transcription can't act on a stale state. */
  private epoch = 0;

  private transcribe: Transcriber;
  private cb: VoiceCallbacks;

  constructor(transcribe: Transcriber, cb: VoiceCallbacks, frameSize = 1024) {
    this.transcribe = transcribe;
    this.cb = cb;
    this.vad = new EnergyVAD(SAMPLE_RATE / frameSize, WAKE_SILENCE);
  }

  start() {
    this.setPhase("wake");
  }

  /** The room's noise floor as the VAD has learned it (mean-square power). */
  get noiseFloor(): number {
    return this.vad.noisePower;
  }

  stop() {
    this.epoch++;
    this.clearSegment();
    this.vad.reset();
    this.setPhase("off");
  }

  /** Someone else has the microphone (a LiveKit conversation): stop listening. */
  pause() {
    this.epoch++;
    this.clearSegment();
    this.vad.reset();
    this.setPhase("paused");
  }

  resume() {
    if (this.phase === "off") return;
    this.toWake();
  }

  /**
   * Listens for the next thing said without waiting for the wake word — the
   * next turn of a voice call. Nothing said within `waitSeconds` cancels.
   */
  listen(waitSeconds = COMMAND_WAIT) {
    if (this.phase === "off") return;
    this.clearSegment();
    this.toCommand(waitSeconds);
  }

  /** One frame of 16 kHz mono audio, -1…1. */
  feed(frame: Float32Array) {
    if (this.phase === "off" || this.phase === "paused") return;
    const seconds = frame.length / SAMPLE_RATE;

    if (this.phase === "command" && !this.commandStarted) {
      this.commandClock += seconds;
      if (this.commandClock > this.commandWait) {
        this.toWake();
        this.cb.onCancel();
        return;
      }
    }

    const ev = this.vad.feed(power(frame));
    if (this.vad.isActive || ev === "end") {
      if (ev === "start") {
        this.segment = [...this.preRoll];
        this.segmentSamples = this.segment.reduce((n, f) => n + f.length, 0);
        if (this.phase === "command") this.commandStarted = true;
      }
      this.segment.push(frame);
      this.segmentSamples += frame.length;
      if (this.phase === "command" && this.segmentSamples / SAMPLE_RATE > COMMAND_MAX) {
        this.vad.reset();
        this.finishSegment();
        return;
      }
    }
    if (ev === "end") {
      this.finishSegment();
    }

    // Pre-roll ring: the last PRE_ROLL seconds before any speech.
    this.preRoll.push(frame);
    const keep = Math.ceil((PRE_ROLL * SAMPLE_RATE) / frame.length);
    while (this.preRoll.length > keep) this.preRoll.shift();
  }

  private finishSegment() {
    const pcm = concat(this.segment);
    this.clearSegment();
    if (pcm.length / SAMPLE_RATE < MIN_SEGMENT) return;
    if (this.phase === "wake") void this.checkWake(pcm);
    else if (this.phase === "command") void this.takeCommand(pcm);
  }

  private async checkWake(pcm: Float32Array) {
    const epoch = this.epoch;
    const head = pcm.length > WAKE_CHECK * SAMPLE_RATE ? pcm.subarray(0, WAKE_CHECK * SAMPLE_RATE) : pcm;
    this.setPhase("transcribing");
    let text = "";
    try {
      text = await this.transcribe(head, "wake");
    } catch {
      text = "";
    }
    if (epoch !== this.epoch) return;
    let split = splitWake(text);
    if (!split.matched) {
      this.setPhase("wake");
      return;
    }
    // Said in one breath: transcribe the whole thing properly for the command.
    if (head !== pcm || split.command) {
      try {
        const full = await this.transcribe(pcm, "command");
        if (epoch !== this.epoch) return;
        const again = splitWake(full);
        if (again.matched) split = again;
      } catch {
        // keep what the quick pass heard
      }
    }
    if (split.command) {
      this.toWake();
      this.deliver(split.command);
    } else {
      this.toCommand();
      this.cb.onWake();
    }
  }

  private async takeCommand(pcm: Float32Array) {
    const epoch = this.epoch;
    this.setPhase("transcribing");
    let text = "";
    try {
      text = (await this.transcribe(pcm, "command")).trim();
    } catch {
      text = "";
    }
    if (epoch !== this.epoch) return;
    this.toWake();
    // He may hear his name again ("Groovy… Groovy, what time is it?").
    const split = splitWake(text);
    const command = split.matched ? split.command : text;
    if (!command || isEndPhrase(command)) {
      this.cb.onCancel();
      return;
    }
    this.deliver(command);
  }

  private deliver(text: string) {
    if (isEndPhrase(text)) {
      this.cb.onCancel();
      return;
    }
    this.cb.onCommand(text);
  }

  private toWake() {
    this.epoch++;
    this.vad.reset();
    this.vad.setSilence(WAKE_SILENCE);
    this.setPhase("wake");
  }

  private toCommand(waitSeconds = COMMAND_WAIT) {
    this.commandWait = waitSeconds;
    this.epoch++;
    this.vad.reset();
    this.vad.setSilence(COMMAND_SILENCE);
    this.commandClock = 0;
    this.commandStarted = false;
    this.setPhase("command");
  }

  private clearSegment() {
    this.segment = [];
    this.segmentSamples = 0;
  }

  private setPhase(p: VoicePhase) {
    if (this.phase === p) return;
    this.phase = p;
    this.cb.onPhase?.(p);
  }
}

function concat(frames: Float32Array[]): Float32Array {
  let n = 0;
  for (const f of frames) n += f.length;
  const out = new Float32Array(n);
  let i = 0;
  for (const f of frames) {
    out.set(f, i);
    i += f.length;
  }
  return out;
}

/** 16-bit samples for the trip to Rust (half the size of floats in JSON). */
export function toInt16(pcm: Float32Array): number[] {
  const out = new Array<number>(pcm.length);
  for (let i = 0; i < pcm.length; i++) {
    const v = Math.max(-1, Math.min(1, pcm[i]));
    out[i] = Math.round(v * 32767);
  }
  return out;
}
