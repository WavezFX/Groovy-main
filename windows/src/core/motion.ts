// One motion system for the whole app (FIXES_ROUND3B items 2 and 8): the same
// durations and easing for the island, its cards and panels, and Settings.
// Only transform, opacity and clip-path are animated; with Windows' animation
// effects off (reduced motion) every change is a short fade instead.

export const MOTION = {
  /** The island changing shape: F ↔ E, a card, quick settings, voice states. */
  island: 280,
  /** A panel unfolding out of the island, or folding back into it. */
  unfold: 290,
  /** Switching a Settings page. */
  page: 220,
  /** A row, toggle or small bounce. */
  small: 180,
  /** What everything becomes with reduced motion. */
  fade: 120,
} as const;

/** CSS easing for all of the above: a quick start and a soft landing. */
export const EASE_OUT = "cubic-bezier(0.22, 1, 0.36, 1)";

/** The same curve in code (the island's own geometry). */
export function easeOut(p: number): number {
  const x = Math.min(1, Math.max(0, p));
  return 1 - (1 - x) ** 3;
}

/** Windows "Animation effects" off (prefers-reduced-motion). */
export function reducedMotion(): boolean {
  return typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/** A duration, or the short fade when motion is reduced. */
export function duration(ms: number): number {
  return reducedMotion() ? Math.min(ms, MOTION.fade) : ms;
}
