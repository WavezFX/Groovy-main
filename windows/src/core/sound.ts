// SoundEngine — port of SoundEngine.swift.
// Groovy's 29 sounds are made in code (core/sounds-synth.ts): no audio files.
// Their timbre follows the look (Night woody, Ember warm, Frost glassy).
// Default volume 0.12, slider range 0–0.2, and several sounds may overlap.
// A file of the user's own in the sounds folder (src-tauri/src/sounds.rs)
// replaces a sound; one that doesn't decode falls back to the built-in one.

import { Bridge } from "./bridge";
import { SAMPLE_RATE, renderSound, type Flavor } from "./sounds-synth";

export const SOUND_NAMES = [
  "peek", "open", "close", "hover", "blip", "slap", "annoyed", "dizzy", "greet",
  "work", "finish", "error", "approval", "question", "approve", "gulp", "tick",
  "send", "love", "pop", "proud", "wink", "yawn", "attach", "think", "search",
  "rate", "sleep", "greeting",
] as const;

export type SoundName = (typeof SOUND_NAMES)[number];

/** Silence after which the audio context is closed. */
const CLOSE_AFTER_MS = 60_000;

class SoundEngine {
  enabled = true;
  volume = 0.12;

  private ctx: AudioContext | null = null;
  private master: GainNode | null = null;
  private buffers = new Map<string, AudioBuffer>();
  private loading: Promise<void> | null = null;
  private idleTimer: number | null = null;
  private closeTimer: number | null = null;
  /** The look's timbre the built-in sounds were made with. */
  private flavor: Flavor = "wood";
  /** The sounds a file of the user's own replaces, as last loaded. */
  customized = new Set<string>();
  /** Sounds still playing, by name, so one can be faded out (the greeting). */
  private playing = new Map<string, Set<{ src: AudioBufferSourceNode; gain: GainNode }>>();

  /** Creates the context and decodes every WAV. Safe to call more than once. */
  preload(): Promise<void> {
    if (this.loading) return this.loading;
    this.loading = (async () => {
      const ctx = this.open();
      if (ctx) await this.loadAll(ctx);
      // Nothing played yet: the context goes again until a sound needs it.
      this.idle();
    })();
    return this.loading;
  }

  /** The audio context, made when a sound needs one (WebView2 starts its audio process for it). */
  private open(): AudioContext | null {
    if (this.ctx) return this.ctx;
    const Ctor = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!Ctor) return null;
    const ctx = new Ctor();
    const master = ctx.createGain();
    master.gain.value = this.volume;
    master.connect(ctx.destination);
    this.ctx = ctx;
    this.master = master;
    return ctx;
  }

  /** Reads the sounds folder again (Settings → Reload sounds). */
  async reload(): Promise<void> {
    await this.preload();
    const ctx = this.open();
    if (ctx) await this.loadAll(ctx);
    this.idle();
  }

  private async loadAll(ctx: AudioContext) {
    const own = new Set((await Bridge.customSounds(SOUND_NAMES)) ?? []);
    const customized = new Set<string>();
    await Promise.all(
      SOUND_NAMES.map(async (name) => {
        if (own.has(name)) {
          try {
            const bytes = await Bridge.customSound(name);
            if (bytes && bytes.byteLength > 0) {
              this.buffers.set(name, await ctx.decodeAudioData(bytes));
              customized.add(name);
              return;
            }
          } catch {
            /* not a format this webview decodes: the built-in sound stays */
          }
        }
        this.makeBuiltIn(name);
      }),
    );
    this.customized = customized;
  }

  /** A built-in sound, made in code; it needs no context, so it outlives one. */
  private makeBuiltIn(name: string) {
    try {
      const samples = renderSound(name, this.flavor, SAMPLE_RATE);
      if (!samples) return;
      const buf = new AudioBuffer({ numberOfChannels: 1, length: samples.length, sampleRate: SAMPLE_RATE });
      buf.copyToChannel(samples, 0);
      this.buffers.set(name, buf);
    } catch {
      /* a missing sound must never break the island */
    }
  }

  /** The look changed: the built-in sounds are made again in its timbre. */
  setFlavor(flavor: Flavor) {
    if (flavor === this.flavor) return;
    this.flavor = flavor;
    if (!this.loading) return;
    for (const name of SOUND_NAMES) if (!this.customized.has(name)) this.makeBuiltIn(name);
  }

  /** WebView2 can hand us a suspended context; call after any user input. */
  resume() {
    this.cancelIdle();
    void this.ctx?.resume();
  }

  private cancelIdle() {
    if (this.idleTimer != null) window.clearTimeout(this.idleTimer);
    if (this.closeTimer != null) window.clearTimeout(this.closeTimer);
    this.idleTimer = null;
    this.closeTimer = null;
  }

  /**
   * Called when the island goes quiet. A running AudioContext keeps an audio
   * thread and its render quantum alive even with nothing playing, which shows
   * up as a steady trickle of CPU on a machine that is supposed to be idle.
   *
   * The delay covers the tail of whatever just played — suspending mid-sound
   * would clip it — and `play()` resumes the context on its own.
   */
  idle() {
    if (!this.ctx || this.idleTimer != null || this.closeTimer != null) return;
    this.idleTimer = window.setTimeout(() => {
      this.idleTimer = null;
      void this.ctx?.suspend();
    }, 1500);
    // A minute of silence: the context is closed, and with it WebView2's audio
    // process (round 3 §9). The sounds themselves are kept; the next one opens
    // a new context.
    this.closeTimer = window.setTimeout(() => {
      this.closeTimer = null;
      const ctx = this.ctx;
      this.ctx = null;
      this.master = null;
      this.playing.clear();
      void ctx?.close().catch(() => {});
    }, CLOSE_AFTER_MS);
  }

  setVolume(v: number) {
    this.volume = Math.max(0, Math.min(0.2, v));
    if (this.master) this.master.gain.value = this.volume;
  }

  setEnabled(on: boolean) {
    this.enabled = on;
  }

  /** Do not disturb (Settings → Home): silent until it is turned off. */
  quiet = false;

  play(name: SoundName | string) {
    if (!this.enabled || this.quiet) return;
    const buf = this.buffers.get(name);
    if (!buf) return;
    this.cancelIdle();
    const ctx = this.open();
    const master = this.master;
    if (!ctx || !master) return;
    if (ctx.state === "suspended") void ctx.resume();
    const src = ctx.createBufferSource();
    src.buffer = buf;
    const gain = ctx.createGain();
    src.connect(gain);
    gain.connect(master);
    const voice = { src, gain };
    let voices = this.playing.get(name);
    if (!voices) this.playing.set(name, (voices = new Set()));
    voices.add(voice);
    src.onended = () => {
      voices.delete(voice);
      gain.disconnect();
    };
    src.start();
  }

  /** Fades every playing copy of `name` to silence over `seconds`, then stops it. */
  fadeOut(name: string, seconds: number) {
    const ctx = this.ctx;
    const voices = this.playing.get(name);
    if (!ctx || !voices) return;
    const t = ctx.currentTime;
    for (const { src, gain } of voices) {
      gain.gain.cancelScheduledValues(t);
      gain.gain.setValueAtTime(gain.gain.value, t);
      gain.gain.linearRampToValueAtTime(0, t + seconds);
      try {
        src.stop(t + seconds);
      } catch {
        /* already stopped */
      }
    }
  }
}

export const Sound = new SoundEngine();
