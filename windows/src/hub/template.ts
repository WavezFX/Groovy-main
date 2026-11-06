// Round 3D item 3: the Groovy pages' markup (section › h2 › .row › .hint) laid
// out as the page template (ui.ts). Each row gets its one-line description,
// longer help moves behind an ⓘ or into "Learn more", every control gets a
// screen-reader label, and the rows a page calls advanced move into one
// Advanced expander at the bottom. Nodes are only moved, never replaced, so
// the code that updates them later (a status line, a redraw) still finds them.

import { t } from "../i18n/i18n";
import { h } from "../views/dom";

/** Help up to this long is the row's description; longer goes behind ⓘ. */
export const SHORT_HELP = 110;

/** Help worth moving: plain text with something in it (a status line or a warning stays where it is). */
function isHelp(el: Element): el is HTMLElement {
  if (!el.classList.contains("hint") || el.tagName !== "DIV") return false;
  if (el.classList.contains("warn") || el.classList.contains("full")) return false;
  if (el.querySelector("button, a, input, select")) return false;
  return (el.textContent ?? "").trim().length > 0;
}

/** The row's left side: its label, then room for a description. */
function textOf(row: HTMLElement): HTMLElement | null {
  const done = row.querySelector<HTMLElement>(":scope > .hub-t-text");
  if (done) return done;
  const label = row.firstElementChild;
  if (!label || label.tagName !== "LABEL") return null;
  const text = h("div", { class: "hub-t-text" }, h("div", { class: "hub-t-name" }));
  row.insertBefore(text, label);
  text.firstElementChild!.append(label);
  return text;
}

/** Help behind an ⓘ: the help node itself is the tip, so it stays live. */
function tip(help: HTMLElement, name: string): HTMLElement {
  help.classList.add("hub-t-tip");
  help.setAttribute("role", "tooltip");
  return h("span", { class: "hub-t-info", tabindex: "0", role: "button", "aria-label": t("About {name}", { name }) }, h("span", { "aria-hidden": "true", text: "ⓘ" }), help);
}

function attach(row: HTMLElement, help: HTMLElement): boolean {
  const text = textOf(row);
  if (!text) return false;
  const name = (text.textContent ?? "").trim();
  if ((help.textContent ?? "").trim().length <= SHORT_HELP && !text.querySelector(".hub-t-detail")) {
    help.classList.add("hub-t-detail");
    text.append(help);
  } else {
    text.firstElementChild!.append(tip(help, name));
  }
  return true;
}

/** A label for every control a row holds that has no name of its own. */
function label(row: HTMLElement) {
  const name = (row.querySelector(":scope > .hub-t-text label, :scope > label")?.textContent ?? "").trim();
  if (!name) return;
  for (const c of row.querySelectorAll<HTMLElement>("input, select, textarea, button")) {
    if (c.getAttribute("aria-label") || c.id) continue;
    if (c.tagName === "BUTTON" && (c.textContent ?? "").trim()) continue;
    c.setAttribute("aria-label", name);
  }
}

export function tidySection(section: HTMLElement) {
  section.classList.add("hub-t-legacy");
  let more: HTMLElement | null = null;
  for (const el of [...section.querySelectorAll<HTMLElement>(".hint")]) {
    if (!isHelp(el)) continue;
    const prev = el.previousElementSibling;
    if (prev?.classList.contains("row") && attach(prev as HTMLElement, el)) continue;
    if ((el.textContent ?? "").trim().length <= SHORT_HELP) continue;
    // Longer help that belongs to no row: one "Learn more" at the card's end.
    if (el.parentElement !== section) continue;
    if (!more) more = h("details", { class: "hub-t-more" }, h("summary", { text: t("Learn more") }));
    more.append(el);
  }
  if (more) section.append(more);
  for (const row of section.querySelectorAll<HTMLElement>(".row")) label(row);
}

/**
 * Lay out a Groovy page. `advanced` names rows (their label) or whole
 * sections (their title) that go into the Advanced expander.
 */
export function templatize(root: HTMLElement, advanced: string[] = []) {
  // Sections already built to the design (data-own) are left as they are.
  for (const s of root.querySelectorAll<HTMLElement>("section")) if (!s.closest("[data-own]")) tidySection(s);
  if (!advanced.length) return;
  const wanted = new Set(advanced.map((a) => t(a)));
  const box = h("section", { class: "hub-t-legacy" });
  const sections: HTMLElement[] = [];
  for (const s of root.querySelectorAll<HTMLElement>(":scope > section")) {
    const title = s.querySelector(":scope > h2")?.textContent?.trim() ?? "";
    if (wanted.has(title)) sections.push(s);
  }
  for (const row of root.querySelectorAll<HTMLElement>(".row")) {
    if (sections.some((s) => s.contains(row))) continue;
    const name = row.querySelector(":scope > .hub-t-text label, :scope > label")?.textContent?.trim() ?? "";
    if (wanted.has(name)) box.append(row);
  }
  if (!box.children.length && !sections.length) return;
  const details = h("details", { class: "hub-t-advanced" }, h("summary", { text: t("Advanced") }));
  if (box.children.length) details.append(box);
  details.append(...sections);
  root.append(details);
}
