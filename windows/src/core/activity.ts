// Nothing runs when nothing is happening (round 4F item 7).
//
// The rule: a page timer or frame loop runs only while its surface is on show
// and something is moving. Folded and idle, the island's page makes no
// callbacks at all. Clocks that must keep time while nothing moves (break
// reminders) wait for their moment with one timer, never by polling.
//
// `countCallbacks` is CI's proof (GROOVY_SMOKE_VIEW=quiet, scripts/quiet-test.ps1):
// it counts every timer and frame callback the page runs over a window, with
// where each was set, so a loop that wakes the page shows by name.

export interface CallbackCount {
  frames: number;
  timeouts: number;
  intervals: number;
  /** Where the callbacks that ran were set (first app frame of the stack), most first. */
  sources: { at: string; calls: number }[];
}

/** The first stack line outside this file, as "file:line:column" (the bundle is minified: the column tells). */
function caller(): string {
  const lines = (new Error().stack ?? "").split("\n").slice(1);
  for (const l of lines) {
    if (/core[/\\]activity\.ts/.test(l) || l.includes("countCallbacks") || l.includes("caller")) continue;
    const m = l.match(/([^/\\( ]+\.m?[jt]s):(\d+)(?::(\d+))?/);
    if (m) return m[3] ? `${m[1]}:${m[2]}:${m[3]}` : `${m[1]}:${m[2]}`;
  }
  return "?";
}

/**
 * Counts callbacks from now until the returned `stop` is called; `stop` puts
 * the page's own timer functions back and returns the counts. Test use only.
 */
export function countCallbacks(w: Window = window): () => CallbackCount {
  const count: CallbackCount = { frames: 0, timeouts: 0, intervals: 0, sources: [] };
  const by = new Map<string, number>();
  const seen = (at: string) => by.set(at, (by.get(at) ?? 0) + 1);
  const own = { raf: w.requestAnimationFrame, st: w.setTimeout, si: w.setInterval };
  const raf = w.requestAnimationFrame.bind(w);
  const st = w.setTimeout.bind(w);
  const si = w.setInterval.bind(w);
  type Fn = (...a: unknown[]) => void;
  const wrap = (kind: "frames" | "timeouts" | "intervals", fn: unknown, at: string): unknown =>
    typeof fn === "function"
      ? (...a: unknown[]) => {
          count[kind]++;
          seen(at);
          (fn as Fn)(...a);
        }
      : fn;
  w.requestAnimationFrame = ((cb: FrameRequestCallback) => raf(wrap("frames", cb, caller()) as FrameRequestCallback)) as typeof w.requestAnimationFrame;
  w.setTimeout = ((cb: unknown, ms?: number, ...a: unknown[]) => st(wrap("timeouts", cb, caller()) as TimerHandler, ms, ...a)) as typeof w.setTimeout;
  w.setInterval = ((cb: unknown, ms?: number, ...a: unknown[]) => si(wrap("intervals", cb, caller()) as TimerHandler, ms, ...a)) as typeof w.setInterval;
  return () => {
    w.requestAnimationFrame = own.raf;
    w.setTimeout = own.st;
    w.setInterval = own.si;
    count.sources = [...by.entries()].sort((a, b) => b[1] - a[1]).map(([at, calls]) => ({ at, calls }));
    return count;
  };
}

/** Milliseconds from `now` to the earliest of `dues` (null when there is none), at least `min`. */
export function untilNext(dues: (number | null | undefined)[], now: number, min = 1_000): number | null {
  let best: number | null = null;
  for (const d of dues) if (d != null && (best == null || d < best)) best = d;
  return best == null ? null : Math.max(min, best - now);
}
