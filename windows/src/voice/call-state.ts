// The voice call as an explicit state machine, so what the island shows, when
// the microphone listens and when Groovy speaks always agree. Pure logic: the
// island's VoiceHost feeds it events and acts on the state it returns.
//
//   idle ──start──▶ listening ──heard──▶ processing ──answered──▶ speaking
//                     ▲   │                  │                      │
//                     │   └─mute─▶ muted     └─silent (no TTS)──┐   │
//                     │            │                            ▼   │
//                     └──unmute────┘◀────────────── listening ◀─spoke/interrupt
//
// Any state: "fail" → error, "end" → idle. From error, "start" tries again.

export type CallState = "idle" | "listening" | "processing" | "speaking" | "muted" | "error";

export type CallEvent =
  | "start" // the call button, or the wake word with "keep talking" on
  | "heard" // a turn was spoken and is being answered
  | "answered" // the answer is being read aloud
  | "silent" // the answer arrived and is not read aloud
  | "spoke" // finished reading it
  | "interrupt" // the user spoke over him, or tapped to interrupt
  | "mute"
  | "unmute"
  | "fail" // no microphone, device lost, transcription failed
  | "end"; // hang up, "bye", long silence

const TABLE: Record<CallState, Partial<Record<CallEvent, CallState>>> = {
  idle: { start: "listening" },
  listening: { heard: "processing", mute: "muted" },
  processing: { answered: "speaking", silent: "listening", mute: "muted" },
  speaking: { spoke: "listening", interrupt: "listening", mute: "muted" },
  // Muted: the call stays up, the microphone is closed; answers still arrive.
  muted: { unmute: "listening", answered: "muted", silent: "muted", spoke: "muted" },
  error: { start: "listening" },
};

/** The state after `event`; an event that doesn't apply leaves it unchanged. */
export function nextCallState(state: CallState, event: CallEvent): CallState {
  if (event === "end") return "idle";
  if (event === "fail") return state === "idle" ? "idle" : "error";
  return TABLE[state][event] ?? state;
}

/** Whether the microphone should be heard in this state. */
export function micOpen(state: CallState, interruptible: boolean): boolean {
  return state === "listening" || (state === "speaking" && interruptible);
}

/**
 * Spots the user speaking over Groovy: a frame clearly louder than the room's
 * noise floor, held for `holdFrames` frames in a row (about a quarter of a
 * second), so a click or a cough doesn't cut him off.
 */
export class BargeIn {
  private run = 0;
  private holdFrames: number;
  /** How many times louder than the noise floor counts as speech. */
  private ratio: number;
  /** Never quieter than this mean-square power (≈ −40 dBFS). */
  private floor: number;

  constructor(holdFrames = 4, ratio = 24, floor = 1e-4) {
    this.holdFrames = holdFrames;
    this.ratio = ratio;
    this.floor = floor;
  }

  reset() {
    this.run = 0;
  }

  /** One frame's power and the current noise floor in; true once speech is held long enough. */
  feed(power: number, noise: number): boolean {
    const threshold = Math.max(this.floor, noise * this.ratio);
    this.run = power > threshold ? this.run + 1 : 0;
    if (this.run >= this.holdFrames) {
      this.run = 0;
      return true;
    }
    return false;
  }
}

/**
 * The microphone level for the visualiser, 0…1: fast attack, slow release,
 * on a log scale between the noise floor and loud speech.
 */
export class LevelMeter {
  level = 0;
  feed(power: number): number {
    const db = 10 * Math.log10(Math.max(power, 1e-10)); // −100 … 0
    const target = Math.min(1, Math.max(0, (db + 60) / 50)); // −60 dB → 0, −10 dB → 1
    this.level = target > this.level ? target : this.level * 0.85 + target * 0.15;
    return this.level;
  }
}
