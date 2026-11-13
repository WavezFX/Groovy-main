// Settings left open (round 4A item 3): Rust asks before closing a window that
// sat unused; this says whether closing it now would lose something.

interface Focused {
  tagName: string;
  value?: unknown;
  isContentEditable?: boolean;
  textContent?: string | null;
}

/** A text field with words in it has the caret, or something is in progress. */
export function hubBusy(doc: { activeElement: unknown; querySelector(selector: string): unknown }): boolean {
  const at = doc.activeElement as Focused | null;
  const typing = at != null
    && ((typeof at.value === "string" && at.value.trim() !== "" && /^(INPUT|TEXTAREA)$/.test(at.tagName))
      || (at.isContentEditable === true && (at.textContent ?? "").trim() !== ""));
  return typing || doc.querySelector("progress, [aria-busy='true'], .busy, .downloading") != null;
}
