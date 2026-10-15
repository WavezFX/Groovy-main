// Groovy's own voice: answers read aloud with the voices built into Windows
// (Settings → Time & language → Speech). The ones marked "offline" run on this
// computer; Edge's "Online (Natural)" voices need the internet, so they are only
// used when picked by name.

/** Markdown and code turned into something worth hearing. */
export function speakable(markdown: string, max = 700): string {
  let s = markdown
    .replace(/```[\s\S]*?```/g, " (code in the chat) ")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/!\[[^\]]*\]\([^)]*\)/g, "")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/^#{1,6}\s+/gm, "")
    .replace(/^\s*[-*+]\s+/gm, "")
    .replace(/^\s*\d+\.\s+/gm, "")
    .replace(/[*_~>|]+/g, "")
    .replace(/https?:\/\/\S+/g, "the link in the chat")
    .replace(/\s+/g, " ")
    .trim();
  if (s.length > max) {
    // Stop at the end of a sentence; the rest stays on screen.
    const cut = s.slice(0, max);
    const end = Math.max(cut.lastIndexOf(". "), cut.lastIndexOf("! "), cut.lastIndexOf("? "));
    s = (end > max * 0.4 ? cut.slice(0, end + 1) : cut) + " The rest is in the chat.";
  }
  return s;
}

function synth(): SpeechSynthesis | null {
  return typeof speechSynthesis === "undefined" ? null : speechSynthesis;
}

/** The voices on this computer, offline ones first. */
export async function voices(): Promise<SpeechSynthesisVoice[]> {
  const s = synth();
  if (!s) return [];
  let list = s.getVoices();
  if (list.length === 0) {
    // Chromium fills the list a moment after start.
    await new Promise<void>((resolve) => {
      const done = () => resolve();
      s.addEventListener("voiceschanged", done, { once: true });
      setTimeout(done, 1500);
    });
    list = s.getVoices();
  }
  return [...list].sort((a, b) => Number(b.localService) - Number(a.localService));
}

/** The voice named in Settings, or the best offline English one. */
export function pickVoice(list: SpeechSynthesisVoice[], wanted: string): SpeechSynthesisVoice | null {
  if (wanted) {
    const named = list.find((v) => v.name === wanted);
    if (named) return named;
  }
  const offline = list.filter((v) => v.localService);
  return (
    offline.find((v) => v.lang === "en-GB") ??
    offline.find((v) => v.lang.startsWith("en")) ??
    offline[0] ??
    list.find((v) => v.lang.startsWith("en")) ??
    list[0] ??
    null
  );
}

let current: SpeechSynthesisUtterance | null = null;

/** Says `text`; resolves when it is done (or was stopped). */
export async function speak(text: string, voiceName = "", rate = 1): Promise<void> {
  const s = synth();
  const words = speakable(text);
  if (!s || !words) return;
  stopSpeaking();
  const voice = pickVoice(await voices(), voiceName);
  await new Promise<void>((resolve) => {
    const u = new SpeechSynthesisUtterance(words);
    if (voice) {
      u.voice = voice;
      u.lang = voice.lang;
    }
    u.rate = rate;
    u.onend = () => resolve();
    u.onerror = () => resolve();
    current = u;
    s.speak(u);
    // A voice that never reports its end must not hold the call forever.
    setTimeout(resolve, 2000 + words.length * 120);
  });
  current = null;
}

export function stopSpeaking() {
  const s = synth();
  if (s && (s.speaking || s.pending || current)) s.cancel();
  current = null;
}

export function isSpeaking(): boolean {
  return !!synth()?.speaking;
}
