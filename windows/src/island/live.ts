// What used to live in the pill at the bottom of the screen, now in the island
// (round 3: one island only). Rust (flow/live.rs) sends:
// - `live-state`: dictation, a recorded call and « OK Groovy », whole on every change;
// - `live-bands`: the nine voice levels while you speak;
// - `island-notice`: a message with an optional button, which opens the island;
// - `voice-say`: what « OK Groovy » answers, to say out loud here.

import { onEvent, IS_TAURI } from "../core/bridge";
import { FlowBridge, type LiveState, type Notice } from "../core/flow";
import { State } from "../core/state";
import { N_ } from "../i18n/i18n";

export const BANDS = 9;

/** The voice levels (0…1) Rust sent last, read by the badge and the strip. */
export const liveLevels = new Array<number>(BANDS).fill(0);

/** Notices waiting in the island, oldest first. */
export const Notices = {
  list: [] as Notice[],
  get current(): Notice | null {
    return this.list[0] ?? null;
  },
  push(n: Notice) {
    if (this.list.some((x) => x.id === n.id)) return;
    this.list.push(n);
  },
  /** The ✕ or the time ran out: Rust forgets it. */
  dismiss(id: number) {
    this.list = this.list.filter((n) => n.id !== id);
    void FlowBridge.liveAction("dismiss", id);
  },
  /** Its button: Rust does what it says. */
  act(id: number) {
    this.list = this.list.filter((n) => n.id !== id);
    void FlowBridge.liveAction("announcement", id);
  },
};

/** Something is listening (dictation, a call being recorded, « OK Groovy »). */
export function liveBusy(s: LiveState = State.live): boolean {
  return s.phase !== "idle" || s.noteStartedAt !== null || s.voice !== null;
}

/** The icon a notice's `icon` names, from the island's own set. */
export function noticeIcon(icon: string): string {
  return ({ mic: "mic", lock: "lock", clipboard: "copy", download: "download", check: "check", error: "x", bolt: "bolt" } as Record<string, string>)[icon] ?? "check";
}

// ── « OK Groovy » speaks ──────────────────────────────────────────────────────

interface Say {
  id: number;
  text: string;
  lang: string | null;
  /** Groovy's voice (Fish Audio's MP3 or Kokoro's WAV) or ElevenLabs' MP3, base64. */
  audio: string | null;
  mime?: string | null;
  gender: "female" | "male";
}

/** Groovy is saying an answer (the bars move with it). */
export let speaking = false;

/** The system's voice in the answer's language, a woman's or a man's when the system says. */
export function systemVoice(voices: SpeechSynthesisVoice[], lang: string | null, gender: string): SpeechSynthesisVoice | null {
  const want = (lang ?? navigator.language).toLowerCase();
  const short = want.split("-")[0];
  const inLang = voices
    .filter((v) => v.lang.toLowerCase() === want)
    .concat(voices.filter((v) => v.lang.toLowerCase().split(/[-_]/)[0] === short && v.lang.toLowerCase() !== want));
  const female = /(female|woman|zira|hortense|julie|denise|aria|jenny|samantha|amelie|eloise)/i;
  const male = /(male|man\b|david|paul|guy|henri|claude|mark|thomas)/i;
  const pick = inLang.find((v) => (gender === "male" ? male.test(v.name) && !female.test(v.name) : female.test(v.name)));
  return pick ?? inLang[0] ?? null;
}

async function sayWithSystem(text: string, lang: string | null, gender: string): Promise<void> {
  const synth = window.speechSynthesis;
  if (synth && synth.getVoices().length === 0) {
    await new Promise<void>((r) => {
      synth.addEventListener("voiceschanged", () => r(), { once: true });
      setTimeout(r, 600);
    });
  }
  if (synth && synth.getVoices().length > 0) {
    await new Promise<void>((resolve) => {
      const u = new SpeechSynthesisUtterance(text);
      const voice = systemVoice(synth.getVoices(), lang, gender);
      if (voice) u.voice = voice;
      u.lang = voice?.lang ?? lang ?? navigator.language;
      u.onend = () => resolve();
      u.onerror = () => resolve();
      synth.cancel();
      synth.speak(u);
    });
    return;
  }
  await FlowBridge.sayNative(text, lang);
}

async function sayAudio(base64: string, mime = "audio/mpeg"): Promise<boolean> {
  try {
    const audio = new Audio(`data:${mime};base64,${base64}`);
    await audio.play();
    await new Promise<void>((resolve) => {
      audio.onended = () => resolve();
      audio.onerror = () => resolve();
    });
    return true;
  } catch {
    return false;
  }
}

