// Island → Layout & tiles, as approved (docs/design/settings-v2/Island, round
// 3D item 3): a live preview of the island, the tiles to show or hide, how it
// looks and when it opens and closes, and the accounts behind its tiles. The
// detailed list (order, sizes, presets, the mail apps) follows below it.

import { Bridge, type MailInbox } from "../core/bridge";
import type { Settings } from "../core/state";
import { AUTO_CLOSE_CHOICES, CLICK_AWAY, nearestChoice } from "../core/autoclose";
import { glassStrength } from "../core/glass";
import { availablePills, sanitizeDeclared } from "../core/pills";
import { currentTiles, withServices } from "../core/tiles";
import { parseGrowth, shownStage } from "../mochi/growth";
import { parseSkin } from "../mochi/skins";
import { lookSprite, spriteCanvas } from "../mochi/sprites";
import { N_, t } from "../i18n/i18n";
import { clear, h } from "../views/dom";
import { btn, statusPill } from "../hub/ui";
import { tileMark, tileName, tilesCard } from "./tilesgrid";

export const ISLAND_STRINGS = {
  tiles: N_("Tiles"),
  feel: N_("Look and feel"),
  accounts: N_("Accounts"),
  glass: N_("Glass"),
  clear: N_("Clear"),
  balanced: N_("Balanced"),
  frosted: N_("Frosted"),
  solid: N_("Solid"),
  opens: N_("Opens"),
  onHover: N_("On hover"),
  onClick: N_("On click"),
  hoverLine: N_("On hover, after 150 ms"),
  clickLine: N_("When you click it"),
  closes: N_("Closes"),
  upNext: N_("Up next"),
  hello: N_("Good to see you"),
  helloLine: N_("Your tiles, live"),
  connect: N_("+ Connect an account"),
  noAccounts: N_("No accounts yet"),
  signIn: N_("Sign in again"),
};

