// Wake words — pure functions, ported from the Mac's WakePhrase.swift and
// made Groovy's: "Groovy", "Hey Groovy" and "Yo Groovy".
//
// The speech-to-text hears the name in many spellings ("groovie", "groovey",
// "gruvy"…), so every word is normalised before matching. "Hey Groovy" and
// "Yo Groovy" count anywhere in what was heard; a bare "Groovy" only counts as
// the first word, so a sentence that merely mentions the name doesn't wake him.
// What follows the last wake phrase is the command, in its original spelling.

export interface WakeSplit {
  matched: boolean;
  /** What was said after the wake phrase ("" when nothing followed). */
  command: string;
}

const NAME = "groovy";
const GREETINGS = new Set(["hey", "yo", "hi", "ok", "okay", "oi"]);
/** How the speech-to-text tends to spell the name. */
const NAME_VARIANTS = new Set([
  "groovy", "groovie", "groovey", "groovee", "groovi", "gruvy", "gruvi", "grooby", "groovys", "groovy's",
  "groovies", "grooving", "groovyy", "grovy", "grooovy",
]);

/** Lower case, accents and punctuation removed (apostrophes kept inside words). */
export function normalise(s: string): string {
  return s
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .replace(/[^\p{L}\p{N}'\s]/gu, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function normWord(w: string): string {
  const n = normalise(w).replace(/^'+|'+$/g, "");
  if (NAME_VARIANTS.has(n)) return NAME;
  if (n === "okay") return "ok";
  return n;
}

export function splitWake(raw: string): WakeSplit {
  const original = raw.split(/\s+/).filter(Boolean);
  if (original.length === 0) return { matched: false, command: "" };
  // "Hey, Groovy" may arrive as one token "hey,groovy" — rare; punctuation inside
  // a token is kept with the original and split here for matching only.
  const words = original.map(normWord);

  let end = -1; // index right after the last wake phrase
  for (let i = 0; i < words.length; i++) {
    if (words[i] !== NAME) continue;
    const greeted = i > 0 && GREETINGS.has(words[i - 1]);
    // "Groovy" alone only counts at the start (a filler like "um" before it is fine).
    const leading = words.slice(0, i).every((w) => w === "" || FILLERS.has(w));
    if (greeted || leading) end = i + 1;
  }
  if (end < 0) return { matched: false, command: "" };

  const tail = original.slice(end);
  if (tail.length === 0) return { matched: true, command: "" };
  tail[0] = tail[0].replace(/^[^\p{L}\p{N}]+/u, "");
  return { matched: true, command: tail.filter(Boolean).join(" ").trim() };
}

const FILLERS = new Set(["um", "uh", "erm", "er", "hmm", "so", "oh", "ah", "and"]);

/** Phrases that end a voice conversation instead of being sent. */
export const END_PHRASES = new Set([
  "stop", "cancel", "never mind", "nevermind", "that's all", "thats all", "thanks that's all",
  "bye", "goodbye", "nothing", "forget it",
]);

export function isEndPhrase(text: string): boolean {
  return END_PHRASES.has(normalise(text).replace(/[.!?]+$/, ""));
}
