// Round 3E item 2: a tile's icon. Apps, websites and folders get the real one
// from Rust (tile_icons.rs, cached on disk), loaded when the tile is drawn;
// services get their logo mark (brands.ts). A letter only when there is
// nothing else. In memory only the few last shown are kept.

import { Bridge } from "../core/bridge";
import { BRANDS } from "./brands";
import { h } from "./dom";

const SVG = "http://www.w3.org/2000/svg";

/** Icons in memory: about two pages of tiles, the oldest dropped first. */
const KEEP = 24;
const loaded = new Map<string, string | null>();
const waiting = new Map<string, Promise<string | null>>();

function remember(key: string, url: string | null) {
  loaded.delete(key);
  loaded.set(key, url);
  while (loaded.size > KEEP) loaded.delete(loaded.keys().next().value as string);
}

function fetchIcon(kind: string, target: string, px: number): Promise<string | null> {
  const key = `${kind}\n${target}\n${px}`;
  if (loaded.has(key)) {
    const url = loaded.get(key) ?? null;
    remember(key, url);
    return Promise.resolve(url);
  }
  let p = waiting.get(key);
  if (!p) {
    p = Bridge.tileIcon(kind, target, px).then((url) => {
      waiting.delete(key);
      remember(key, url);
      return url;
    });
    waiting.set(key, p);
  }
  return p;
}

/** A service's logo mark: its glyph in white on its brand colour. */
export function brandMark(id: string, size = 22): HTMLElement | null {
  const brand = BRANDS[id];
  if (!brand) return null;
  const s = document.createElementNS(SVG, "svg");
  s.setAttribute("viewBox", "0 0 24 24");
  s.setAttribute("width", String(Math.round(size * 0.62)));
  s.setAttribute("height", String(Math.round(size * 0.62)));
  const path = document.createElementNS(SVG, "path");
  path.setAttribute("d", brand.path);
  path.setAttribute("fill", "#fff");
  s.append(path);
  return h("span", { class: "home-mark brand", style: `background:${brand.hex}` }, s as unknown as HTMLElement);
}

/**
 * The icon for an app, website or folder tile: the letter shows at once and
 * is replaced by the real icon as soon as it arrives (2× the shown size).
 */
export function tileIcon(kind: string, target: string, letter: string, color: string, size = 22): HTMLElement {
  const mark = h("span", { class: "home-mark letter", style: `background:${color}`, text: letter });
  void fetchIcon(kind, target, size * 2).then((url) => {
    if (!url) return;
    const img = h("img", { class: "home-mark icon", src: url, alt: "", draggable: "false" });
    mark.replaceWith(img);
  });
  return mark;
}
