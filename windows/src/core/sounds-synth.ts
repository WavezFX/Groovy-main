// Groovy's sounds, made in code (FIXES_ROUND3 item 7, FIXES_ROUND4 item 6):
// no audio files to license or ship. Each event name renders to a short mono
// buffer of samples — soft bells, glides, taps and puffs — once, when the
// island loads its sounds or the look changes. The recipes are those of the
// old scripts/gen-sounds.py (which made the WAVs this replaces); the bell's
// timbre follows the look: Night woody, Ember warm, Frost glassy.

export type Flavor = "wood" | "warm" | "glass";

/** The look's timbre: Night → wood, Ember → warm, Frost → glass. */
export function flavorOf(look: string): Flavor {
  return look === "ember" ? "warm" : look === "frost" ? "glass" : "wood";
}

export const SAMPLE_RATE = 48000;
/** Every sound ends within this, in seconds (the longest is approval, ~0.9 s). */
export const MAX_SECONDS = 1.2;

type Samples = Float32Array<ArrayBuffer>;

function len(sr: number, dur: number): number {
  return Math.max(1, Math.round(sr * dur));
}

/** Quick attack, exponential-ish release over the whole length. */
function env(i: number, n: number, sr: number, attack: number, release: number, curve: number): number {
  const t = i / sr;
  const dur = n / sr;
  const a = Math.min(1, t / Math.max(attack, 1e-4));
  const r = Math.min(1, Math.max(0, (dur - t) / Math.max(release, 1e-4)));
  return a * r * Math.exp((-curve * t) / Math.max(dur, 1e-4));
}

/** A small deterministic noise source (the same puff every time). */
function rng(seed: number): () => number {
  let s = seed >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let x = Math.imul(s ^ (s >>> 15), 1 | s);
    x = (x + Math.imul(x ^ (x >>> 7), 61 | x)) ^ x;
    return ((x ^ (x >>> 14)) >>> 0) / 4294967296;
  };
}

interface Synth {
  sr: number;
  flavor: Flavor;
}

function bell(s: Synth, freq: number, dur: number, amp = 0.5, bright = 0.35, curve = 4): Samples {
  const n = len(s.sr, dur);
  const out = new Float32Array(n);
  // Partials and how fast they fade: a marimba bar, a warm bell, a glass.
  const [ratio, fade, second, extra] =
    s.flavor === "wood" ? [3.93, 22, 0.08, 0] :
    s.flavor === "warm" ? [2.76, 10, 0.3, 0] :
    [2.76, 5, 0.12, 0.18];
  const w = 2 * Math.PI * freq;
  for (let i = 0; i < n; i++) {
    const t = i / s.sr;
    let v = Math.sin(w * t) + bright * Math.sin(w * ratio * t) * Math.exp(-fade * t) + second * Math.sin(2 * w * t);
    if (extra) v += extra * Math.sin(w * 5.4 * t) * Math.exp(-9 * t);
    out[i] = amp * v * env(i, n, s.sr, 0.004, 0.05, s.flavor === "wood" ? curve * 1.4 : curve);
  }
  // Wood: the mallet's tap at the start.
  if (s.flavor === "wood") add(out, noise(s, 0.006, 0.12 * amp, 0.4, 20), 0, s.sr);
  return out;
}

function tone(s: Synth, freq: number, dur: number, amp = 0.5, shape: "sine" | "tri" | "soft-square" = "sine", curve = 3, vib = 0, vibRate = 6): Samples {
  const n = len(s.sr, dur);
  const out = new Float32Array(n);
  let ph = 0;
  for (let i = 0; i < n; i++) {
    const t = i / s.sr;
    ph += (2 * Math.PI * freq * (1 + vib * Math.sin(2 * Math.PI * vibRate * t))) / s.sr;
    const v = shape === "tri" ? (2 / Math.PI) * Math.asin(Math.sin(ph)) : shape === "soft-square" ? Math.tanh(3 * Math.sin(ph)) * 0.7 : Math.sin(ph);
    out[i] = amp * v * env(i, n, s.sr, 0.006, 0.05, curve);
  }
  return out;
}

