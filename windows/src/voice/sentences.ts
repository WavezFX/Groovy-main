// Speaking an answer while it is still being written (FIXES_ROUND2 item 4):
// each finished sentence of the streamed reply is read out as soon as it is
// complete, so a light local model starts talking after its first sentence.

/** The sentences completed in `text` from `from` on, and where the next one starts. */
export function finishedSentences(text: string, from: number): { sentences: string[]; next: number } {
  const sentences: string[] = [];
  let start = from;
  const end = /([.!?…]+["'”’)]*)(\s+)|\n+/g;
  end.lastIndex = from;
  for (let m = end.exec(text); m; m = end.exec(text)) {
    const after = text[m.index + m[0].length];
    // At the very end of the stream the next word may still be coming
    // ("e.g. " then "a walk"): wait for it, unless a line ended.
    if (after === undefined && !m[0].includes("\n")) break;
    // "e.g. a walk": a full stop followed by a small letter isn't a sentence end.
    if (m[1] && after && /\p{Ll}/u.test(after)) continue;
    const stop = m.index + (m[1]?.length ?? 0);
    const s = text.slice(start, stop).trim();
    // "e.g. " or "3. " alone isn't a sentence worth a pause.
    if (s.length >= 3) sentences.push(s);
    if (s.length >= 3 || !s) start = m.index + m[0].length;
  }
  return { sentences, next: start };
}

/**
 * Reads sentences one after another. `feed` takes the reply so far; `finish`
 * the whole reply, and resolves once everything has been said (or `stop`).
 */
export class SentenceSpeaker {
  private next = 0;
  private chain: Promise<void> = Promise.resolve();
  private stopped = false;
  private started = false;
  private sayOne: (sentence: string) => Promise<string | null>;
  private onFirst: () => void;
  /** Why speaking failed, if it did (no voice installed…). */
  failure: string | null = null;

  constructor(sayOne: (sentence: string) => Promise<string | null>, onFirst: () => void) {
    this.sayOne = sayOne;
    this.onFirst = onFirst;
  }

  get speaking(): boolean {
    return this.started && !this.stopped;
  }

  feed(soFar: string) {
    if (this.stopped) return;
    const { sentences, next } = finishedSentences(soFar, this.next);
    this.next = next;
    for (const s of sentences) this.queue(s);
  }

  finish(full: string): Promise<void> {
    if (!this.stopped) {
      // The final text can differ from the stream (a note in front of it):
      // nothing said yet → say it all; else the rest after what was said.
      const rest = this.started ? full.slice(Math.min(this.next, full.length)) : full;
      this.next = full.length;
      for (const s of finishedSentences(`${rest}\n`, 0).sentences) this.queue(s);
    }
    return this.chain;
  }

  stop() {
    this.stopped = true;
  }

  private queue(sentence: string) {
    if (!this.started) {
      this.started = true;
      this.onFirst();
    }
    this.chain = this.chain.then(async () => {
      if (this.stopped || this.failure) return;
      const why = await this.sayOne(sentence);
      if (why) this.failure = why;
    });
  }
}
