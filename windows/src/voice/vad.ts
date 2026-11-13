// Energy-based voice-activity detector — a port of the Mac's EnergyVAD.swift.
//
//   - Calibration (~0.5 s): fast noise-floor learning, no triggers.
//   - Then the noise floor drifts slowly; speech starts when power > noise × 8.
//   - A segment ends after `silenceSeconds` below noise × 2, or after 30 s.
//   - reset() keeps the learned noise floor.

export type VadEvent = "none" | "start" | "end";

const REF_RATE = 43;
const CALIB = 22;
const RISE = 8;
const FALL = 2;
const MAX_ACTIVE = 1300;

export class EnergyVAD {
  isActive = false;
  noisePower = 1e-7;
  private calibCount = 0;
  private silentCount = 0;
  private activeCount = 0;
  private segSum = 0;
  private segN = 0;
  private calibLimit = CALIB;
  private silenceLimit = 35;
  private activeLimit = MAX_ACTIVE;
  private kCalib = 0.15;
  private kIdle = 0.005;
  private kSilent = 0.001;
  private fps = REF_RATE;

  constructor(framesPerSecond = REF_RATE, silenceSeconds = 0.8) {
    this.setFrameRate(framesPerSecond);
    this.setSilence(silenceSeconds);
  }

  /** Frames per second actually delivered: keeps durations and smoothing per second. */
  setFrameRate(fps: number) {
    if (!(fps > 1)) return;
    this.fps = fps;
    const r = fps / REF_RATE;
    this.calibLimit = Math.max(1, Math.round(CALIB * r));
    this.activeLimit = Math.max(1, Math.round(MAX_ACTIVE * r));
    const k = (perRef: number) => 1 - Math.pow(1 - perRef, 1 / r);
    this.kCalib = k(0.15);
    this.kIdle = k(0.005);
    this.kSilent = k(0.001);
  }

  /** How long a pause ends a segment (longer while a command is being spoken). */
  setSilence(seconds: number) {
    this.silenceLimit = Math.max(1, Math.round(seconds * this.fps));
  }

  reset() {
    this.isActive = false;
    this.silentCount = 0;
    this.activeCount = 0;
    this.segSum = 0;
    this.segN = 0;
  }

  /** One frame's mean-square power in, whether a segment started or ended out. */
  feed(power: number): VadEvent {
    if (this.calibCount < this.calibLimit) {
      this.calibCount++;
      this.noisePower = this.noisePower * (1 - this.kCalib) + power * this.kCalib;
      return "none";
    }
    if (!this.isActive) {
      this.noisePower = this.noisePower * (1 - this.kIdle) + power * this.kIdle;
      if (power > this.noisePower * RISE) {
        this.isActive = true;
        this.silentCount = 0;
        this.activeCount = 0;
        this.segSum = power;
        this.segN = 1;
        return "start";
      }
      return "none";
    }
    this.activeCount++;
    this.segSum += power;
    this.segN++;
    if (this.activeCount >= this.activeLimit) {
      // Continuous noise or music: take it as the new ambient level.
      if (this.segN > 0) this.noisePower = this.segSum / this.segN;
      this.reset();
      return "end";
    }
    if (power < this.noisePower * FALL) {
      this.silentCount++;
      this.noisePower = this.noisePower * (1 - this.kSilent) + power * this.kSilent;
      if (this.silentCount >= this.silenceLimit) {
        this.isActive = false;
        this.silentCount = 0;
        this.activeCount = 0;
        return "end";
      }
    } else {
      this.silentCount = 0;
    }
    return "none";
  }
}

/** Mean-square power of a frame. */
export function power(frame: Float32Array): number {
  let s = 0;
  for (let i = 0; i < frame.length; i++) s += frame[i] * frame[i];
  return frame.length ? s / frame.length : 0;
}
