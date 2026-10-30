// Island → Layout & tiles → Tiles (round 3E item 3): the one place for the
// island's tiles. Every tile, the user's apps, websites and folders and
// Groovy's own and the services together, each with its real icon; tap to
// show or hide, drag (or the arrow keys) to reorder, "…" for Compact/Wide,
// Rename and Remove; "+ Add tile" picks an app, a website, a folder or a
// service. The order here is the island's order. Presets sit above the grid;
// any change makes the list Custom.

import { Bridge } from "../core/bridge";
import { availablePills, sanitizeDeclared } from "../core/pills";
import type { Settings } from "../core/state";
import {
  OWN_KINDS, addTile, applyPreset, currentTiles, hideTile, keptServices, moveTile, removeTile, renameTile, resizeTile, servicesOf, showTile, shortAppName, tile, tileService, withServices,
  type IslandPreset, type IslandTiles, type Tile,
} from "../core/tiles";
import { N_, t } from "../i18n/i18n";
import { clear, h } from "../views/dom";
import { showError } from "../views/toast";
import { brandMark, tileIcon } from "../views/tileicon";

export const TILES_STRINGS = {
  title: N_("Tiles"),
  hint: N_("Tap to show or hide · drag to reorder · this is the island's order"),
  add: N_("+ Add tile"),
  developer: N_("Developer"),
  everyday: N_("Everyday"),
  custom: N_("Custom"),
  compact: N_("Compact"),
  wide: N_("Wide"),
  rename: N_("Rename"),
  remove: N_("Remove"),
  more: N_("More for {name}"),
  app: N_("App"),
  site: N_("Website"),
  folder: N_("Folder"),
  service: N_("Service"),
  search: N_("Search your apps"),
  noApps: N_("No apps found"),
  addSite: N_("Add website"),
  pickFolder: N_("Choose a folder…"),
  needsHttps: N_("A website starts with https://"),
  full: N_("This tile is already on the island."),
};

/** Groovy's own tiles, always on offer: name and colour. */
const OWN: { kind: Tile["kind"]; name: string; color: string; icon: string }[] = [
  { kind: "mail", name: N_("Mail"), color: "#4C8DF6", icon: "✉" },
  { kind: "music", name: "Spotify", color: "#1ED760", icon: "♪" },
  { kind: "reminders", name: N_("Reminders"), color: "#F5A524", icon: "⏰" },
  { kind: "system", name: N_("This PC"), color: "#3FC1C9", icon: "▤" },
  { kind: "agents", name: N_("Coding agents"), color: "#E8A27C", icon: "⌘" },
];

