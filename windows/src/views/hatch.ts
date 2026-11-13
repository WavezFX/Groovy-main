// Choose your Groovy and name him (round 5 item 6), shown once, right after
// the very first Seed intro hatches him. Three hatchlings side by side, each
// with a tiny preview of what he grows into: tap one, the other two fade and
// he hops into place; then a glass name field, "What will you call him?",
// filled with "Groovy". Animated with the island's motion and still when
// Windows' animations are off (style.css).

import { h } from "./dom";
import type { ViewActions, ViewHost } from "./views";
import { lookSprite, spriteCanvas } from "../mochi/sprites";
import { GROWS_INTO, SKINS, SKIN_KEYS, type Skin } from "../mochi/skins";
import { DEFAULT_NAME, MAX_NAME, checkName } from "../mochi/naming";
import { N_, t, tl } from "../i18n/i18n";

/** One line on each: what he grows into. */
export const HATCH_LINES: Record<Skin, string> = {
  night: N_("Calm and green. Grows into Grove, a little bonsai."),
  ember: N_("Warm and bright. Grows into Blaze, a golden flame."),
  frost: N_("Cool and clear. Grows into Tide, a sea of his own."),
};

const PREVIEW = 54;

export function buildHatch(actions: ViewActions): ViewHost {
  let chosen: Skin | null = null;
  const dpr = () => Math.min(2, window.devicePixelRatio || 1);
  const title = h("div", { class: "hatch-title", text: tl("Who hatched? Pick your Groovy.") });
  const row = h("div", { class: "hatch-row", role: "radiogroup", "aria-label": tl("Look") });
  const buttons = new Map<Skin, HTMLButtonElement>();
  for (const look of SKINS) {
    const b = h("button", { class: "hatch-pick", type: "button", role: "radio", "aria-checked": "false" },
      h("span", { class: "hatch-art" }),
      h("b", { text: tl(SKIN_KEYS[look]) }),
      h("span", { class: "hatch-line", text: tl(HATCH_LINES[look]) })) as HTMLButtonElement;
    b.title = `${t(SKIN_KEYS[look])} → ${t(GROWS_INTO[look])}`;
    b.addEventListener("click", () => pick(look));
    buttons.set(look, b);
    row.append(b);
  }

  const input = h("input", { class: "hatch-name", type: "text", maxlength: MAX_NAME * 2, "aria-label": tl("What will you call him?"), placeholder: DEFAULT_NAME }) as HTMLInputElement;
  input.value = DEFAULT_NAME;
  const why = h("span", { class: "hatch-why", role: "alert" });
  const done = h("button", { class: "hatch-done", type: "button", text: tl("That's him") }) as HTMLButtonElement;
  const hello = h("div", { class: "hatch-hello", text: tl("Hello, little one.") });
  const naming = h("div", { class: "hatch-naming" },
    hello, h("label", { class: "hatch-ask", text: tl("What will you call him?") }),
    h("div", { class: "hatch-field" }, input, done), why);
  naming.hidden = true;

  const finish = () => {
    if (!chosen) return;
    const check = checkName(input.value);
    if (!check.ok) {
      why.textContent = t(check.why);
      input.focus();
      return;
    }
    actions.hatch(chosen, check.name);
  };
  done.addEventListener("click", finish);
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") finish();
  });
  input.addEventListener("input", () => {
    why.textContent = "";
  });

  function pick(look: Skin) {
    if (chosen) return;
    chosen = look;
    actions.previewLook(look);
    for (const [l, b] of buttons) {
      b.classList.add(l === look ? "chosen" : "gone");
      b.setAttribute("aria-checked", String(l === look));
      b.disabled = l !== look;
    }
    // The other two fade; then he takes his place beside the name field.
    setTimeout(() => {
      el.classList.add("naming-step");
      naming.hidden = false;
      input.focus();
      input.select();
    }, 300);
  }

  const el: HTMLElement = h("div", { class: "view hatch" }, title, row, naming);
  let drawn = false;
  return {
    el,
    sync() {
      // The previews are drawn once, at stage 5: what each one grows into.
      if (drawn) return;
      drawn = true;
      for (const [look, b] of buttons) b.querySelector(".hatch-art")!.replaceChildren(spriteCanvas(lookSprite(look, 5, PREVIEW, dpr()), PREVIEW));
    },
    focus() {
      if (chosen) input.focus();
    },
  };
}
