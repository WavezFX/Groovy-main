// Island → Layout & tiles, below the Tiles card (settings/tilesgrid.ts, one
// list since round 3E item 3): a service tile that still needs its key, with
// the field to save it, and the mail pop-ups from Windows' notifications.

import { Bridge } from "../core/bridge";
import { KEY_PAGES, availablePills, pillDefinition, sanitizeDeclared } from "../core/pills";
import type { Settings } from "../core/state";
import { currentTiles, keptServices, servicesOf, withServices, type IslandTiles } from "../core/tiles";
import { h, clear } from "../views/dom";
import { showError } from "../views/toast";
import { t } from "../i18n/i18n";

export function islandSection(settings: Settings, save: () => void): HTMLElement {
  const section = h("section", { id: "my-island" });

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
    // The tiles themselves are in the Tiles card above (round 3E item 3);
    // here: a shown service that still needs its key, and the mail pop-ups.
    const needs = h("div", { class: "tile-list" });
    for (const x of cur.tiles) {
      const def = x.kind === "service" ? pillDefinition(x.target) : undefined;
      if (def?.connect.kind === "key" && keys[def.connect.key] === false) needs.append(keyRow(def.id, def.name, def.connect.key));
    }
    if (needs.childElementCount) section.append(h("div", { class: "subhead", text: t("Keys") }), needs);

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
