// Wardrobe view — opened with a right-click on Groovy or from the tray menu.
// His looks, then Head, Face and Neck (FIXES_ROUND3 item 5): each slot on its
// own, with its own None, and Remove all, then the Seasonal items (round 5
// item 4). Resting the pointer on an item tries it on in its slot; a click
// wears it (or takes it off). An item that waits for a stage, or a season not
// yet earned, is dashed and says when it comes (round 3E item 7).

import { h } from "./dom";
import { State } from "../core/state";
import { ACCESSORIES, OUTFIT_KEYS, SEASONAL_ITEMS, isSeasonal, type Accessory } from "../mochi/wardrobe";
import { lockLabel, parseEarned } from "../mochi/seasons";
import { SLOTS, SLOT_OF, currentWardrobe, worn, type Slot } from "../mochi/appearance";
import type { ViewActions, ViewHost } from "./views";
import { itemSprite, lookSprite, spriteCanvas } from "../mochi/sprites";
import { GROWS_INTO, SKINS, SKIN_KEYS, parseSkin, type Skin } from "../mochi/skins";
import { parseGrowth, progressLine, shownStage } from "../mochi/growth";
import { N_, t, tl } from "../i18n/i18n";

const ICON = 26;

export const SLOT_TITLES: Record<Slot, string> = { head: N_("Head"), face: N_("Face"), neck: N_("Neck") };

function icon(item: Accessory | "none"): HTMLCanvasElement {
  return spriteCanvas(itemSprite(item, ICON, Math.min(2, window.devicePixelRatio || 1)), ICON);
}

export function buildWardrobe(actions: ViewActions): ViewHost {
  const looks = h("div", { class: "wardrobe-grid wardrobe-looks" });
  const rows = h("div", { class: "wardrobe-slots" });
  // How far he has grown (round 3 item 6); always shown as he is (round 3B item 5).
  const progress = h("span", { class: "wardrobe-progress" });
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
        h("div", { class: "wardrobe-head" }, h("span", { class: "wardrobe-title", text: tl("Look") }), progress),
        looks,
        h("div", { class: "wardrobe-head" }, h("span", { class: "wardrobe-title", text: tl("Wardrobe") }), removeAll),
        rows,
      ),
    ),
  );

  const items = new Map<Accessory, HTMLButtonElement>();
  const nones = new Map<Slot, HTMLButtonElement>();
  const itemButton = (item: Accessory) => {
    const button = h("button", { class: "wardrobe-item", title: tl(OUTFIT_KEYS[item]), "aria-label": tl(OUTFIT_KEYS[item]) }, icon(item)) as HTMLButtonElement;
    button.addEventListener("mouseenter", () => actions.previewOutfit(item));
    button.addEventListener("mouseleave", () => actions.previewOutfit(null));
    button.addEventListener("click", () => actions.dress(item));
    items.set(item, button);
    return button;
  };
  for (const slot of SLOTS) {
    const row = h("div", { class: "wardrobe-grid wardrobe-slot" }, h("span", { class: "wardrobe-slot-name", text: tl(SLOT_TITLES[slot]) }));
    const none = h("button", { class: "wardrobe-item", title: tl("None"), "aria-label": tl("None") }, icon("none")) as HTMLButtonElement;
    none.addEventListener("click", () => actions.dress(slot));
    nones.set(slot, none);
    row.append(none);
    for (const item of ACCESSORIES.filter((a) => SLOT_OF[a] === slot && !isSeasonal(a))) row.append(itemButton(item));
    rows.append(row);
  }
  const seasonal = h("div", { class: "wardrobe-grid wardrobe-slot" }, h("span", { class: "wardrobe-slot-name", text: tl("Seasonal") }));
  for (const item of SEASONAL_ITEMS) seasonal.append(itemButton(item));
  rows.append(seasonal);

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
  // Each portrait is a whole Groovy drawn in software on a GPU-less PC: never
  // all three in the frame the wardrobe opens in (the frame-time test saw
  // 0.1–3.5 s there). They are drawn ahead, while the island is idle, and
  // any still missing one per frame after it opens.
  const LOOK_SIZE = 40;
  const dprNow = () => Math.min(2, window.devicePixelRatio || 1);
  const drawLooks = (stage: number) => {
    if (looksDrawn === stage) return;
    looksDrawn = stage;
    const queue = [...lookItems];
    const next = () => {
      const item = queue.shift();
      if (!item || looksDrawn !== stage) return;
      const [look, button] = item;
      button.querySelector("canvas")!.replaceWith(spriteCanvas(lookSprite(look, stage, LOOK_SIZE, dprNow()), LOOK_SIZE));
      if (queue.length) requestAnimationFrame(next);
    };
    requestAnimationFrame(next);
  };
  const prewarm = (stage: number) => {
    const idle = (fn: () => void) => {
      if (typeof requestIdleCallback === "function") requestIdleCallback(fn, { timeout: 4000 });
      else setTimeout(fn, 200);
    };
    const looks = [...lookItems.keys()];
    const one = () => {
      const look = looks.shift();
      if (!look) return;
      lookSprite(look, stage, LOOK_SIZE, dprNow());
      idle(one);
    };
    idle(one);
  };
  prewarm(shownStage(parseGrowth(State.settings.growth)));

  // Locks change with the stage, the season or the language: worked out then
  // only. The first time is slow (dates in the language), so it is done while
  // the island is idle, never in the frame the wardrobe opens in.
  let locksFor = "";
  const relock = (reached: number) => {
    const earned = parseEarned(State.settings.seasonalEarned);
    const key = `${reached}|${earned.join()}|${t("None")}|${new Date().toDateString()}`;
    if (key === locksFor) return;
    locksFor = key;
    for (const [item, button] of items) {
      const lock = lockLabel(item, reached, earned);
      button.classList.toggle("locked", !!lock);
      button.title = lock ? `${t(OUTFIT_KEYS[item])} · ${lock}` : t(OUTFIT_KEYS[item]);
    }
  };
  if (typeof requestIdleCallback === "function") requestIdleCallback(() => relock(parseGrowth(State.settings.growth).reached), { timeout: 5000 });
  else setTimeout(() => relock(parseGrowth(State.settings.growth).reached), 300);
  return {
    el,
    sync() {
      const g = parseGrowth(State.settings.growth);
      const shown = shownStage(g);
      drawLooks(shown);
      progress.textContent = progressLine(g);
      const look = parseSkin(State.settings.groovyLook);
      for (const [l, button] of lookItems) {
        button.classList.toggle("on", l === look);
        button.title = `${t(SKIN_KEYS[l])} → ${t(GROWS_INTO[l])} · ${progressLine(g)}`;
      }
      const on = worn(currentWardrobe(State.settings.wardrobe).appearance.layers);
      relock(g.reached);
      for (const [item, button] of items) button.classList.toggle("on", on.includes(item));
      for (const [slot, button] of nones) button.classList.toggle("on", !on.some((o) => o !== "none" && SLOT_OF[o] === slot));
      removeAll.disabled = on.length === 0;
    },
  };
}
