// Round 4C item 3: the chat's "Look at my screen" button. One click takes one
// picture (Rust keeps it in memory for the next message only); a thumbnail with
// a remove button shows what will be sent. The picture is dropped after the reply.

import { invoke } from "@tauri-apps/api/core";
import { h, clear } from "./dom";
import { N_, t, tl } from "../i18n/i18n";
import { State } from "../core/state";

const EYE =
  '<svg viewBox="0 0 24 24" width="12" height="12" aria-hidden="true"><path d="M2 12s3.6-6.5 10-6.5S22 12 22 12s-3.6 6.5-10 6.5S2 12 2 12z" fill="none" stroke="currentColor" stroke-width="2"/><circle cx="12" cy="12" r="3" fill="currentColor"/></svg>';

const STRINGS = {
  look: N_("Look at my screen"),
  remove: N_("Don't send the picture"),
};

let pending = false;

/** Whether a picture waits for the next message (Rust sends it and forgets it). */
export function lookPending(): boolean {
  return pending;
}

/** The message went: Rust used the picture; the thumbnail goes. */
export function lookSent(row: HTMLElement) {
  pending = false;
  clear(row);
}

/** The button, and the row its thumbnail shows in. */
export function lookButton(row: HTMLElement, onChange: () => void): HTMLElement {
  const btn = h("button", { class: "call-btn look-btn", title: tl(STRINGS.look) });
  btn.innerHTML = EYE;
  btn.addEventListener("click", async () => {
    if (btn.classList.contains("busy")) return;
    btn.classList.add("busy");
    try {
      const shot = await invoke<{ dataUrl: string; width: number; height: number }>("screen_look", { target: "screen" });
      pending = true;
      clear(row);
      const img = h("img", { class: "look-thumb", alt: t(STRINGS.look) }) as HTMLImageElement;
      img.src = shot.dataUrl;
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