async function say(s: Say) {
  speaking = true;
  State.notify();
  try {
    if (!(s.audio && (await sayAudio(s.audio, s.mime ?? undefined)))) await sayWithSystem(s.text, s.lang, s.gender);
  } finally {
    speaking = false;
    State.notify();
    void FlowBridge.voiceDone(s.id);
  }
}

// ── Wiring ────────────────────────────────────────────────────────────────────

export interface LiveHost {
  /** A notice came: open the island on it. */
  showNotice(): void;
}

export async function registerLiveHandlers(host: LiveHost) {
  if (!IS_TAURI) return;
  const first = await FlowBridge.liveState();
  if (first) State.live = first;
  for (const n of (await FlowBridge.liveNotices()) ?? []) Notices.push(n);
  if (Notices.current) host.showNotice();
  void onEvent<LiveState>("live-state", (s) => {
    if (s.phase === "listening" && State.live.phase !== "listening") liveLevels.fill(0);
    State.live = s;
    State.notify();
  });
  void onEvent<number[]>("live-bands", (bands) => {
    for (let i = 0; i < BANDS; i++) liveLevels[i] = bands[i] ?? 0;
  });
  void onEvent<Notice>("island-notice", (n) => {
    Notices.push(n);
    host.showNotice();
  });
  void onEvent<Say>("voice-say", (s) => void say(s));
}

// ── The badge and the live strip (round 3 item 3, mockup Voice.dc.html) ──────

export type LiveKind = "recording" | "dictating" | "call" | "voice" | "message";

export interface LiveView {
  kind: LiveKind;
  /** Green while Groovy listens, red while a call is recorded. */
  tone: "green" | "red";
  /** English key, shown with t(); {0} is the clock when `timed`. */
  label: string;
  timed: boolean;
  /** What Stop does: a live_action, or the island call's toggle; null: no Stop. */
  stop: "finish" | "stopNote" | "endCall" | null;
  /** « OK Groovy »'s words: what it heard, then its answer ({1} in the label). */
  words?: string;
}

/**
 * What the badge shows, the most important first: a call being recorded,
 * dictation, a voice call with Groovy, « OK Groovy », a short message.
 * Null: nothing is listening, no badge.
 */
export function liveView(s: LiveState, voiceCall: boolean): LiveView | null {
  if (s.noteStartedAt !== null) return { kind: "recording", tone: "red", label: N_("Recording {0}"), timed: true, stop: "stopNote" };
  if (s.phase === "listening" || s.phase === "locked") return { kind: "dictating", tone: "green", label: N_("Dictating {0}"), timed: true, stop: "finish" };
  if (s.phase === "thinking") return { kind: "dictating", tone: "green", label: N_("Writing it down…"), timed: false, stop: null };
  if (voiceCall) return { kind: "call", tone: "green", label: N_("Talking with Groovy {0}"), timed: true, stop: "endCall" };
  if (s.voice) {
    // Listening → heard "…" → thinking → the answer, in the island (round 4 item 1).
    const { phase, heard, answer } = s.voice;
    if (phase === "speaking" && answer) return { kind: "voice", tone: "green", label: "{1}", timed: false, stop: null, words: answer };
    if (phase === "thinking") return { kind: "voice", tone: "green", label: heard ? N_("Thinking about “{1}”…") : N_("Thinking…"), timed: false, stop: null, words: heard };
    if (phase === "listening" && heard) return { kind: "voice", tone: "green", label: N_("Heard “{1}”"), timed: false, stop: null, words: heard };
    const label = phase === "speaking" ? N_("Speaking…") : N_("Listening…");
    return { kind: "voice", tone: "green", label, timed: false, stop: null };
  }
  switch (s.phase) {
    case "copied":
      return { kind: "message", tone: "green", label: N_("Copied · press Ctrl+V"), timed: false, stop: null };
    case "pastedAgain":
      return { kind: "message", tone: "green", label: N_("Pasted again"), timed: false, stop: null };
    case "failed":
      return { kind: "message", tone: "red", label: N_("Microphone unavailable"), timed: false, stop: null };
    default:
      return null;
  }
}

/** m:ss, or h:mm:ss past an hour. */
export function liveClock(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}

/** When each kind started, as the island saw it (Rust sends the recording's own start). */
export class LiveClock {
  private started = new Map<LiveKind, number>();

  /** The start time for `view` at `now`, remembered while it lasts. */
  start(view: LiveView | null, s: LiveState, now: number): number | null {
    for (const k of [...this.started.keys()]) if (k !== view?.kind) this.started.delete(k);
    if (!view || !view.timed) return null;
    if (view.kind === "recording" && s.noteStartedAt !== null) return s.noteStartedAt;
    if (!this.started.has(view.kind)) this.started.set(view.kind, now);
    return this.started.get(view.kind)!;
  }
}
