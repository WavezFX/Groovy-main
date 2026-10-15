// Control Centre → My island (FIXES_ROUND1 item 4): which tiles the island
// shows, in which order and size, from a preset or the user's own list, and
// which apps' notifications pop up as mail. The agents and their hooks are set
// up once in Coding agents; they don't need to sit on the island.

import { Bridge } from "../core/bridge";
import { availablePills } from "../core/pills";
import type { Settings } from "../core/state";
import {
  MAX_TILES, addTile, applyPreset, currentTiles, moveTile, removeTile, resizeTile, tile,
  type IslandPreset, type IslandTiles, type Tile, type TileKind,
} from "../core/tiles";
import { h, clear } from "../views/dom";
import { showError } from "../views/toast";
import { N_, t } from "../i18n/i18n";

const PRESETS: [IslandPreset, string, string][] = [
  ["developer", N_("Developer"), N_("Today's island: your coding agents, their questions and diffs.")],
  ["everyday", N_("Everyday"), N_("Mail, music, reminders and your PC, with your own apps added.")],
  ["custom", N_("Custom"), N_("Exactly the tiles below.")],
];

const KIND_LABEL: Record<TileKind, string> = {
  app: N_("App"),
  site: N_("Website"),
  folder: N_("Folder"),
  service: N_("Service"),
  mail: N_("Mail"),
  music: N_("Music"),
  agents: N_("Agents"),
  reminders: N_("Reminders"),
  system: N_("This PC"),
};

