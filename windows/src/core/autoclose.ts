// Auto-close (round 2 item 2, round 4 item 2): how long the open island waits
// once the pointer leaves it. 0 is "When I leave": it folds after
// fsm.LEAVE_GRACE. CLICK_AWAY keeps it open until you click outside it.

export const CLICK_AWAY = -1;

export const AUTO_CLOSE_CHOICES = [0, 1, 2, 3, 5, CLICK_AWAY] as const;

/** A stored delay → the nearest choice, so older values (10 s, 30 s, 120 s) still show one. */
export function nearestChoice(seconds: number): number {
  if (seconds === CLICK_AWAY) return CLICK_AWAY;
  if (!Number.isFinite(seconds) || seconds <= 0) return 0;
  const timed = AUTO_CLOSE_CHOICES.filter((c) => c > 0);
  return timed.reduce((best, c) => (Math.abs(c - seconds) < Math.abs(best - seconds) ? c : best), timed[0]);
}
