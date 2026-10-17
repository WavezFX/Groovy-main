// Small pieces every settings section draws with.

import { h } from "../views/dom";

export function statusDot(ok: boolean): HTMLElement {
  return h("i", { class: "dot", style: `background:${ok ? "#22c55e" : "#f4505e"}` });
}

export function renderDiff(text: string): HTMLElement {
  const box = h("div", { class: "diff" });
  for (const line of text.split("\n")) {
    const cls = line.startsWith("+") ? "add" : line.startsWith("-") ? "del" : "ctx";
    box.append(h("div", { class: cls, text: line }));
  }
  return box;
}

/**
 * A button that needs two clicks: the first one changes its label to `ask`
 * for a few seconds, the second one within that time runs `action`. Used for
 * anything that deletes, instead of a dialog.
 */
export function confirmClick(button: HTMLButtonElement, ask: string, action: () => void) {
  let armed: ReturnType<typeof setTimeout> | null = null;
  const label = button.textContent ?? "";
  button.addEventListener("click", () => {
    if (armed) {
      clearTimeout(armed);
      armed = null;
      button.textContent = label;
      action();
      return;
    }
    button.textContent = ask;
    armed = setTimeout(() => {
      armed = null;
      button.textContent = label;
    }, 4000);
  });
}
