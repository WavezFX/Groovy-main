// Wardrobe pictures drawn once and reused (FIXES_ROUND3B item 4: the wardrobe
// lagged). A look's portrait or an item's icon is drawn on its own canvas the
// first time it is asked for, then only copied: no engine per tile, and the
// only live engine is the preview's.

import { BotEngine } from "./engine";
import { drawWardrobeIcon } from "./outfits";
import type { Skin } from "./skins";
import type { Outfit } from "./wardrobe";

const MAX = 64;
const cache = new Map<string, HTMLCanvasElement>();

function remember(key: string, draw: (ctx: CanvasRenderingContext2D, size: number) => void, size: number, dpr: number): HTMLCanvasElement {
  const hit = cache.get(key);
  if (hit) return hit;
  const sprite = document.createElement("canvas");
  sprite.width = Math.round(size * dpr);
  sprite.height = Math.round(size * dpr);
  const ctx = sprite.getContext("2d");
  if (ctx) {
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    draw(ctx, size);
  }
  if (cache.size >= MAX) cache.delete(cache.keys().next().value as string);
  cache.set(key, sprite);
  return sprite;
}

/** A still portrait of a look at a stage. */
export function lookSprite(look: Skin, stage: number, size: number, dpr: number): HTMLCanvasElement {
  return remember(`look:${look}:${stage}:${size}:${dpr}`, (ctx, s) => {
    const e = new BotEngine();
    e.setSkin(look, false);
    e.setStage(stage, false);
    e.lite = true;
    e.draw(ctx, s, s);
  }, size, dpr);
}

/** An item's icon (or None). */
export function itemSprite(item: Outfit, size: number, dpr: number): HTMLCanvasElement {
  return remember(`item:${item}:${size}:${dpr}`, (ctx, s) => drawWardrobeIcon(ctx, s, item), size, dpr);
}

/** A canvas of `size` CSS px showing a sprite (a copy: one sprite, many places). */
export function spriteCanvas(sprite: HTMLCanvasElement, size: number): HTMLCanvasElement {
  const c = document.createElement("canvas");
  c.width = sprite.width;
  c.height = sprite.height;
  c.style.width = `${size}px`;
  c.style.height = `${size}px`;
  c.getContext("2d")?.drawImage(sprite, 0, 0);
  return c;
}

/** How many pictures are kept (tests). */
export function spriteCount(): number {
  return cache.size;
}
