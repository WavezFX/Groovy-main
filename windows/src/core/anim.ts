// Groovy's easing and spring helpers for the island and the character (round 4E
// item 1). Built on the same three tools as the character's motion
// (mochi/motion.ts): a damped spring, a cubic ease-out, and curves made from them.
import { easeOut } from "../mochi/motion";

export type EaseFn = (t: number) => number;

export const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
export const clamp = (v: number, lo: number, hi: number) => (v < lo ? lo : v > hi ? hi : v);

/** The settle of a spring released at 0 toward 1 (ζ 0.55), normalised so it ends exactly on 1. */
function springSettle(zeta: number, cycles: number): EaseFn {
  const w = 2 * Math.PI * cycles;
  const wd = w * Math.sqrt(1 - zeta * zeta);
  const raw = (t: number) => 1 - Math.exp(-zeta * w * t) * (Math.cos(wd * t) + ((zeta * w) / wd) * Math.sin(wd * t));
  const end = raw(1);
  return (t) => (t <= 0 ? 0 : t >= 1 ? 1 : raw(t) / end);
}

/** Groovy's curves: everything eases out; "settle" overshoots once, like a soft spring. */
export const Ease = {
  out: easeOut,
  inOut: (t: number) => (t < 0.5 ? (1 - easeOut(1 - 2 * t)) / 2 : 0.5 + easeOut(2 * t - 1) / 2),
  back: springSettle(0.55, 1.1),
  lin: (t: number) => t,
  easeIn: (t: number) => 1 - easeOut(1 - t),
};

/**
 * A CSS-style cubic Bézier timing curve through (0,0), (x1,y1), (x2,y2), (1,1):
 * solves x(t) = p by Newton steps (falling back to halving), then returns y(t).
 */
export function cubicBezier(x1: number, y1: number, x2: number, y2: number): EaseFn {
  const bez = (a: number, b: number, t: number) => 3 * a * t * (1 - t) * (1 - t) + 3 * b * t * t * (1 - t) + t * t * t;
  const slope = (a: number, b: number, t: number) => 3 * a * (1 - t) * (1 - t) + 6 * (b - a) * t * (1 - t) + 3 * (1 - b) * t * t;
  return (p) => {
    if (p <= 0) return 0;
    if (p >= 1) return 1;
    let t = p;
    for (let i = 0; i < 6; i++) {
      const d = slope(x1, x2, t);
      if (Math.abs(d) < 1e-6) break;
      t = clamp(t - (bez(x1, x2, t) - p) / d, 0, 1);
    }
    let lo = 0, hi = 1;
    for (let i = 0; i < 20 && Math.abs(bez(x1, x2, t) - p) > 1e-5; i++) {
      if (bez(x1, x2, t) < p) lo = t; else hi = t;
      t = (lo + hi) / 2;
    }
    return bez(y1, y2, t);
  };
}

/** The island's fold-away curve: a slow start, then a long ease-out (340 ms). */
export const closeCurve = cubicBezier(0.45, 0, 0.2, 1);

/**
 * A damped spring. `response` is the period of one swing in seconds, `damping`
 * the damping ratio (under 1 it overshoots). Sub-stepped so a slow frame can't
 * make it explode.
 */
export class Spring {
  value: number;
  target: number;
  velocity = 0;
  omega: number;
  zeta: number;

  constructor(value: number, response = 0.5, damping = 0.72) {
    this.value = value;
    this.target = value;
    this.omega = (2 * Math.PI) / response;
    this.zeta = damping;
  }

  configure(response: number, damping: number) {
    this.omega = (2 * Math.PI) / response;
    this.zeta = damping;
  }

  /** Puts it at `value`, still. */
  set(value: number) {
    this.value = this.target = value;
    this.velocity = 0;
  }

  /** Close enough to its target, and slow enough, to call it still. */
  get settled(): boolean {
    return Math.abs(this.target - this.value) < 0.01 && Math.abs(this.velocity) < 0.05;
  }

  step(dt: number) {
    const n = Math.max(1, Math.ceil(dt * 240));
    const h = dt / n;
    const w2 = this.omega * this.omega;
    const c = 2 * this.zeta * this.omega;
    for (let i = 0; i < n; i++) {
      this.velocity += (w2 * (this.target - this.value) - c * this.velocity) * h;
      this.value += this.velocity * h;
    }
  }
}

/**
 * A value that grows on a spring and shrinks on a timed curve (the island
 * opens with a little give and folds away without bouncing). Idle once there.
 */
export class Tracked {
  private s: Spring;
  private mode: "idle" | "spring" | "curve" = "idle";
  private from = 0;
  private to = 0;
  private start = 0;
  private dur = 0;
  private ease: EaseFn = closeCurve;

  constructor(value: number) {
    this.s = new Spring(value);
  }

  get value(): number {
    return this.s.value;
  }

  get animating(): boolean {
    return this.mode !== "idle";
  }

  /** Straight to `v`, no motion. */
  jump(v: number) {
    this.s.set(v);
    this.mode = "idle";
  }

  /** Springs to `v` (opening, growing). */
  springTo(v: number, response = 0.5, damping = 0.72) {
    this.s.configure(response, damping);
    this.s.target = v;
    this.mode = "spring";
  }

  /** Eases to `v` over `durationMs` without overshoot (closing, shrinking). */
  curveTowards(v: number, durationMs = 340, now = performance.now(), ease: EaseFn = closeCurve) {
    this.from = this.s.value;
    this.to = v;
    this.start = now;
    this.dur = durationMs;
    this.ease = ease;
    this.s.target = v;
    this.s.velocity = 0;
    this.mode = "curve";
  }

  step(dt: number, now = performance.now()) {
    if (this.mode === "spring") {
      this.s.step(dt);
      if (this.s.settled) {
        this.s.set(this.s.target);
        this.mode = "idle";
      }
    } else if (this.mode === "curve") {
      const p = this.dur > 0 ? clamp((now - this.start) / this.dur, 0, 1) : 1;
      this.s.value = lerp(this.from, this.to, this.ease(p));
      if (p >= 1) {
        this.s.velocity = 0;
        this.mode = "idle";
      }
    }
  }
}
