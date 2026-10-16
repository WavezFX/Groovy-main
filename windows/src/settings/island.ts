// Control Centre → My island (FIXES_ROUND1 item 4, one list since round 3
// item 4): every tile the island shows, in order — apps, websites, folders,
// mail, music, reminders, the PC, and every service turned on (GitHub,
// Notion, Cal.com…), with no limit: the island pages them. Adding a service
// tile turns the service on and asks for its key; removing it turns it off.
// The island's "+ Add tile" opens this page.

import { Bridge } from "../core/bridge";
import { KEY_PAGES, availablePills, pillDefinition, sanitizeDeclared } from "../core/pills";
import type { Settings } from "../core/state";
import {
  MAX_TILES, addTile, applyPreset, currentTiles, moveTile, removeTile, resizeTile, keptServices, servicesOf, tile, tileService, withServices,
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

  /** Keys already stored, to say which service still needs one. */
  const keys: Record<string, boolean> = {};
  const commit = (next: IslandTiles, removed?: string) => {
    // One list: the service tiles are the services turned on. Only removing a
    // service's own tile turns it off: a preset or the mail apps never do
    // (round 3B item 7: they used to switch every integration off).
    const declared = sanitizeDeclared(settings);
    settings.activeIntegrations = keptServices(servicesOf(next, declared.mainPill), declared.activeIntegrations, removed);
    settings.islandTiles = withServices(next, settings.activeIntegrations, declared.mainPill);
    save();
    draw();
  };

  function draw() {
    clear(section);
    const declared = sanitizeDeclared(settings);
    const cur = withServices(currentTiles(settings), declared.activeIntegrations, declared.mainPill);
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
      remove.addEventListener("click", () => commit(removeTile(cur, x.id), tileService(x) ?? undefined));
      // The main tool (Coding agents) is always on: its tile stays.
      remove.hidden = tileService(x) === declared.mainPill;
      row.append(
        h("span", { class: "drag", text: "⠿", "aria-hidden": "true" }),
        h("label", { text: tileName(x) }),
        h("span", { class: "hint", text: t(KIND_LABEL[x.kind]) }),
        size, up, down, remove,
      );
      list.append(row);
      // A service that needs a key and has none: its field, never a silent refusal.
      const def = x.kind === "service" ? pillDefinition(x.target) : undefined;
      if (def?.connect.kind === "key" && keys[def.connect.key] === false) list.append(keyRow(def.id, def.name, def.connect.key));
    });
    if (!cur.tiles.length) list.append(h("div", { class: "hint", text: t("No tiles yet: add one below.") }));
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

  /** "Notion needs a key": the field, Save, and where to get one. */
  function keyRow(id: string, name: string, key: string): HTMLElement {
    const input = h("input", { type: "password", placeholder: t("{name} key", { name }), autocomplete: "off", spellcheck: "false", style: "flex:1" }) as HTMLInputElement;
    const saveBtn = h("button", { class: "primary", text: t("Save") }) as HTMLButtonElement;
    saveBtn.addEventListener("click", () => {
      const value = input.value.trim();
      if (!value) return;
      void Bridge.secretSet(key, value).then(() => {
        keys[key] = true;
        draw();
      }, showError);
    });
    const row = h("div", { class: "row tile-key" }, h("span", { class: "hint warn", text: t("{name} needs its key:", { name }) }), input, saveBtn);
    const page = KEY_PAGES[id];
    if (page) row.append(h("a", { href: "#", text: t("Get a key ↗"), onclick: (e: Event) => { e.preventDefault(); void Bridge.openUrl(page); } }));
    return row;
  }

  // Which keys are there, once; the rows redraw when they are known.
  const wanted = availablePills().flatMap((p) => (p.connect.kind === "key" ? [p.connect.key] : []));
  void Promise.all(wanted.map(async (k) => [k, (await Bridge.secretPresent(k)) ?? false] as const)).then((pairs) => {
    for (const [k, v] of pairs) keys[k] = v;
    draw();
  });

  draw();
  return section;
}
