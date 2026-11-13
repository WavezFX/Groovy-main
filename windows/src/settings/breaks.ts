// Settings → Breaks: how often Groovy reminds you to drink water, stretch and
// rest your eyes.

import type { Settings } from "../core/state";
import { h } from "../views/dom";
import { t } from "../i18n/i18n";

type Key = "breakWater" | "breakStretch" | "breakEyes";

const CHOICES: Record<Key, number[]> = {
  breakWater: [0, 30, 45, 60, 90, 120],
  breakStretch: [0, 30, 45, 60, 90, 120],
  breakEyes: [0, 20, 30, 45, 60],
};

export function breaksSection(settings: Settings, save: () => void): HTMLElement {
  const row = (key: Key, label: string) => {
    const select = h("select", {}) as HTMLSelectElement;
    for (const m of CHOICES[key]) {
      select.append(h("option", { value: String(m), text: m === 0 ? t("Off") : t("Every {0} min", { 0: m }) }));
    }
    const current = settings[key];
    if (!CHOICES[key].includes(current)) select.append(h("option", { value: String(current), text: t("Every {0} min", { 0: current }) }));
    select.value = String(current);
    select.addEventListener("change", () => {
      settings[key] = Number(select.value) || 0;
      save();
    });
    return h("div", { class: "row" }, h("label", { text: label }), select);
  };
  return h(
    "section",
    {},
    h("h2", {}, h("span", { text: t("Breaks") })),
    row("breakWater", t("Drink water")),
    row("breakStretch", t("Stand up and stretch")),
    row("breakEyes", t("Rest your eyes")),
    h("div", {
      class: "hint",
      text: t("Groovy pops out to remind you, and says it out loud when voice is on. “Done” starts the timer again; “In 10 min” asks again a little later."),
    }),
  );
}
