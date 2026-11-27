// Round 7 item 1: Instagram's new DMs, as Windows' notifications told Rust
// (src-tauri/src/instagram/). The newest five, in memory only; nothing here
// runs while Instagram is off (Rust sends nothing then).

/** A new DM (instagram/notes.rs `Dm`). */
export interface InstagramDm {
  id: number;
  sender: string;
  username: string | null;
  preview: string;
  source: "app" | "web";
  appId: string;
}

export const INSTAGRAM_COLOR = "#E1306C";

export const Instagram = {
  dms: [] as InstagramDm[],
  push(d: InstagramDm) {
    this.dms = [d, ...this.dms.filter((x) => x.id !== d.id)].slice(0, 5);
  },
  clear() {
    this.dms = [];
  },
};

/** Settings → General → Instagram (settings.json's `instagram`). */
export interface InstagramPrefs {
  on: boolean;
  openIn: "auto" | "app" | "edge" | "chrome" | "browser";
  typeForMe: boolean;
}

export const DEFAULT_INSTAGRAM: InstagramPrefs = { on: false, openIn: "auto", typeForMe: false };

/** What the tile says: the newest sender and preview, or how to start. */
export function instagramFace(dms: readonly InstagramDm[], on: boolean, t: (s: string) => string): { title: string; sub: string } {
  const d = dms[0];
  if (!on) return { title: "Instagram", sub: t("Turn on Instagram in Settings") };
  if (!d) return { title: "Instagram", sub: t("New messages pop up here") };
  return { title: d.sender, sub: d.preview || t("Sent you a message") };
}
