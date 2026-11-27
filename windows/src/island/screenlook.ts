// Round 4C items 2 and 6: the island's part in a look at the screen.
// - "screen-look-ask": Allow / Not now, when Settings → Screen is "Ask each time".
// - "screen-looking": an eye badge while the picture is taken and for 2 s after.
// Nothing here is built until the first look; the picture never comes to this page.

import { invoke } from "@tauri-apps/api/core";
import { IS_TAURI, onEvent } from "../core/bridge";
import { t } from "../i18n/i18n";
import { svg } from "../views/dom";

const EYE_PATH = "M2 12s3.6-6.5 10-6.5S22 12 22 12s-3.6 6.5-10 6.5S2 12 2 12zM12 10.5a1.5 1.5 0 1 0 0 3a1.5 1.5 0 1 0 0-3z";

const BASE =
  "position:fixed;top:6px;left:50%;transform:translateX(-50%);z-index:9999;display:flex;align-items:center;gap:8px;" +
  "padding:6px 10px;border-radius:999px;font:500 12px/1.2 system-ui,sans-serif;color:#eafff0;" +
  "background:rgba(20,28,24,.72);backdrop-filter:blur(14px);box-shadow:0 2px 12px rgba(0,0,0,.35);" +
  "transition:opacity .18s ease,transform .18s ease;";

let badge: HTMLElement | null = null;
let badgeTimer = 0;

/** The eye badge: lit while looking, gone 2 s after. */
export function showEye(on: boolean) {
  if (!badge) {
    badge = document.createElement("div");
    badge.className = "screen-eye";
    badge.setAttribute("role", "status");
    badge.style.cssText = BASE + "pointer-events:none;opacity:0;";
    const label = document.createElement("span");
    label.textContent = t("Looking at your screen");
    badge.replaceChildren(svg(EYE_PATH, 14, { stroke: 1.8 }), label);
    document.body.append(badge);
  }
  clearTimeout(badgeTimer);
  badge.style.opacity = "1";
  if (!on) {
    badgeTimer = window.setTimeout(() => {
      badge?.remove();
      badge = null;
    }, 2000);
  }
}

/** Allow / Not now for one look; no answer in 20 s counts as Not now (Rust times out). */
export function askToLook(id: number, answer = (allow: boolean) => invoke("screen_look_answer", { id, allow })) {
  const chip = document.createElement("div");
  chip.className = "screen-ask";
  chip.style.cssText = BASE;
  const words = document.createElement("span");
  words.replaceChildren(svg(EYE_PATH, 14, { stroke: 1.8 }));
  words.append(" " + t("Look at your screen?"));
  const button = (label: string, allow: boolean) => {
    const b = document.createElement("button");
    b.textContent = label;
    b.style.cssText = "border:0;border-radius:999px;padding:3px 10px;font:inherit;cursor:pointer;" + (allow ? "background:#3ddc84;color:#06210f;" : "background:rgba(255,255,255,.14);color:inherit;");
    b.onclick = () => {
      chip.remove();
      void Promise.resolve(answer(allow)).catch(() => {});
    };
    return b;
  };
  chip.append(words, button(t("Allow"), true), button(t("Not now"), false));
  document.body.append(chip);
  window.setTimeout(() => chip.remove(), 20_000);
  return chip;
}

export function registerScreenLook() {
  if (!IS_TAURI) return;
  void onEvent<{ id: number }>("screen-look-ask", (a) => askToLook(a.id));
  void onEvent<{ on: boolean }>("screen-looking", (e) => showEye(e.on));
}