function glide(s: Synth, f0: number, f1: number, dur: number, amp = 0.5, shape: "sine" | "tri" = "sine", curve = 3, vib = 0): Samples {
  const n = len(s.sr, dur);
  const out = new Float32Array(n);
  let ph = 0;
  for (let i = 0; i < n; i++) {
    const t = i / s.sr;
    const f = f0 * (f1 / f0) ** (t / dur) * (1 + vib * Math.sin(2 * Math.PI * 7 * t));
    ph += (2 * Math.PI * f) / s.sr;
    const v = shape === "sine" ? Math.sin(ph) : (2 / Math.PI) * Math.asin(Math.sin(ph));
    out[i] = amp * v * env(i, n, s.sr, 0.004, 0.04, curve);
  }
  return out;
}

function noise(s: Synth, dur: number, amp = 0.3, lp = 0.15, curve = 12): Samples {
  const n = len(s.sr, dur);
  const out = new Float32Array(n);
  const r = rng(7);
  let acc = 0;
  let peak = 1e-9;
  for (let i = 0; i < n; i++) {
    // Box–Muller: a normal sample, then a one-pole low-pass.
    const g = Math.sqrt(-2 * Math.log(1 - r())) * Math.cos(2 * Math.PI * r());
    acc += lp * (g - acc);
    out[i] = acc;
    peak = Math.max(peak, Math.abs(acc));
  }
  for (let i = 0; i < n; i++) out[i] = (amp * out[i] / peak) * env(i, n, s.sr, 0.001, 0.02, curve);
  return out;
}

function add(into: Samples, part: Samples, start: number, sr: number) {
  const at = Math.round(start * sr);
  for (let i = 0; i < part.length && at + i < into.length; i++) into[at + i] += part[i];
}

/** Parts are [start seconds, samples], mixed into one buffer with a short tail. */
function seq(s: Synth, ...parts: [number, Samples][]): Samples {
  const end = Math.max(...parts.map(([t, p]) => Math.round(t * s.sr) + p.length)) + Math.round(0.02 * s.sr);
  const out = new Float32Array(end);
  for (const [t, p] of parts) add(out, p, t, s.sr);
  return out;
}

function mix(s: Synth, ...parts: Samples[]): Samples {
  return seq(s, ...parts.map((p) => [0, p] as [number, Samples]));
}

function note(name: string): number {
  const names: Record<string, number> = { C: -9, D: -7, E: -5, F: -4, G: -2, A: 0, B: 2 };
  const semis = names[name[0]] + (name.includes("#") ? 1 : 0);
  const octave = Number(name[name.length - 1]);
  return 440 * 2 ** ((semis + 12 * (octave - 4)) / 12);
}

type Recipe = (s: Synth) => Samples;
const N = note;

