// Auto-close (round 2 item 2): how long the open island waits once the
// pointer leaves it. 0 is "When I leave": it folds after fsm.LEAVE_GRACE.

export const AUTO_CLOSE_CHOICES = [0, 1, 3, 5, 10, 30] as const;

/** A stored delay → the nearest choice, so older values (15 s, 120 s) still show one. */
export function nearestChoice(seconds: number): number {
  if (!Number.isFinite(seconds) || seconds <= 0) return 0;
  return AUTO_CLOSE_CHOICES.reduce((best, c) => (Math.abs(c - seconds) < Math.abs(best - seconds) ? c : best), AUTO_CLOSE_CHOICES[1]);
}
