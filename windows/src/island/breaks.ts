// Break reminders: Groovy pops out to remind you to drink some water, stand up
// and stretch, or rest your eyes, each on its own timer (Settings → Breaks).
// "Done" starts that timer again; "In 10 min" asks again a little later.

import { State } from "../core/state";
import { N_, t } from "../i18n/i18n";

export type BreakKind = "water" | "stretch" | "eyes";

export const BREAKS: Record<BreakKind, { title: string; detail: string; setting: "breakWater" | "breakStretch" | "breakEyes" }> = {
  water: {
    title: N_("Time for some water."),
    detail: N_("You've been at it for a while. Have a glass of water."),
    setting: "breakWater",
  },
  stretch: {
    title: N_("Stand up and stretch."),
    detail: N_("Roll your shoulders, stretch your back, walk around for a minute."),
    setting: "breakStretch",
  },
  eyes: {
    title: N_("Rest your eyes."),
    detail: N_("Look at something far away for 20 seconds."),
    setting: "breakEyes",
  },
};

const SNOOZE_MIN = 10;

export interface BreakHost {
  /** Opens the island on the reminder. */
  show(): void;
}

/** When each break is next due (ms since epoch). Pure enough to test. */
export class BreakTimer {
  private due: Partial<Record<BreakKind, number>> = {};

  /** Minutes between reminders for `kind`; 0 is off. */
  private every(kind: BreakKind): number {
    const v = Number(State.settings[BREAKS[kind].setting]);
    return Number.isFinite(v) && v > 0 ? v : 0;
  }

  /** The break due now, if any. Timers start counting the first time they're seen. */
  check(now: number): BreakKind | null {
    for (const kind of Object.keys(BREAKS) as BreakKind[]) {
      const every = this.every(kind);
      if (!every) {
        delete this.due[kind];
        continue;
      }
      const due = this.due[kind];
      if (due == null || due - now > every * 60_000) {
        // New, or the interval was shortened in Settings.
        this.due[kind] = now + every * 60_000;
        continue;
      }
      if (now >= due) return kind;
    }
    return null;
  }

  done(kind: BreakKind, now: number) {
    const every = this.every(kind);
    if (every) this.due[kind] = now + every * 60_000;
  }

  snooze(kind: BreakKind, now: number) {
    this.due[kind] = now + SNOOZE_MIN * 60_000;
  }
}

/** The reminder loop in the island: checks twice a minute while Groovy isn't paused. */
export class Breaks {
  private timer = new BreakTimer();
  private host: BreakHost;

  constructor(host: BreakHost) {
    this.host = host;
    window.setInterval(() => this.tick(), 30_000);
  }

  private tick() {
    if (State.paused || State.breakDue) return;
    // Never on top of something that needs you (an approval, a question) or a call.
    if (State.pendingApproval || State.voiceCall || State.brainSetup) return;
    const kind = this.timer.check(Date.now());
    if (!kind) return;
    State.breakDue = kind;
    State.noteMessage = t(BREAKS[kind].title);
    this.host.show();
    State.notify();
  }

  done() {
    if (!State.breakDue) return;
    this.timer.done(State.breakDue, Date.now());
    this.clear();
  }

  snooze() {
    if (!State.breakDue) return;
    this.timer.snooze(State.breakDue, Date.now());
    this.clear();
  }

  private clear() {
    State.breakDue = null;
    State.notify();
  }
}

/** What the reminder card's buttons call; filled in by the island. */
export const BreakControl = {
  done: () => {},
  snooze: () => {},
};
