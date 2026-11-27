// Round 4C item 3: the chat's "Look at my screen" button. One click takes one
// picture (Rust keeps it in memory for the next message only); a thumbnail with
// a remove button shows what will be sent. The picture is dropped after the reply.

import { invoke } from "@tauri-apps/api/core";
import { h, clear, svg } from "./dom";
import { N_, t, tl } from "../i18n/i18n";
import { State } from "../core/state";

const EYE_PATH = "M2 12s3.6-6.5 10-6.5S22 12 22 12s-3.6 6.5-10 6.5S2 12 2 12zM12 10.5a1.5 1.5 0 1 0 0 3a1.5 1.5 0 1 0 0-3z";

const STRINGS = {
  look: N_("Look at my screen"),
  remove: N_("Don't send the picture"),
};

let pending = false;
/** The look row of the chat on screen, and a circled picture waiting for it. */
let currentRow: HTMLElement | null = null;
let currentChange: () => void = () => {};
let waitingThumb: string | null = null;

/** Whether a picture waits for the next message (Rust sends it and forgets it). */
export function lookPending(): boolean {
  return pending;
}

/** The message went: Rust used the picture; the thumbnail goes. */
export function lookSent(row: HTMLElement) {
  pending = false;
  clear(row);
}

/** The thumbnail with its remove button. */
function showThumb(row: HTMLElement, dataUrl: string, onChange: () => void) {
  pending = true;
  clear(row);
  const img = h("img", { class: "look-thumb", alt: t(STRINGS.look) }) as HTMLImageElement;
  img.src = dataUrl;
  img.style.cssText = "height:36px;border-radius:6px;display:block;";
  const remove = h("button", { class: "look-remove", title: tl(STRINGS.remove), text: "×" });
  remove.style.cssText = "border:0;background:rgba(0,0,0,.5);color:#fff;border-radius:999px;width:18px;height:18px;cursor:pointer;";
  remove.addEventListener("click", () => {
    pending = false;
    clear(row);
    void invoke("screen_look_forget").catch(() => {});
    onChange();
  });
  const chip = h("div", { class: "look-chip" }, img, remove);
  chip.style.cssText = "display:flex;align-items:center;gap:6px;padding:4px 0;";
  row.append(chip);
  onChange();
}

/**
 * Round 4C item 5: the circle-and-ask shortcut. The user circles something on
 * screen (Rust), and the picture waits for the next message, its thumbnail in
 * the chat. False when cancelled or refused (the island says why).
 */
export async function circled(): Promise<boolean> {
  try {
    const shot = await invoke<{ dataUrl: string }>("screen_circle");
    if (currentRow?.isConnected) showThumb(currentRow, shot.dataUrl, currentChange);
    else {
      waitingThumb = shot.dataUrl;
      pending = true;
    }
    return true;
  } catch (err) {
    const why = String(err).replace(/^Error:\s*/, "");
    if (why !== "Nothing circled.") {
      State.noteMessage = why;
      State.view = "note";
      State.notify();
    }
    return false;
  }
}

/** The button, and the row its thumbnail shows in. */
export function lookButton(row: HTMLElement, onChange: () => void): HTMLElement {
  currentRow = row;
  currentChange = onChange;
  if (waitingThumb) {
    const url = waitingThumb;
    waitingThumb = null;
    queueMicrotask(() => showThumb(row, url, onChange));
  }
  const btn = h("button", { class: "call-btn look-btn", title: tl(STRINGS.look) });
  btn.append(svg(EYE_PATH, 12, { stroke: 2 }));
  btn.addEventListener("click", async () => {
    if (btn.classList.contains("busy")) return;
    btn.classList.add("busy");
    try {
      const shot = await invoke<{ dataUrl: string; width: number; height: number }>("screen_look", { target: "screen" });
      showThumb(row, shot.dataUrl, onChange);
    } catch (err) {
      State.noteMessage = String(err).replace(/^Error:\s*/, "");
      State.view = "note";
      State.notify();
    } finally {
      btn.classList.remove("busy");
      onChange();
    }
  });
  return btn;
}