export async function islandPage(settings: Settings, save: () => Promise<void> | void, redrawBelow: () => void, go: (page: string) => void, connected: Record<string, boolean> = {}): Promise<HTMLElement> {
  const root = h("div", { class: "hub-i", "data-own": "" });
  const inboxes: MailInbox[] = (await Bridge.mailInboxes()) ?? [];

  // ── Tiles: the one list (round 3E item 3), made once so an open menu or
  // the focused tile stays as the rest of the page redraws around it ──
  const grid = tilesCard(settings, save, () => {
    draw();
    redrawBelow();
  });

  const draw = () => {
    clear(root);
    const declared = sanitizeDeclared(settings);
    const cur = withServices(currentTiles(settings), declared.activeIntegrations, declared.mainPill);

    // ── Preview ──
    const look = parseSkin(settings.groovyLook);
    const portrait = h("div", { class: "hub-i-face" });
    requestAnimationFrame(() => portrait.append(spriteCanvas(lookSprite(look, shownStage(parseGrowth(settings.growth)), 38, Math.min(2, window.devicePixelRatio || 1)), 38)));
    const shown = cur.tiles.slice(0, 5);
    const solid = settings.islandStyle === "solid";
    const strength = glassStrength(settings.glassStrength);
    const glass = solid ? "#15181c" : strength === "clear" ? "rgba(30,34,40,0.25)" : strength === "frosted" ? "rgba(22,26,32,0.62)" : "rgba(22,26,32,0.45)";
    root.append(h("div", { class: "hub-i-preview", "aria-label": t("Preview") },
      h("div", { class: "hub-i-island", style: `background:${glass}` },
        portrait,
        h("div", { class: "hub-i-words" }, h("span", { class: "hub-i-up", text: t(ISLAND_STRINGS.upNext) }), h("span", { class: "hub-b-name", text: t(ISLAND_STRINGS.hello) }), h("span", { class: "hub-b-detail", text: t(ISLAND_STRINGS.helloLine) })),
        ...shown.map((x) => h("div", { class: "hub-i-mini" }, tileMark(x, 18), h("small", { text: tileName(x).split(" ")[0] }))),
      ),
    ));

    // ── Look and feel ──
    const seg = h("div", { class: "hub-segmented", role: "radiogroup", "aria-label": t(ISLAND_STRINGS.glass) });
    const current = solid ? "solid" : strength;
    for (const [id, label] of [["clear", ISLAND_STRINGS.clear], ["balanced", ISLAND_STRINGS.balanced], ["frosted", ISLAND_STRINGS.frosted], ["solid", ISLAND_STRINGS.solid]] as const) {
      const b = h("button", { class: id === current ? "on" : "", type: "button", role: "radio", "aria-checked": String(id === current), text: t(label) });
      b.addEventListener("click", () => {
        if (id === "solid") settings.islandStyle = "solid";
        else {
          settings.islandStyle = "glass";
          settings.glassStrength = id;
        }
        void save();
        draw();
      });
      seg.append(b);
    }
    const opens = h("select", { "aria-label": t(ISLAND_STRINGS.opens) }, h("option", { value: "hover", text: t(ISLAND_STRINGS.onHover) }), h("option", { value: "click", text: t(ISLAND_STRINGS.onClick) })) as HTMLSelectElement;
    opens.value = settings.openOnHover === false ? "click" : "hover";
    opens.addEventListener("change", () => {
      settings.openOnHover = opens.value === "hover";
      void save();
      draw();
    });
    const closes = h("select", { "aria-label": t(ISLAND_STRINGS.closes) },
      ...AUTO_CLOSE_CHOICES.map((c) => h("option", { value: String(c), text: c === 0 ? t("When I leave") : c === CLICK_AWAY ? t("When I click away") : t("{seconds} s after I leave", { seconds: c }) })),
    ) as HTMLSelectElement;
    closes.value = String(nearestChoice(settings.autoCloseInterval));
    closes.addEventListener("change", () => {
      settings.autoCloseInterval = nearestChoice(Number(closes.value));
      void save();
    });
    const feelRow = (name: string, line: string | null, control: HTMLElement) =>
      h("div", { class: "hub-home-row" }, h("div", { class: "hub-b-text" }, h("span", { class: "hub-b-name", text: name }), line ? h("span", { class: "hub-b-detail", text: line }) : null), control);
    const feel = h("div", { class: "hub-card hub-b-list" },
      feelRow(t(ISLAND_STRINGS.glass), null, seg),
      feelRow(t(ISLAND_STRINGS.opens), settings.openOnHover === false ? t(ISLAND_STRINGS.clickLine) : t(ISLAND_STRINGS.hoverLine), opens),
      feelRow(t(ISLAND_STRINGS.closes), null, closes),
    );

    // ── Accounts ──
    const accounts = h("div", { class: "hub-card hub-b-list" });
    accounts.append(h("div", { class: "hub-home-row hub-i-none" }, h("span", { class: "hub-b-detail", text: t(ISLAND_STRINGS.noAccounts) })));
    for (const box of inboxes) {
      const name = box.account.kind === "gmail" ? "Gmail" : box.account.kind === "outlook" ? "Outlook" : box.account.kind === "icloud" ? "iCloud Mail" : t("Mail");
      const color = box.account.kind === "gmail" ? "#E5484D" : box.account.kind === "outlook" ? "#2F6FD1" : "#4C8DF6";
      accounts.append(h("div", { class: "hub-i-account" },
        h("span", { class: "hub-i-acc-swatch", style: `background:${color}` }),
        h("div", { class: "hub-b-text" }, h("span", { class: "hub-b-name", text: name }), h("span", { class: "hub-b-detail", text: box.account.email })),
        box.error ? statusPill("warn", t(ISLAND_STRINGS.signIn)) : statusPill("connected"),
      ));
    }
    // The services behind the tiles that are signed in (a key or a hook).
    for (const p of availablePills().filter((p) => connected[p.id] && p.category !== "workspace")) {
      accounts.append(h("div", { class: "hub-i-account" },
        h("span", { class: "hub-i-acc-swatch", style: `background:${p.color}` }),
        h("div", { class: "hub-b-text" }, h("span", { class: "hub-b-name", text: p.name })),
        statusPill("connected"),
      ));
    }
    if (inboxes.length || accounts.childElementCount > 1) accounts.querySelector(".hub-i-none")?.remove();
    accounts.append(h("div", { class: "hub-i-connect" }, btn(t(ISLAND_STRINGS.connect), () => go("accounts"))));

    root.append(h("span", { class: "hub-label", text: t(ISLAND_STRINGS.tiles) }), grid);
    root.append(h("div", { class: "hub-home-two" },
      h("div", { class: "hub-home-col" }, h("span", { class: "hub-label", text: t(ISLAND_STRINGS.feel) }), feel),
      h("div", { class: "hub-home-col" }, h("span", { class: "hub-label", text: t(ISLAND_STRINGS.accounts) }), accounts),
    ));
  };
  draw();
  return root;
}