export function tileName(x: Tile): string {
  if (x.label) return x.label;
  if (x.kind === "service") return availablePills().find((p) => p.id === x.target)?.name ?? x.target;
  if (x.kind === "app" || x.kind === "folder") return shortAppName(x.target.split(/[\\/]/).pop()?.replace(/\.(lnk|exe|url)$/i, "") ?? x.target);
  if (x.kind === "site") return x.target.replace(/^https?:\/\//, "").split("/")[0];
  const own = OWN.find((o) => o.kind === x.kind);
  return own ? t(own.name) : x.kind;
}

/** A tile's icon: its logo mark, its real icon, or Groovy's own glyph. */
export function tileMark(x: Tile, size = 22): HTMLElement {
  if (x.kind === "service") return brandMark(x.target, size) ?? h("span", { class: "home-mark letter", style: `background:${availablePills().find((p) => p.id === x.target)?.color ?? "#8E939C"}`, text: tileName(x).slice(0, 1) });
  if (x.kind === "music") return brandMark("integration_spotify", size)!;
  if (x.kind === "agents") return brandMark("integration_claude", size)!;
  if (OWN_KINDS.includes(x.kind)) return tileIcon(x.kind, x.target, x.kind === "site" ? "🌐" : x.kind === "folder" ? "📁" : tileName(x).slice(0, 1).toUpperCase(), x.kind === "site" ? "#3B9EFF" : x.kind === "folder" ? "#F7B32B" : "#A78BFA", size);
  const own = OWN.find((o) => o.kind === x.kind);
  return h("span", { class: "home-mark letter", style: `background:${own?.color ?? "#8E939C"}`, text: own?.icon ?? "•" });
}

interface Entry {
  tile: Tile;
  shown: boolean;
}

/** Every tile: the island's in order, then those kept hidden, then everything else on offer. */
export function entries(cur: IslandTiles): Entry[] {
  const out: Entry[] = cur.tiles.map((x) => ({ tile: x, shown: true }));
  const has = (x: Tile) => out.some((e) => e.tile.id === x.id || (tileService(x) !== null && tileService(e.tile) === tileService(x)));
  for (const x of cur.hidden ?? []) if (!has(x)) out.push({ tile: x, shown: false });
  for (const o of OWN) {
    const x = tile(o.kind);
    if (!has(x)) out.push({ tile: x, shown: false });
  }
  for (const p of availablePills().filter((p) => p.category === "service" && p.id !== "integration_spotify")) {
    const x = tile("service", p.id);
    if (!has(x)) out.push({ tile: x, shown: false });
  }
  return out;
}

export function tilesCard(settings: Settings, save: () => void, changed: () => void): HTMLElement {
  const root = h("div", { class: "hub-i-tilesbox" });
  let menuFor: string | null = null;
  let renaming: string | null = null;
  let picker = false;
  let pickKind: "app" | "site" | "folder" | "service" = "app";
  let apps: { name: string; target: string }[] | null = null;
  let query = "";
  let focusId: string | null = null;

  const commit = (next: IslandTiles, removed?: string) => {
    const declared = sanitizeDeclared(settings);
    settings.activeIntegrations = keptServices(servicesOf(next, declared.mainPill), declared.activeIntegrations, removed);
    settings.islandTiles = withServices(next, settings.activeIntegrations, declared.mainPill);
    save();
    draw();
    changed();
  };

  function draw() {
    clear(root);
    const declared = sanitizeDeclared(settings);
    const cur = withServices(currentTiles(settings), declared.activeIntegrations, declared.mainPill);

    // Presets: a small segmented control.
    const seg = h("div", { class: "hub-segmented", role: "radiogroup", "aria-label": t(TILES_STRINGS.title) });
    for (const id of ["developer", "everyday", "custom"] as IslandPreset[]) {
      const on = cur.preset === id;
      const b = h("button", { type: "button", role: "radio", class: on ? "on" : "", "aria-checked": String(on), text: t(TILES_STRINGS[id]) });
      b.addEventListener("click", () => commit(applyPreset(cur, id)));
      seg.append(b);
    }
    const addBtn = h("button", { type: "button", class: "hub-btn", text: t(TILES_STRINGS.add), "aria-expanded": String(picker) });
    addBtn.addEventListener("click", () => {
      picker = !picker;
      draw();
    });
    root.append(h("div", { class: "hub-i-tilesbar" }, seg, addBtn));
    if (picker) root.append(pickerCard(cur));

    const grid = h("div", { class: "hub-card hub-i-tiles", role: "list" });
    const list = entries(cur);
    list.forEach((e, i) => {
      const x = e.tile;
      const main = tileService(x) === declared.mainPill;
      const name = tileName(x);
      const cell = h("div", { class: `hub-i-tile${e.shown ? " on" : ""}`, role: "listitem", "data-id": x.id });
      const toggle = h("button", { type: "button", class: "hub-i-tile-main", "aria-pressed": String(e.shown), title: main ? t("The main tool stays on the island") : name }, tileMark(x), h("span", { class: "hub-i-name", text: name }));
      (toggle as HTMLButtonElement).disabled = main && e.shown;
      toggle.addEventListener("click", () => {
        if (e.shown) {
          if (OWN_KINDS.includes(x.kind)) commit(hideTile(cur, x.id));
          else commit(removeTile(cur, x.id), tileService(x) ?? undefined);
        } else if ((cur.hidden ?? []).some((y) => y.id === x.id)) commit(showTile(cur, x.id));
        else commit(addTile(cur, x) ?? cur);
      });
      cell.append(toggle);
      if (e.shown) {
        // Drag to reorder; the arrow keys move a focused tile.
        cell.draggable = true;
        cell.addEventListener("dragstart", (ev) => ev.dataTransfer?.setData("text/plain", x.id));
        cell.addEventListener("dragover", (ev) => ev.preventDefault());
        cell.addEventListener("drop", (ev) => {
          ev.preventDefault();
          const id = ev.dataTransfer?.getData("text/plain");
          if (id && id !== x.id) commit(moveTile(cur, id, i));
        });
        toggle.addEventListener("keydown", (ev) => {
          const step = ev.key === "ArrowLeft" || ev.key === "ArrowUp" ? -1 : ev.key === "ArrowRight" || ev.key === "ArrowDown" ? 1 : 0;
          if (!step) return;
          ev.preventDefault();
          focusId = x.id;
          commit(moveTile(cur, x.id, Math.max(0, Math.min(cur.tiles.length - 1, i + step))));
        });
      }
      const more = h("button", { type: "button", class: "hub-i-more", text: "…", "aria-label": t(TILES_STRINGS.more, { name }), "aria-expanded": String(menuFor === x.id) });
      more.addEventListener("click", () => {
        menuFor = menuFor === x.id ? null : x.id;
        renaming = null;
        draw();
      });
      cell.append(more);
      if (menuFor === x.id) cell.append(menu(cur, x, e.shown));
      grid.append(cell);
    });
    root.append(h("span", { class: "hub-b-detail hub-i-hint", text: t(TILES_STRINGS.hint) }), grid);
    if (focusId) {
      const again = grid.querySelector<HTMLElement>(`[data-id="${CSS.escape(focusId)}"] .hub-i-tile-main`);
      focusId = null;
      again?.focus();
    }
  }

  function menu(cur: IslandTiles, x: Tile, shown: boolean): HTMLElement {
    const box = h("div", { class: "hub-i-menu", role: "menu" });
    if (renaming === x.id) {
      const input = h("input", { type: "text", maxlength: 40, value: x.label || tileName(x), "aria-label": t(TILES_STRINGS.rename) }) as HTMLInputElement;
      const done = () => {
        menuFor = null;
        renaming = null;
        commit(renameTile(cur, x.id, input.value.trim() === tileName({ ...x, label: "" }) ? "" : input.value));
      };
      input.addEventListener("keydown", (e) => {
        if (e.key === "Enter") done();
        if (e.key === "Escape") {
          renaming = null;
          draw();
        }
      });
      box.append(input, h("button", { type: "button", class: "hub-btn", text: t("Save"), onclick: done }));
      requestAnimationFrame(() => input.focus());
      return box;
    }
    const item = (label: string, run: () => void, danger = false) => h("button", { type: "button", role: "menuitem", class: danger ? "danger" : "", text: label, onclick: run });
    if (shown) box.append(item(x.size === "expanded" ? t(TILES_STRINGS.compact) : t(TILES_STRINGS.wide), () => {
      menuFor = null;
      commit(resizeTile(cur, x.id, x.size === "expanded" ? "compact" : "expanded"));
    }));
    if (shown || OWN_KINDS.includes(x.kind)) box.append(item(t(TILES_STRINGS.rename), () => {
      renaming = x.id;
      draw();
    }));
    if (OWN_KINDS.includes(x.kind)) box.append(item(t(TILES_STRINGS.remove), () => {
      menuFor = null;
      commit(removeTile(cur, x.id));
    }, true));
    return box;
  }

  function pickerCard(cur: IslandTiles): HTMLElement {
    const box = h("div", { class: "hub-card hub-i-picker" });
    const seg = h("div", { class: "hub-segmented", role: "radiogroup" });
    for (const k of ["app", "site", "folder", "service"] as const) {
      const b = h("button", { type: "button", role: "radio", class: pickKind === k ? "on" : "", "aria-checked": String(pickKind === k), text: t(TILES_STRINGS[k]) });
      b.addEventListener("click", () => {
        pickKind = k;
        draw();
      });
      seg.append(b);
    }
    box.append(seg);
    const add = (x: Tile | null) => {
      if (!x) return;
      const next = addTile(cur, x);
      if (!next) return showError(t(TILES_STRINGS.full));
      picker = false;
      commit(next);
    };
    if (pickKind === "app") {
      const search = h("input", { type: "search", placeholder: t(TILES_STRINGS.search), value: query, "aria-label": t(TILES_STRINGS.search) }) as HTMLInputElement;
      const results = h("div", { class: "hub-i-apps", role: "list" });
      const fill = () => {
        clear(results);
        const q = query.toLowerCase();
        const found = (apps ?? []).filter((a) => !q || a.name.toLowerCase().includes(q)).slice(0, 40);
        if (apps && !found.length) results.append(h("span", { class: "hub-b-detail", text: t(TILES_STRINGS.noApps) }));
        for (const a of found) {
          const x = tile("app", a.target, "");
          results.append(h("button", { type: "button", class: "hub-i-app", role: "listitem", onclick: () => add(x) }, tileMark(x, 20), h("span", { text: shortAppName(a.name) })));
        }
      };
      search.addEventListener("input", () => {
        query = search.value;
        fill();
      });
      box.append(search, results);
      if (apps) fill();
      else void Bridge.islandApps().then((list) => {
        apps = list ?? [];
        fill();
      });
    } else if (pickKind === "site") {
      const url = h("input", { type: "url", placeholder: "https://…" }) as HTMLInputElement;
      const name = h("input", { type: "text", placeholder: t("Name (optional)"), maxlength: 40 }) as HTMLInputElement;
      box.append(h("div", { class: "hub-i-row" }, url, name, h("button", { type: "button", class: "hub-btn primary", text: t(TILES_STRINGS.addSite), onclick: () => {
        const u = url.value.trim();
        if (!/^https?:\/\//i.test(u)) return showError(t(TILES_STRINGS.needsHttps));
        add(tile("site", u, name.value.trim()));
      } })));
    } else if (pickKind === "folder") {
      box.append(h("button", { type: "button", class: "hub-btn primary", text: t(TILES_STRINGS.pickFolder), onclick: () => void Bridge.islandPickFolder().then((dir) => add(dir ? tile("folder", dir) : null), showError) }));
    } else {
      const results = h("div", { class: "hub-i-apps", role: "list" });
      for (const p of availablePills().filter((p) => p.category === "service" || p.category === "ai")) {
        const x = tile("service", p.id);
        results.append(h("button", { type: "button", class: "hub-i-app", role: "listitem", onclick: () => add(x) }, tileMark(x, 20), h("span", { text: p.name })));
      }
      box.append(results);
    }
    return box;
  }

  draw();
  return root;
}
