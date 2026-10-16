// Wardrobe view — opened with a right-click on Groovy or from the tray menu.
// His looks, then Head, Face and Neck (FIXES_ROUND3 item 5): each slot on its
// own, with its own None, and Remove all. Resting the pointer on an item tries
// it on in its slot; a click wears it (or takes it off). Nothing is greyed out.

import { h } from "./dom";
import { State } from "../core/state";
import { drawWardrobeIcon } from "../mochi/outfits";
import { ACCESSORIES, OUTFIT_KEYS, type Accessory } from "../mochi/wardrobe";
import { SLOTS, SLOT_OF, currentWardrobe, worn, type Slot } from "../mochi/appearance";
import type { ViewActions, ViewHost } from "./views";
import { BotEngine } from "../mochi/engine";
import { GROWS_INTO, SKINS, SKIN_KEYS, parseSkin, type Skin } from "../mochi/skins";
import { parseGrowth, progressLine, showStage, shownStage } from "../mochi/growth";
import { Bridge } from "../core/bridge";
import { N_, language, t, tl } from "../i18n/i18n";

const ICON = 26;

export const SLOT_TITLES: Record<Slot, string> = { head: N_("Head"), face: N_("Face"), neck: N_("Neck") };

function icon(item: Accessory | "none"): HTMLCanvasElement {
  const canvas = h("canvas") as HTMLCanvasElement;
  const dpr = Math.min(2, window.devicePixelRatio || 1);
  canvas.width = Math.round(ICON * dpr);
  canvas.height = Math.round(ICON * dpr);
  const ctx = canvas.getContext("2d");
  if (ctx) {
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    drawWardrobeIcon(ctx, ICON, item);
  }
  return canvas;
}

export function buildWardrobe(actions: ViewActions): ViewHost {
  const looks = h("div", { class: "wardrobe-grid wardrobe-looks" });
  const rows = h("div", { class: "wardrobe-slots" });
  // How far he has grown (round 3 item 6), and which stage reached to show.
  const progress = h("span", { class: "wardrobe-progress" });
  const stagePick = h("select", { class: "wardrobe-stage", title: tl("Show stage"), "aria-label": tl("Show stage") }) as HTMLSelectElement;
  stagePick.addEventListener("change", () => {
    const g = showStage(parseGrowth(State.settings.growth), Number(stagePick.value));
    State.settings.growth = g;
    void Bridge.saveGrowth(g);
    State.notify();
  });
  const removeAll = h("button", { class: "wardrobe-clear", text: tl("Remove all"), onclick: () => actions.dress("all") });
  const el = h(
    "div",
    { class: "view wardrobe" },
    h(
      "div",
      { class: "card" },
      h(
        "div",
        { class: "stack wardrobe-stack" },
        h("div", { class: "wardrobe-head" }, h("span", { class: "wardrobe-title", text: tl("Look") }), progress, stagePick),
        looks,
        h("div", { class: "wardrobe-head" }, h("span", { class: "wardrobe-title", text: tl("Wardrobe") }), removeAll),
        rows,
      ),
    ),
  );

  const items = new Map<Accessory, HTMLButtonElement>();
  const nones = new Map<Slot, HTMLButtonElement>();
  for (const slot of SLOTS) {
    const row = h("div", { class: "wardrobe-grid wardrobe-slot" }, h("span", { class: "wardrobe-slot-name", text: tl(SLOT_TITLES[slot]) }));
    const none = h("button", { class: "wardrobe-item", title: tl("None"), "aria-label": tl("None") }, icon("none")) as HTMLButtonElement;
    none.addEventListener("click", () => actions.dress(slot));
    nones.set(slot, none);
    row.append(none);
    for (const item of ACCESSORIES.filter((a) => SLOT_OF[a] === slot)) {
      const button = h("button", { class: "wardrobe-item", title: tl(OUTFIT_KEYS[item]), "aria-label": tl(OUTFIT_KEYS[item]) }, icon(item)) as HTMLButtonElement;
      button.addEventListener("mouseenter", () => actions.previewOutfit(item));
      button.addEventListener("mouseleave", () => actions.previewOutfit(null));
      button.addEventListener("click", () => actions.dress(item));
      items.set(item, button);
      row.append(button);
    }
    rows.append(row);
  }

  // Looks: a small still portrait of each, drawn once. Hover tries it on, click keeps it.
  const lookItems = new Map<Skin, HTMLButtonElement>();
  let looksDrawn = -1;
  for (const look of SKINS) {
    const canvas = h("canvas");
    const button = h(
      "button",
      { class: "wardrobe-item wardrobe-look", title: tl(SKIN_KEYS[look]), "aria-label": tl(SKIN_KEYS[look]) },
      canvas,
    );
    button.addEventListener("mouseenter", () => actions.previewLook(look));
    button.addEventListener("mouseleave", () => actions.previewLook(null));
    button.addEventListener("click", () => actions.chooseLook(look));
    lookItems.set(look, button);
    looks.append(button);
  }
  const drawLooks = (stage: number) => {
    if (looksDrawn === stage) return;
    looksDrawn = stage;
    const dpr = Math.min(2, window.devicePixelRatio || 1);
    const size = 40;
    for (const [look, button] of lookItems) {
      const canvas = button.querySelector("canvas")!;
      canvas.width = Math.round(size * dpr);
      canvas.height = Math.round(size * dpr);
      const ctx = canvas.getContext("2d");
      if (!ctx) continue;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      const engine = new BotEngine();
      engine.setSkin(look, false);
      engine.setStage(stage, false);
      engine.lite = true;
      engine.draw(ctx, size, size);
    }
  };

  return {
    el,
    sync() {
      const g = parseGrowth(State.settings.growth);
      const shown = shownStage(g);
      drawLooks(shown);
      progress.textContent = progressLine(g);
      const key = `${g.reached}:${shown}:${language()}`;
      if (stagePick.dataset.key !== key) {
        stagePick.dataset.key = key;
        stagePick.replaceChildren(...Array.from({ length: g.reached + 1 }, (_, i) =>
          h("option", { value: String(i), text: i === g.reached ? t("Stage {n} (now)", { n: i }) : t("Stage {n}", { n: i }) })));
        stagePick.value = String(shown);
      }
      stagePick.hidden = g.reached === 0;
      const look = parseSkin(State.settings.groovyLook);
      for (const [l, button] of lookItems) {
        button.classList.toggle("on", l === look);
        button.title = `${t(SKIN_KEYS[l])} → ${t(GROWS_INTO[l])} · ${progressLine(g)}`;
      }
      const on = worn(currentWardrobe(State.settings.wardrobe).appearance.layers);
      for (const [item, button] of items) button.classList.toggle("on", on.includes(item));
      for (const [slot, button] of nones) button.classList.toggle("on", !on.some((o) => o !== "none" && SLOT_OF[o] === slot));
      removeAll.disabled = on.length === 0;
    },
  };
}
