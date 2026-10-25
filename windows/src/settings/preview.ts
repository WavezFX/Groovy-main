// Round 3E item 11: a stage, a look or a locked item tried for 5 seconds in
// Settings → Groovy's big preview, then back to him as he is. Never saved,
// never shown on the island: it changes the preview engine only, and the
// stage he has reached (and the look he wears) stay as they are.

import type { Skin } from "../mochi/skins";
import type { Accessory } from "../mochi/wardrobe";

export const PREVIEW_MS = 5000;

export interface Preview {
  look?: Skin;
  stage?: number;
  item?: Accessory;
  /** Something he hasn't earned: the preview says "Preview only". */
  locked?: boolean;
}

export class Previewer {
  current: Preview | null = null;
  private timer: ReturnType<typeof setTimeout> | null = null;

  private readonly changed: () => void;
  private readonly after: (fn: () => void, ms: number) => ReturnType<typeof setTimeout>;
  private readonly cancel: (t: ReturnType<typeof setTimeout>) => void;

  constructor(
    changed: () => void,
    after: (fn: () => void, ms: number) => ReturnType<typeof setTimeout> = (fn, ms) => setTimeout(fn, ms),
    cancel: (t: ReturnType<typeof setTimeout>) => void = (t) => clearTimeout(t),
  ) {
    this.changed = changed;
    this.after = after;
    this.cancel = cancel;
  }

  /** Shows `p` for five seconds; another preview replaces it and starts the five seconds again. */
  show(p: Preview) {
    this.clearTimer();
    this.current = p;
    this.changed();
    this.timer = this.after(() => this.stop(), PREVIEW_MS);
  }

  /** Back to him as he is. */
  stop() {
    this.clearTimer();
    if (!this.current) return;
    this.current = null;
    this.changed();
  }

  private clearTimer() {
    if (this.timer !== null) this.cancel(this.timer);
    this.timer = null;
  }
}
