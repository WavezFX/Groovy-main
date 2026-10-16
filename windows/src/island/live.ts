// What used to live in the pill at the bottom of the screen, now in the island
// (round 3: one island only). Rust (flow/live.rs) sends:
// - `live-state`: dictation, a recorded call and « OK Groovy », whole on every change;
// - `live-bands`: the nine voice levels while you speak;
// - `island-notice`: a message with an optional button, which opens the island;
// - `voice-say`: what « OK Groovy » answers, to say out loud here.

import { onEvent, IS_TAURI } from "../core/bridge";
import { FlowBridge, type LiveState, type Notice } from "../core/flow";
import { State } from "../core/state";

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
  /** ElevenLabs' MP3, base64. */
  audio: string | null;
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

async function sayMp3(base64: string): Promise<boolean> {
  try {
    const audio = new Audio(`data:audio/mpeg;base64,${base64}`);
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
    if (!(s.audio && (await sayMp3(s.audio)))) await sayWithSystem(s.text, s.lang, s.gender);
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