/** One sound per event name: the names SoundEngine plays (core/sound.ts). */
export const RECIPES: Record<string, Recipe> = {
  pop: (s) => glide(s, 900, 380, 0.09, 0.5, "sine", 5),
  blip: (s) => tone(s, 1320, 0.05, 0.5, "sine", 6),
  tick: (s) => mix(s, tone(s, 2200, 0.018, 0.5, "sine", 8), noise(s, 0.018, 0.2, 0.5)),
  hover: (s) => tone(s, 760, 0.06, 0.4, "sine", 5),
  open: (s) => seq(s, [0, bell(s, N("C5"), 0.18)], [0.07, bell(s, N("G5"), 0.25)]),
  close: (s) => seq(s, [0, bell(s, N("G5"), 0.16)], [0.07, bell(s, N("C5"), 0.22)]),
  peek: (s) => glide(s, 500, 1100, 0.14, 0.45, "sine", 3),
  greet: (s) => seq(s, [0, bell(s, N("C5"), 0.25)], [0.09, bell(s, N("E5"), 0.25)], [0.18, bell(s, N("G5"), 0.45)]),
  // The Seed intro: the seed's soft pop as it cracks, then a rising chime as he comes out.
  greeting: (s) => seq(
    s,
    [0, mix(s, glide(s, 520, 180, 0.11, 0.55, "sine", 4), noise(s, 0.03, 0.18, 0.3, 14))],
    [0.12, bell(s, N("C5"), 0.22, 0.4)], [0.2, bell(s, N("E5"), 0.22, 0.42)],
    [0.28, bell(s, N("G5"), 0.24, 0.44)], [0.36, bell(s, N("C6"), 0.55, 0.5, 0.5, 3)],
  ),
  finish: (s) => seq(
    s,
    [0, bell(s, N("C5"), 0.3)], [0.08, bell(s, N("E5"), 0.3)],
    [0.16, bell(s, N("G5"), 0.3)], [0.26, bell(s, N("C6"), 0.8, 0.5, 0.5, 3)],
  ),
  approval: (s) => seq(
    s,
    [0, bell(s, N("E5"), 0.3, 0.5, 0.5)], [0.16, bell(s, N("B5"), 0.3, 0.5, 0.5)],
    [0.42, bell(s, N("E5"), 0.25)], [0.56, bell(s, N("B5"), 0.3)],
  ),
  approve: (s) => seq(s, [0, tone(s, N("G5"), 0.08, 0.5, "tri")], [0.07, bell(s, N("C6"), 0.3)]),
  question: (s) => seq(s, [0, bell(s, N("C5"), 0.2)], [0.12, glide(s, N("C5"), N("F5"), 0.28, 0.45)]),
  error: (s) => seq(s, [0, tone(s, N("E4"), 0.18, 0.35, "soft-square")], [0.16, tone(s, N("C4"), 0.32, 0.35, "soft-square")]),
  rate: (s) => seq(s, [0, bell(s, N("G4"), 0.25)], [0.18, bell(s, N("E4"), 0.25)], [0.36, bell(s, N("C4"), 0.5)]),
  attach: (s) => seq(s, [0, glide(s, 300, 1400, 0.16, 0.25, "tri")], [0.16, tone(s, 1800, 0.03, 0.5, "sine", 8)]),
  send: (s) => mix(s, glide(s, 400, 1600, 0.22, 0.4, "sine", 2), noise(s, 0.22, 0.12, 0.05, 4)),
  gulp: (s) => seq(s, [0, glide(s, 700, 220, 0.14, 0.5, "sine", 3)], [0.12, tone(s, 110, 0.12, 0.6, "sine", 6)]),
  search: (s) => seq(s, [0, bell(s, 1046, 0.6, 0.4, 0.2, 2.5)], [0.25, bell(s, 1046, 0.5, 0.15, 0.35, 3)]),
  think: (s) => seq(s, [0, tone(s, 660, 0.07, 0.35)], [0.12, tone(s, 784, 0.07, 0.35)], [0.24, tone(s, 880, 0.09, 0.3)]),
  work: (s) => seq(s, ...[0, 1, 2, 3].map((i) => [i * 0.07, mix(s, tone(s, 1500 + 120 * (i % 2), 0.025, 0.5, "sine", 8), noise(s, 0.02, 0.15, 0.4))] as [number, Samples])),
  slap: (s) => mix(s, noise(s, 0.09, 0.8, 0.35, 18), tone(s, 180, 0.08, 0.4, "sine", 10)),
  dizzy: (s) => glide(s, 900, 300, 0.7, 0.4, "sine", 1.5, 0.08),
  annoyed: (s) => seq(s, [0, tone(s, N("D4"), 0.14, 0.45, "tri")], [0.13, tone(s, N("C#4"), 0.25, 0.45, "tri")]),
  love: (s) => seq(s, [0, tone(s, N("E5"), 0.3, 0.5, "sine", 3, 0.01)], [0.18, tone(s, N("A5"), 0.5, 0.5, "sine", 2.5, 0.012)]),
  proud: (s) => seq(
    s,
    [0, bell(s, N("G4"), 0.15)], [0.1, bell(s, N("C5"), 0.15)], [0.2, bell(s, N("E5"), 0.15)],
    [0.3, bell(s, N("G5"), 0.15)], [0.4, bell(s, N("C6"), 0.5, 0.5, 0.5)],
  ),
  wink: (s) => glide(s, 1100, 1700, 0.08, 0.4, "sine", 4),
  sleep: (s) => seq(s, [0, tone(s, N("E4"), 0.5, 0.35, "sine", 2)], [0.35, tone(s, N("C4"), 0.8, 0.3, "sine", 2)]),
  yawn: (s) => glide(s, 520, 240, 0.9, 0.35, "tri", 1.2, 0.015),
};

/** Renders one sound, peak-normalised to 0.8 like the old WAVs. Null for a name with no recipe. */
export function renderSound(name: string, flavor: Flavor, sr = SAMPLE_RATE): Float32Array<ArrayBuffer> | null {
  const recipe = RECIPES[name];
  if (!recipe) return null;
  const out = recipe({ sr, flavor });
  let peak = 1e-9;
  for (const v of out) peak = Math.max(peak, Math.abs(v));
  const k = 0.8 / peak;
  for (let i = 0; i < out.length; i++) out[i] *= k;
  return out;
}
