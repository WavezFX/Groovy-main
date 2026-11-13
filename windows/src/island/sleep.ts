// The island sleeps at idle (round 4F item 1): when he has been resting for a
// while with nothing going on, the page asks Rust to put it to sleep
// (island_sleep.rs: the webview is suspended and its memory handed back, a
// picture of the island standing in). Rust wakes it for the cursor or for
// anything it has to show, then sends `island-woke`.
//
// Nothing here runs while the island is busy: the only timer is the one
// counting the quiet time, started when his frame loop stops.

/** How long he rests, with nothing going on, before the island sleeps. */
export const QUIET_MS = 20_000;
/** After Rust said "not now", the next tries wait longer each time, up to this. */
export const LONGEST_WAIT_MS = 5 * 60_000;

/** What the island is doing, as far as sleeping goes. */
export interface Quiet {
  /** The island is open (a panel, the chat, quick settings). */
  expanded: boolean;
  /** His frame loop is running: something is moving. */
  moving: boolean;
  /** A voice call, a question being listened to or answered, dictation, a recording. */
  talking: boolean;
  /** A card is waiting for the user: an approval, a reminder, a break, a notice, the brain's setup. */
  asking: boolean;
  /** The music or recording strip is on show. */
  strip: boolean;
  /** He is out on the desktop, or a file is being dragged over the island. */
  away: boolean;
}

/** Whether the island may ask to sleep. */
export function canSleep(q: Quiet): boolean {
  return !(q.expanded || q.moving || q.talking || q.asking || q.strip || q.away);
}

export class IslandSleep {
  private timer: number | null = null;
  private wait: number;
  private readonly quiet: () => Quiet;
  private readonly ask: () => Promise<boolean>;
  private readonly delay: number;

  constructor(quiet: () => Quiet, ask: () => Promise<boolean>, delay = QUIET_MS) {
    this.quiet = quiet;
    this.ask = ask;
    this.delay = delay;
    this.wait = delay;
  }

  /** His frame loop stopped: start counting the quiet time. */
  rest() {
    if (this.timer != null) return;
    this.timer = window.setTimeout(() => {
      this.timer = null;
      void this.tryToSleep();
    }, this.wait);
  }

  /** Something happened: he isn't resting. */
  stir() {
    this.wait = this.delay;
    if (this.timer == null) return;
    window.clearTimeout(this.timer);
    this.timer = null;
  }

  private async tryToSleep() {
    if (!canSleep(this.quiet())) return; // whatever keeps him up calls rest() again when it ends
    let slept = false;
    try {
      slept = await this.ask();
    } catch {
      slept = false;
    }
    // Asleep: the page is suspended here and carries on at the wake. Not now
    // (the cursor is near, a conversation, another system): try again later,
    // less and less often.
    if (!slept) {
      this.wait = Math.min(this.wait * 2, LONGEST_WAIT_MS);
      if (canSleep(this.quiet())) this.rest();
    } else {
      this.wait = this.delay;
    }
  }
}
