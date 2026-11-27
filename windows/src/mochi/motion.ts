// Groovy's motion tools (round 4E item 1, docs/plan/CHARACTER_MOTION.md §2).
// Three tools only: a spring (frequency and damping), an uneven sine breath,
// and a cubic ease-out. No keyframe tables: every move is a target the
// springs chase, or a short envelope built from the ease-out.

/** Cubic ease-out on 0…1 (clamped). */
export const easeOut = (t: number): number => {
  const u = t <= 0 ? 0 : t >= 1 ? 1 : t;
  return 1 - (1 - u) * (1 - u) * (1 - u);
};

/** A spring's value and speed. */
export interface Spring {
  v: number;
  vel: number;
}

export const spring = (v = 0): Spring => ({ v, vel: 0 });

/**
 * Moves `s` toward `target`: `hz` is the spring's natural frequency, `zeta`
 * its damping ratio (below 1 it overshoots and settles). Semi-implicit Euler,
 * sub-stepped so a long frame can't blow it up. Snaps to exactly `target`
 * once it is closer than `eps` and nearly still, so nothing redraws at rest.
 */
export function stepSpring(s: Spring, target: number, hz: number, zeta: number, dt: number, eps = 0.0005): void {
  if (s.v === target && s.vel === 0) {
    s.v += 0;
    return;
  }
  const w = 2 * Math.PI * hz;
  let left = Math.min(dt, 0.1);
  while (left > 0) {
    const h = Math.min(left, 1 / 120);
    s.vel += (w * w * (target - s.v) - 2 * zeta * w * s.vel) * h;
    s.v += s.vel * h;
    left -= h;
  }
  if (Math.abs(target - s.v) < eps && Math.abs(s.vel) < eps * w) {
    s.v = target + 0; // never −0
    s.vel = 0;
  }
}

/** True when the spring sits exactly on its target. */
export const atRest = (s: Spring, target: number): boolean => s.v === target && s.vel === 0;

/**
 * The pillow breath: 0 (empty) … 1 (full). Uneven on purpose: in over 1.6 of
 * every 4.2 s, a 0.3 s hold, then out over 2.3 s (scaled to `period`).
 */
export function breath(t: number, period = 4.2): number {
  const p = ((t % period) + period) % period / period;
  const inEnd = 1.6 / 4.2;
  const holdEnd = 1.9 / 4.2;
  if (p < inEnd) return 0.5 - 0.5 * Math.cos((p / inEnd) * Math.PI);
  if (p < holdEnd) return 1;
  return 0.5 + 0.5 * Math.cos(((p - holdEnd) / (1 - holdEnd)) * Math.PI);
}

/**
 * A one-off envelope: rises on the ease-out over `rise` s, holds, then falls
 * on the ease-out over `fall` s. 0 before and after.
 */
export function envelope(age: number, rise: number, hold: number, fall: number): number {
  if (age <= 0) return 0;
  if (age < rise) return easeOut(age / rise);
  if (age < rise + hold) return 1;
  const f = age - rise - hold;
  return f >= fall ? 0 : 1 - easeOut(f / fall);
}

/** A tiny deterministic noise in −1…1, for flicker that never repeats exactly. */
export function flicker(t: number, seed = 0): number {
  return (Math.sin(t * 7.3 + seed) * 0.5 + Math.sin(t * 13.1 + seed * 2.1) * 0.3 + Math.sin(t * 23.7 + seed * 0.7) * 0.2);
}
