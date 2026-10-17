// Choosing Groovy's look (FIXES_ROUND3B item 4): one source of truth. A choice
// is applied at once where it was made, stamped with when it was made, and
// saved once after 300 ms of quiet; Rust keeps the newer of two stamps
// (settings::keep_newer_look), so a slower save never undoes a newer choice.

import { parseSkin, type Skin } from "../mochi/skins";

export interface LookChoice {
  groovyLook: string;
  lookAt: number;
}

/** Applies a look choice to `settings`, stamped later than any before it. */
export function chooseLook(settings: LookChoice, look: Skin, now = Date.now()): void {
  settings.groovyLook = look;
  settings.lookAt = Math.max(now, (settings.lookAt || 0) + 1);
}

/** The look to keep when two copies meet: the newer choice. */
export function newerLook(current: LookChoice, incoming: LookChoice): LookChoice {
  return (incoming.lookAt || 0) >= (current.lookAt || 0)
    ? { groovyLook: incoming.groovyLook, lookAt: incoming.lookAt || 0 }
    : { groovyLook: current.groovyLook, lookAt: current.lookAt || 0 };
}

/** The look on screen: a hover try-on, else the saved one. */
export function shownLook(preview: Skin | null, saved: string): Skin {
  return preview ?? parseSkin(saved);
}

/** Runs `save` once, `ms` after the last call to `schedule`. */
export class DebouncedSave {
  private timer: ReturnType<typeof setTimeout> | null = null;
  private readonly save: () => void;
  private readonly ms: number;

  constructor(save: () => void, ms = 300) {
    this.save = save;
    this.ms = ms;
  }

  schedule(): void {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = setTimeout(() => this.flush(), this.ms);
  }

  flush(): void {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
    this.save();
  }
}