function tileName(x: Tile): string {
  if (x.label) return x.label;
  if (x.kind === "service") return availablePills().find((p) => p.id === x.target)?.name ?? x.target;
  if (x.kind === "app" || x.kind === "folder") return x.target.split(/[\\/]/).pop()?.replace(/\.(lnk|exe|url)$/i, "") ?? x.target;
  if (x.kind === "site") return x.target.replace(/^https?:\/\//, "");
  return t(KIND_LABEL[x.kind]);
}

export function islandSection(settings: Settings, save: () => void): HTMLElement {
  const section = h("section", { id: "my-island" });
  let apps: { name: string; target: string }[] | null = null;

  const commit = (next: IslandTiles) => {
    settings.islandTiles = next;
    save();
    draw();
  };

  function draw() {
    clear(section);
    const cur = currentTiles(settings);
    section.append(h("h2", {}, h("span", { text: t("My island") })));

    // Preset.
    const presetRow = h("div", { class: "row" });
    for (const [id, label] of PRESETS) {
      const b = h("button", { class: cur.preset === id ? "primary" : "", text: t(label), "aria-pressed": cur.preset === id }) as HTMLButtonElement;
      b.addEventListener("click", () => commit(applyPreset(cur, id)));
      presetRow.append(b);
    }
    section.append(presetRow, h("div", { class: "hint", text: t(PRESETS.find((p) => p[0] === cur.preset)![2]) }));

    // The tiles, in order: drag to reorder, or use the arrows.
    const list = h("div", { class: "tile-list" });
    cur.tiles.forEach((x, i) => {
      const row = h("div", { class: "row tile-row", draggable: "true" });
      row.addEventListener("dragstart", (e) => e.dataTransfer?.setData("text/plain", x.id));
      row.addEventListener("dragover", (e) => e.preventDefault());
      row.addEventListener("drop", (e) => {
        e.preventDefault();
        const id = e.dataTransfer?.getData("text/plain");
        if (id) commit(moveTile(cur, id, i));
      });
      const size = h("button", { text: x.size === "expanded" ? t("Wide") : t("Compact"), title: t("Compact or wide on the island") }) as HTMLButtonElement;
      size.addEventListener("click", () => commit(resizeTile(cur, x.id, x.size === "expanded" ? "compact" : "expanded")));
      const up = h("button", { text: "↑", title: t("Move up"), "aria-label": t("Move up") }) as HTMLButtonElement;
      up.addEventListener("click", () => commit(moveTile(cur, x.id, i - 1)));
      const down = h("button", { text: "↓", title: t("Move down"), "aria-label": t("Move down") }) as HTMLButtonElement;
      down.addEventListener("click", () => commit(moveTile(cur, x.id, i + 1)));
      const remove = h("button", { class: "danger", text: t("Remove") }) as HTMLButtonElement;
      remove.addEventListener("click", () => commit(removeTile(cur, x.id)));
      row.append(
        h("span", { class: "drag", text: "⠿", "aria-hidden": "true" }),
        h("label", { text: tileName(x) }),
        h("span", { class: "hint", text: t(KIND_LABEL[x.kind]) }),
        size, up, down, remove,
      );
      list.append(row);
    });
    if (!cur.tiles.length) list.append(h("div", { class: "hint", text: t("No tiles: the island shows your agents as before.") }));
    section.append(h("div", { class: "subhead", text: t("Tiles") }), list);

    // Adding a tile.
    const kind = h("select", {}) as HTMLSelectElement;
    for (const k of Object.keys(KIND_LABEL) as TileKind[]) kind.append(h("option", { value: k, text: t(KIND_LABEL[k]) }));
    const extra = h("span", { class: "row", style: "flex:1;gap:6px;margin:0" });
    let pending: () => Promise<Tile | null> = async () => tile(kind.value as TileKind);
    const drawExtra = async () => {
      clear(extra);
      const k = kind.value as TileKind;
      pending = async () => tile(k);
      if (k === "app") {
        apps ??= (await Bridge.islandApps()) ?? [];
        const pick = h("select", { style: "flex:1" }) as HTMLSelectElement;
        if (!apps.length) pick.append(h("option", { value: "", text: t("No apps found") }));
        for (const a of apps) pick.append(h("option", { value: a.target, text: a.name }));
        extra.append(pick);
        pending = async () => (pick.value ? tile("app", pick.value, pick.selectedOptions[0]?.textContent ?? "") : null);
      } else if (k === "site") {
        const url = h("input", { type: "url", placeholder: "https://…", style: "flex:1" }) as HTMLInputElement;
        const name = h("input", { type: "text", placeholder: t("Name (optional)"), maxlength: 40, style: "width:140px" }) as HTMLInputElement;
        extra.append(url, name);
        pending = async () => {
          const u = url.value.trim();
          return /^https?:\/\//i.test(u) ? tile("site", u, name.value.trim()) : (showError(t("A website starts with https://")), null);
        };
      } else if (k === "folder") {
        extra.append(h("span", { class: "hint", text: t("You pick the folder after clicking Add.") }));
        pending = async () => {
          const dir = await Bridge.islandPickFolder();
          return dir ? tile("folder", dir) : null;
        };
      } else if (k === "service") {
        const pick = h("select", { style: "flex:1" }) as HTMLSelectElement;
        for (const p of availablePills().filter((p) => p.category === "service" || p.category === "ai")) pick.append(h("option", { value: p.id, text: p.name }));
        extra.append(pick);
        pending = async () => tile("service", pick.value);
      }
    };
    kind.addEventListener("change", () => void drawExtra());
    const add = h("button", { class: "primary", text: t("Add") }) as HTMLButtonElement;
    add.addEventListener("click", () => {
      void pending().then((x) => {
        if (!x) return;
        const next = addTile(currentTiles(settings), x);
        if (!next) {
          showError(currentTiles(settings).tiles.length >= MAX_TILES ? t("The island holds {n} tiles at most.", { n: MAX_TILES }) : t("This tile is already on the island."));
          return;
        }
        commit(next);
      }, showError);
    });
    section.append(h("div", { class: "subhead", text: t("Add a tile") }), h("div", { class: "row" }, kind, extra, add));
    void drawExtra();

    // Mail pop-ups.
    const status = h("span", { class: "hint" });
    const allow = h("button", { text: t("Allow mail pop-ups") }) as HTMLButtonElement;
    allow.addEventListener("click", () => {
      void Bridge.mailAccess().then((s) => {
        status.textContent =
          s === "allowed" ? t("Allowed: new mail shows on the island.")
          : s === "denied" ? t("Windows said no. Allow it in Windows Settings → Privacy → Notifications, then try again.")
          : s === "unsupported" ? t("This version of Windows doesn't let Groovy read notifications.")
          : t("Windows didn't answer. Try again.");
      });
    });
    const appsInput = h("input", { type: "text", placeholder: t("Outlook, Mail, Chrome…"), value: cur.mailApps.join(", "), style: "flex:1" }) as HTMLInputElement;
    appsInput.addEventListener("change", () => {
      commit({ ...currentTiles(settings), mailApps: appsInput.value.split(",").map((s) => s.trim()).filter(Boolean).slice(0, 20) });
    });
    section.append(
      h("div", { class: "subhead", text: t("Mail pop-ups") }),
      h("div", { class: "hint", text: t("New mail pops up on the island from Windows' own notifications: no account or password, it works with Outlook, Mail, Gmail in a browser and anything else that shows a notification. Add a Mail tile to turn it on.") }),
      h("div", { class: "row" }, allow, status),
      h("div", { class: "row" }, h("label", { text: t("From these apps") }), appsInput),
      h("div", { class: "hint", text: t("Empty: any app with mail in its name (Outlook, Mail, Thunderbird…).") }),
      h("div", { class: "hint", text: t("Agents and their hooks are set up in Coding agents.") }),
    );
  }

  draw();
  return section;
}
