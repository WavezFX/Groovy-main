// The frame-time test (FIXES_ROUND3B item 8b): CI starts Groovy with
// GROOVY_SMOKE_VIEW=frames, the island is driven through its layouts, and
// every frame's time is recorded from requestAnimationFrame. It fails when
// more than 5% of frames take over 20 ms, or any frame over 50 ms.

export interface FrameStats {
  frames: number;
  over20: number;
  over50: number;
  /** Share of frames over 20 ms, 0–1. */
  share20: number;
  mean: number;
  p95: number;
  max: number;
}

export const SLOW_FRAME_MS = 20;
export const STALL_MS = 50;
export const SLOW_SHARE = 0.05;

export function frameStats(deltas: number[]): FrameStats {
  const d = deltas.filter((v) => Number.isFinite(v) && v >= 0);
  if (!d.length) return { frames: 0, over20: 0, over50: 0, share20: 0, mean: 0, p95: 0, max: 0 };
  const sorted = [...d].sort((a, b) => a - b);
  const over20 = d.filter((v) => v > SLOW_FRAME_MS).length;
  const round = (v: number) => Math.round(v * 10) / 10;
  return {
    frames: d.length,
    over20,
    over50: d.filter((v) => v > STALL_MS).length,
    share20: over20 / d.length,
    mean: round(d.reduce((a, b) => a + b, 0) / d.length),
    p95: round(sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.95))]),
    max: round(sorted[sorted.length - 1]),
  };
}

/** Why the frames fail the test, or null when they pass. */
export function frameVerdict(s: FrameStats): string | null {
  if (s.frames === 0) return "no frames were recorded";
  if (s.over50 > 0) return `${s.over50} frame(s) over ${STALL_MS} ms (longest ${s.max} ms)`;
  if (s.share20 > SLOW_SHARE) return `${(s.share20 * 100).toFixed(1)}% of frames over ${SLOW_FRAME_MS} ms`;
  return null;
}

export interface TourStep {
  name: string;
  run: () => void;
}

/**
 * Runs each step, holds it `holdMs`, and records the time between frames
 * meanwhile, per step. The first frame after a step starts is measured too:
 * that's where a layout change would stall.
 */
export async function frameTour(
  steps: TourStep[],
  holdMs: number,
  raf: (cb: (t: number) => void) => number = (cb) => requestAnimationFrame(cb),
  wait: (ms: number) => Promise<void> = (ms) => new Promise((r) => setTimeout(r, ms)),
): Promise<Record<string, number[]>> {
  const out: Record<string, number[]> = {};
  let current: number[] | null = null;
  let last: number | null = null;
  let running = true;
  const loop = (t: number) => {
    if (!running) return;
    if (last !== null && current) current.push(t - last);
    last = t;
    raf(loop);
  };
  raf(loop);
  for (const step of steps) {
    current = out[step.name] = [];
    step.run();
    await wait(holdMs);
  }
  running = false;
  return out;
}
