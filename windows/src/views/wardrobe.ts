// Wardrobe view — port of WardrobeView (IslandViewContent.swift). Opened with a
// right-click on Groovy or from the tray menu. Resting the pointer on a button
// tries the outfit on Groovy; a click keeps it.

import { h } from "./dom";
import { State } from "../core/state";
import { drawWardrobeIcon } from "../mochi/outfits";
import {
  OUTFIT_KEYS, OUTFIT_SELECTIONS, WARDROBE_STRINGS, parseOutfit, resolveOutfit, seasonalOutfit,
  wardrobeHeader, type Outfit, type OutfitSelection,
} from "../mochi/wardrobe";
import type { ViewActions, ViewHost } from "./views";
import { BotEngine } from "../mochi/engine";
import { SKINS, SKIN_KEYS, parseSkin, type Skin } from "../mochi/skins";
import { language, tl } from "../i18n/i18n";

const ICON = 28;

export function buildWardrobe(actions: ViewActions): ViewHost {
  const note = h("span", { class: "wardrobe-note" });
  const grid = h("div", { class: "wardrobe-grid" });
  const looks = h("div", { class: "wardrobe-grid wardrobe-looks" });
  const el = h(
    "div",
    { class: "view wardrobe" },
    h(
      "div",
      { class: "card" },
      h(
        "div",
        { class: "stack wardrobe-stack" },
        h("div", { class: "wardrobe-head" }, h("span", { class: "wardrobe-title", text: tl("Look") })),
        looks,
        h("div", { class: "wardrobe-head" }, h("span", { class: "wardrobe-title", text: tl("Wardrobe") }), note),
        grid,
      ),
    ),
  );

  let hovered: OutfitSelection | null = null;
  let drawnSeason: Outfit | null = null;
  let drawnLanguage = language();
  const items = new Map<OutfitSelection, { button: HTMLButtonElement; canvas: HTMLCanvasElement }>();

  const updateNote = () => {
    note.textContent = wardrobeHeader(hovered, parseOutfit(State.settings.mochiOutfit), new Date());
  };

  for (const sel of OUTFIT_SELECTIONS) {
    const canvas = h("canvas");
    const button = h(
      "button",
      { class: "wardrobe-item", title: tl(OUTFIT_KEYS[sel]), "aria-label": tl(OUTFIT_KEYS[sel]) },
      canvas,
    );
    button.addEventListener("mouseenter", () => {
      hovered = sel;
      actions.previewOutfit(resolveOutfit(sel, new Date()));
      updateNote();
    });
    button.addEventListener("mouseleave", () => {
      if (hovered !== sel) return;
      hovered = null;
      actions.previewOutfit(null);
      updateNote();
    });
    button.addEventListener("click", () => actions.chooseOutfit(sel));
    items.set(sel, { button, canvas });
    grid.append(button);
  }

  // Looks: a small still portrait of each, drawn once. Hover tries it on, click keeps it.
  const lookItems = new Map<Skin, HTMLButtonElement>();
  let looksDrawn = false;
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
  const drawLooks = () => {
    if (looksDrawn) return;
    looksDrawn = true;
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
      engine.draw(ctx, size, size);
    }
  };

  /** Icons are drawn once; only "auto" changes, with the season and its badge's language. */
  const drawIcons = () => {
    const season = seasonalOutfit(new Date());
    if (season === drawnSeason && drawnLanguage === language()) return;
    const first = drawnSeason == null;
    drawnSeason = season;
    drawnLanguage = language();
    const dpr = Math.min(2, window.devicePixelRatio || 1);
    for (const [sel, { canvas }] of items) {
      if (!first && sel !== "auto") continue;
      canvas.width = Math.round(ICON * dpr);
      canvas.height = Math.round(ICON * dpr);
      const ctx = canvas.getContext("2d");
      if (!ctx) continue;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      drawWardrobeIcon(ctx, ICON, sel, season, WARDROBE_STRINGS.autoBadge);
    }
  };

  return {
    el,
    sync() {
      drawIcons();
      drawLooks();
      const look = parseSkin(State.settings.groovyLook);
      for (const [l, button] of lookItems) button.classList.toggle("on", l === look);
      // The island drops the preview when the view closes: so does the hover.
      if (State.wardrobePreview == null) hovered = null;
      const current = parseOutfit(State.settings.mochiOutfit);
      for (const [sel, { button }] of items) button.classList.toggle("on", sel === current);
      updateNote();
    },
  };
}
