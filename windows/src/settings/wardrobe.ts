// Settings → Wardrobe Studio: Groovy's look, his Head, Face and Neck items
// (each slot on its own, with its own None — FIXES_ROUND3 item 5), the
// Seasonal tab (round 5 item 4; items earned in their season, kept for good),
// items that wait for a stage (dashed, "Stage N", round 5 item 3) and
// colours, with a live preview, search, favourites, presets, randomise,
// reset and wardrobe packs (docs/WARDROBE_PACKS.md). Every change is saved at
// once, so the real Groovy on the island and the desktop follows it live.

import { Bridge } from "../core/bridge";
import type { Settings } from "../core/state";
import { clear, h } from "../views/dom";
import { BotEngine } from "../mochi/engine";
import { ACCESSORIES, OUTFIT_KEYS, available, isSeasonal, type Accessory, type Outfit } from "../mochi/wardrobe";
import { SEASONS, SEASON_ITEMS, lockLabel, parseEarned, type Season } from "../mochi/seasons";
import { GROWS_INTO, SKINS, SKIN_KEYS, parseSkin, type Skin } from "../mochi/skins";
import { chooseLook } from "../core/lookchoice";
import { itemSprite, lookSprite, spriteCanvas } from "../mochi/sprites";
import { Previewer } from "./preview";
import { MAX_STAGE, highestReached, lookGrowth, lookStarted, progressLine, shownStage, stageStrip, unlocksIn } from "../mochi/growth";
import {
  DEFAULT_APPEARANCE, SLOTS, SLOT_OF, clearSlot, combine, parseWardrobe, tryOn, wear, worn as wornOf, randomLook, readPack, writePack,
  type Appearance, type Slot, type WardrobePrefs,
} from "../mochi/appearance";
import { N_, t } from "../i18n/i18n";
import { EASE_OUT, MOTION, duration } from "../core/motion";
import { confirmClick } from "./parts";
import { DEFAULT_NAME, checkName, nameOf } from "../mochi/naming";

type Tab = Slot | "seasonal";

const TAB_LABELS: Record<Tab, string> = {
  head: N_("Head"),
  face: N_("Face"),
  neck: N_("Neck"),
  seasonal: N_("Seasonal"),
};

const SEASON_LABELS: Record<Season, string> = {
  halloween: N_("Halloween"),
  winter: N_("Winter & Christmas"),
  easter: N_("Easter"),
  summer: N_("Summer"),
};


/** The growth bar's marks: when each stage comes (docs/design/settings-v2/Groovy). */
const STAGE_MARKS = [N_("Seed"), N_("1 d"), N_("3 d"), N_("1 wk"), N_("2 wk"), N_("1 mo"), N_("6 mo")];

const COLOR_FIELDS: [keyof Pick<Appearance, "body" | "body2" | "glow" | "eyes">, string, string][] = [
  ["body", N_("Body"), "#f6fbf8"],
  ["body2", N_("Body, bottom"), "#ccdcd1"],
  ["glow", N_("Glow"), "#5aff96"],
  ["eyes", N_("Eyes"), "#46eb78"],
];

/** Lower-case, accent-free text for search. */
export function folded(s: string): string {
  return s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
}

export function wardrobeSection(settings: Settings, save: () => void): HTMLElement {
  let prefs: WardrobePrefs = parseWardrobe(settings.wardrobe);
  let query = "";
  let favouritesOnly = false;
  const message = h("div", {});
  const say = (text: string, kind: "ok" | "err" | "warn" = "ok") => {
    clear(message);
    message.append(h("div", { class: `notice ${kind}`, text }));
  };

  const commit = () => {
    settings.wardrobe = prefs;
    save();
    drawAll();
  };
  const setAppearance = (a: Appearance) => {
    prefs = { ...prefs, appearance: { ...a, layers: combine("none", a.layers) } };
    commit();
  };

  // The look the page is about (round 3H item 2): the tabs at the top choose
  // it; only "Wear this look" changes what he wears on the island.
  let viewing: Skin = parseSkin(settings.groovyLook);
  const wearing = () => parseSkin(settings.groovyLook);

  // ── Live preview ──
  const PREVIEW = 200;
  const preview = h("canvas", { class: "studio-preview", width: PREVIEW, height: PREVIEW, "aria-label": t("Preview") }) as HTMLCanvasElement;
  const engine = new BotEngine();
  let last = performance.now();
  let raf = 0;
  // A stage, a look or a locked item tried for 5 seconds (round 3E item 11):
  // the preview engine only; nothing is saved and the island never sees it.
  const previewNote = h("span", { class: "hub-w-preview-only", text: t("Preview only"), hidden: true });
  const previewer = new Previewer(() => {
    previewNote.hidden = !previewer.current?.locked;
    drawStrip();
    kick();
  });
  const dressPreview = () => {
    const p = previewer.current;
    const worn = p?.item ? tryOn(prefs.appearance.layers, p.item) : wornOf(prefs.appearance.layers);
    engine.setSkin(p?.look ?? viewing, true);
    engine.setStage(p?.stage ?? shownStage(lookGrowth(settings, viewing)), true);
    engine.setOutfit(worn[0] ?? "none", true);
    engine.layers = worn.slice(1);
    engine.appearance = prefs.appearance;
  };
  const frame = (now: number) => {
    raf = 0;
    if (!preview.isConnected) return;
    const dt = Math.min(0.05, (now - last) / 1000);
    last = now;
    const dpr = Math.min(2, window.devicePixelRatio || 1);
    if (preview.width !== PREVIEW * dpr) {
      preview.width = PREVIEW * dpr;
      preview.height = PREVIEW * dpr;
    }
    const ctx = preview.getContext("2d");
    if (ctx) {
      engine.update(dt);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, PREVIEW, PREVIEW);
      engine.draw(ctx, PREVIEW, PREVIEW);
    }
    // Draw while something moves, else a few frames a second: blinking stays alive, the CPU stays idle.
    if (engine.busy) raf = requestAnimationFrame(frame);
    else setTimeout(() => (raf = requestAnimationFrame(frame)), 250);
  };
  const kick = () => {
    dressPreview();
    if (!raf) raf = requestAnimationFrame(frame);
  };
  preview.addEventListener("click", () => engine.triggerEmote("happy", 1.2));

  // ── The stage strip (round 3E item 11): the viewed look's six stages ──
  // Small pictures, each drawn once and kept: a redraw reuses them (no
  // flicker); new ones are drawn one per frame, only while the page shows.
  const strip = h("div", { class: "hub-w-strip", "aria-label": t("Stage looks") });
  const stripNote = h("span", { class: "hub-b-detail hub-w-strip-note" });
  const stripBox = h("div", { class: "hub-w-strip-box" }, strip, stripNote);
  const stripArt = new Map<string, HTMLCanvasElement>();
  let stripQueue: (() => void)[] = [];
  const pump = () => {
    const next = stripQueue.shift();
    if (!next || !strip.isConnected) {
      stripQueue = [];
      return;
    }
    next();
    if (stripQueue.length) requestAnimationFrame(pump);
  };
  function drawStrip() {
    // Each look's own stages (round 3E item 12): Ember's are locked until Ember grows.
    const look = viewing;
    const g = lookGrowth(settings, look);
    const shown = previewer.current;
    clear(strip);
    stripQueue = [];
    const cells = stageStrip(g);
    for (const s of cells) {
      const art = h("span", { class: "hub-w-strip-art" });
      const label = s.unlocked ? t("Stage {n}", { n: s.stage }) : `${t("Stage {n}", { n: s.stage })} · ${unlocksIn(s.left ?? 0)}`;
      const on = shown?.stage === s.stage && (shown.look ?? wearing()) === look;
      const b = h("button", { class: `hub-w-strip-stage${s.unlocked ? " unlocked" : " locked"}${on ? " on" : ""}`, type: "button", title: label, "aria-label": label },
        art, h("small", { text: s.unlocked ? `✓ ${s.stage}` : `${s.stage}` })) as HTMLButtonElement;
      b.addEventListener("click", () => previewer.show({ look, stage: s.stage, locked: !s.unlocked || look !== wearing() }));
      strip.append(b);
      const key = `${look}:${s.stage}`;
      const cached = stripArt.get(key);
      if (cached) art.append(cached);
      else stripQueue.push(() => {
        const c = spriteCanvas(lookSprite(look, s.stage, 40, Math.min(2, window.devicePixelRatio || 1)), 40);
        stripArt.set(key, c);
        art.append(c);
      });
    }
    // The next stage to come, in words (the cells are too small for them).
    const next = cells.find((s) => !s.unlocked);
    stripNote.textContent = !lookStarted(settings, look)
      ? t("Not started: {look} starts growing when he wears it.", { look: t(SKIN_KEYS[look]) })
      : next ? `${t("Stage {n}", { n: next.stage })} · ${unlocksIn(next.left ?? 0)}` : t("Fully grown");
    if (stripQueue.length) requestAnimationFrame(pump);
  }

  // ── Looks ──
  const looks = h("div", { class: "studio-grid" });
  // How far he has grown, and which stage reached to show (round 3 item 6).
  const growthRow = h("div", { class: "studio-growth" });
  // ── Head, Face, Neck ──
  const slots = h("div", { class: "stack" });
  const colours = h("div", { class: "stack" });
  const presets = h("div", { class: "stack" });

  const star = (id: string) => {
    const on = prefs.favourites.includes(id);
    const b = h("button", { class: on ? "fav on" : "fav", text: on ? "★" : "☆", title: on ? t("Remove from favourites") : t("Add to favourites"), "aria-pressed": on }) as HTMLButtonElement;
    b.addEventListener("click", (e) => {
      e.stopPropagation();
      const favourites = on ? prefs.favourites.filter((f) => f !== id) : [...prefs.favourites, id];
      prefs = { ...prefs, favourites };
      commit();
    });
    return b;
  };

  // What he can wear: stages reached, seasonal items earned.
  // Keepsakes unlock at the highest stage any look has reached (round 3E item 12).
  const reached = () => highestReached(settings);
  const earned = () => parseEarned(settings.seasonalEarned);
  const can = (item: Accessory) => available(item, reached(), earned());
  const matches = (id: string, label: string) =>
    (!favouritesOnly || prefs.favourites.includes(id)) && (!query || folded(t(label)).includes(query) || folded(id).includes(query));

  // Drawn once, then copied (round 3B item 4: the wardrobe lagged).
  const iconCanvas = (sel: Outfit, size = 30) => spriteCanvas(itemSprite(sel, size, Math.min(2, window.devicePixelRatio || 1)), size);

  // The look the page is about (Night, Ember, Frost) and how far it has grown
  // (docs/design/settings-v2/Groovy; one switch, round 3H item 2).
  const lookSeg = h("div", { class: "hub-w-seg hub-w-looks", role: "tablist", "aria-label": t("Look") });
  const wearRow = h("div", { class: "hub-w-wear" });
  const settle = (el: HTMLElement) => el.animate([{ opacity: 0.35 }, { opacity: 1 }], { duration: duration(MOTION.small), easing: EASE_OUT });
  function drawLooks() {
    clear(looks);
    clear(lookSeg);
    const worn = wearing();
    const g = lookGrowth(settings, viewing);
    const shown = shownStage(g);
    const started = lookStarted(settings, viewing);
    for (const look of SKINS) {
      // Each look's own state under its name (round 3E item 12), and which one he wears.
      const state = lookStarted(settings, look) ? t("Stage {n}", { n: shownStage(lookGrowth(settings, look)) }) : t("Not started");
      const b = h("button", { class: look === viewing ? "hub-w-look on" : "hub-w-look", type: "button", role: "tab", "aria-selected": String(look === viewing) },
        h("span", { text: t(SKIN_KEYS[look]) }), h("small", { text: state }),
        look === worn ? h("em", { class: "hub-w-wearing", text: t("Wearing") }) : null);
      b.addEventListener("click", () => {
        if (look === viewing) return;
        viewing = look;
        previewer.stop();
        drawLooks();
        kick();
        settle(growthRow);
        settle(strip);
      });
      lookSeg.append(b);
    }
    // The stages: Seed, 1 d, 3 d… each lit once reached.
    const marks = h("div", { class: "hub-w-stages", style: `grid-template-columns:repeat(${MAX_STAGE + 1},minmax(0,1fr))` });
    for (let i = 0; i <= MAX_STAGE; i++) {
      marks.append(h("div", { class: started && i <= shown ? "hub-w-stage on" : "hub-w-stage" }, h("span", {}), h("small", { text: t(STAGE_MARKS[i] ?? "") })));
    }
    // Wearing it, or a button to: the row keeps its height either way.
    clear(wearRow);
    if (viewing === worn) wearRow.append(h("span", { class: "hub-w-wearing", text: t("He's wearing this look") }));
    else {
      const wearBtn = h("button", { class: "primary", type: "button", text: t("Wear this look") }) as HTMLButtonElement;
      wearBtn.addEventListener("click", () => {
        chooseLook(settings, viewing);
        save();
        drawAll();
      });
      wearRow.append(wearBtn);
    }
    growthRow.replaceChildren(
      h("div", { class: "hub-w-growth-head" },
        h("span", { class: "hub-w-growth-name", text: t("Growing into {name}", { name: t(GROWS_INTO[viewing]) }) })),
      marks,
      h("span", { class: "hub-b-detail", text: started ? progressLine(g) : t("Not started") }),
      wearRow,
    );
    drawStrip();
    // Favourite looks, as before, for the search.
    for (const look of SKINS) {
      if (!query && !favouritesOnly) break;
      if (!matches(look, SKIN_KEYS[look])) continue;
      const portrait = spriteCanvas(lookSprite(look, shownStage(lookGrowth(settings, look)), 34, Math.min(2, window.devicePixelRatio || 1)), 34);
      const b = h("button", { class: look === worn ? "studio-item on" : "studio-item", title: t(SKIN_KEYS[look]), "aria-pressed": look === worn }, portrait, h("span", { text: t(SKIN_KEYS[look]) }), star(look)) as HTMLButtonElement;
      b.addEventListener("click", () => {
        chooseLook(settings, look);
        viewing = look;
        save();
        drawAll();
      });
      looks.append(b);
    }
  }

  // Head / Face / Neck / Seasonal as tabs over one grid of items, as approved.
  let slotShown: Tab = SLOTS[0];
  const slotTabs = h("div", { class: "hub-w-seg", role: "tablist", "aria-label": t("Wardrobe") });
  const itemButton = (item: Accessory, worn: readonly Outfit[]) => {
    const on = worn.includes(item);
    const lock = lockLabel(item, reached(), earned());
    const cls = `hub-w-item${on ? " on" : ""}${lock ? " locked" : ""}`;
    const b = h("button", { class: cls, type: "button", title: lock ? `${t(OUTFIT_KEYS[item])} · ${lock}` : t(OUTFIT_KEYS[item]), "aria-pressed": String(on), "aria-disabled": lock ? "true" : "false" },
      iconCanvas(item, 34), h("span", { text: t(OUTFIT_KEYS[item]) }), lock ? h("small", { class: "hub-w-lock", text: lock }) : star(item)) as HTMLButtonElement;
    b.addEventListener("click", () => {
      // Still locked: it says when it comes, and nothing changes.
      // Still locked: tried on for 5 seconds, then it comes off (round 3E item 11).
      if (lock && !on) {
        previewer.show({ item, locked: true });
        return say(`${t(OUTFIT_KEYS[item])}: ${lock}`, "warn");
      }
      const next = wear(prefs.appearance.layers, item);
      prefs = { ...prefs, appearance: { ...prefs.appearance, layers: next.layers } };
      commit();
      if (next.did === "replaced") say(t("{item} replaces what was worn there.", { item: t(OUTFIT_KEYS[item]) }));
    });
    return b;
  };
  function drawSlots() {
    clear(slots);
    clear(slotTabs);
    const worn = wornOf(prefs.appearance.layers);
    for (const tab of [...SLOTS, "seasonal"] as const) {
      const b = h("button", { class: tab === slotShown ? "on" : "", type: "button", role: "tab", "aria-selected": String(tab === slotShown), text: t(TAB_LABELS[tab]) });
      b.addEventListener("click", () => {
        slotShown = tab;
        drawSlots();
      });
      slotTabs.append(b);
    }
    const grid = h("div", { class: "hub-card hub-w-items" });
    if (slotShown === "seasonal") {
      // Each season's four, earned while Groovy runs during it and kept for good.
      for (const season of SEASONS) {
        const items = SEASON_ITEMS[season].filter((o) => matches(o, OUTFIT_KEYS[o]));
        if (!items.length) continue;
        grid.append(h("span", { class: "hub-w-season", text: t(SEASON_LABELS[season]) }));
        for (const item of items) grid.append(itemButton(item, worn));
      }
      const auto = h("input", { type: "checkbox", checked: prefs.autoSeason }) as HTMLInputElement;
      auto.addEventListener("change", () => {
        prefs = { ...prefs, autoSeason: auto.checked };
        commit();
      });
      slots.append(grid, h("label", { class: "row hub-w-auto" }, auto, h("span", { text: t("Dress for the season: on its first day he puts on one of its hats.") })));
      return;
    }
    const slot = slotShown;
    const items = ACCESSORIES.filter((o) => SLOT_OF[o] === slot && !isSeasonal(o) && matches(o, OUTFIT_KEYS[o]));
    // This slot's None: empties it, the others stay.
    const empty = !worn.some((w) => w !== "none" && SLOT_OF[w] === slot);
    const none = h("button", { class: empty ? "hub-w-item on" : "hub-w-item", type: "button", "aria-pressed": String(empty) }, iconCanvas("none", 34), h("span", { text: t("None") })) as HTMLButtonElement;
    none.addEventListener("click", () => {
      prefs = { ...prefs, appearance: { ...prefs.appearance, layers: clearSlot(prefs.appearance.layers, slot) } };
      commit();
    });
    grid.append(none);
    for (const item of items) grid.append(itemButton(item, worn));
    slots.append(grid);
  }

  function drawColours() {
    clear(colours);
    for (const [key, label, fallback] of COLOR_FIELDS) {
      const value = prefs.appearance[key];
      const input = h("input", { type: "color", value: value || fallback, "aria-label": t(label) }) as HTMLInputElement;
      input.addEventListener("input", () => {
        engine.appearance = { ...prefs.appearance, [key]: input.value };
      });
      input.addEventListener("change", () => setAppearance({ ...prefs.appearance, [key]: input.value }));
      const reset = h("button", { text: t("Look's own"), disabled: !value }) as HTMLButtonElement;
      reset.addEventListener("click", () => setAppearance({ ...prefs.appearance, [key]: "" }));
      colours.append(h("div", { class: "row" }, h("label", { text: t(label) }), input, h("span", { class: "hint", text: value || t("from the look") }), reset));
    }
  }

  function drawPresets() {
    clear(presets);
    const name = h("input", { type: "text", maxlength: 40, placeholder: t("Preset name"), style: "flex:1" }) as HTMLInputElement;
    const saveBtn = h("button", { class: "primary", text: t("Save this look") }) as HTMLButtonElement;
    saveBtn.addEventListener("click", () => {
      const n = name.value.trim();
      if (!n) return;
      const preset = { name: n, look: parseSkin(settings.groovyLook), outfit: "none", appearance: prefs.appearance };
      prefs = { ...prefs, presets: [...prefs.presets.filter((p) => p.name !== n), preset] };
      commit();
      say(t("Saved “{name}”.", { name: n }));
    });
    presets.append(h("div", { class: "row" }, name, saveBtn));
    if (prefs.presets.length === 0) presets.append(h("div", { class: "hint", text: t("No presets yet.") }));
    for (const p of prefs.presets) {
      const wear = h("button", { text: t("Wear") }) as HTMLButtonElement;
      wear.addEventListener("click", () => {
        settings.groovyLook = p.look;
        viewing = p.look;
        // Only what he can wear now: a locked item in a preset waits.
        prefs = { ...prefs, appearance: { ...p.appearance, layers: wornOf(p.appearance.layers).filter((o) => o !== "none" && can(o)) } };
        commit();
      });
      const del = h("button", { class: "danger", text: t("Delete") }) as HTMLButtonElement;
      confirmClick(del, t("Click again to delete"), () => {
        prefs = { ...prefs, presets: prefs.presets.filter((x) => x !== p) };
        commit();
      });
      presets.append(h("div", { class: "pill-row full" }, h("div", { class: "name" }, h("div", { text: p.name }), h("div", { class: "hint", text: [t(SKIN_KEYS[p.look]), ...wornOf(p.appearance.layers).map((o) => t(OUTFIT_KEYS[o]))].join(" · ") })), wear, del));
    }
    // Packs: import a .json file, or export the presets to one.
    const file = h("input", { type: "file", accept: ".json,application/json", style: "display:none" }) as HTMLInputElement;
    file.addEventListener("change", async () => {
      const f = file.files?.[0];
      file.value = "";
      if (!f) return;
      if (f.size > 1024 * 1024) return say(t("The file is larger than 1 MB."), "err");
      const result = readPack(await f.text());
      const problems = result.problems.map((p) => t(p.msg, p.vars));
      if (result.presets.length === 0) return say(problems.join(" ") || t("No presets in this pack."), "err");
      const names = new Set(result.presets.map((p) => p.name));
      prefs = { ...prefs, presets: [...prefs.presets.filter((p) => !names.has(p.name)), ...result.presets].slice(-50) };
      commit();
      say([t("{count} presets imported.", { count: result.presets.length }), ...problems].join(" "), problems.length ? "warn" : "ok");
    });
    const importBtn = h("button", { text: t("Import a pack…") });
    importBtn.addEventListener("click", () => file.click());
    const exportBtn = h("button", { text: t("Export presets") }) as HTMLButtonElement;
    exportBtn.disabled = prefs.presets.length === 0;
    exportBtn.addEventListener("click", async () => {
      try {
        const path = await Bridge.wardrobeExport(writePack("Groovy wardrobe", prefs.presets));
        say(t("Saved to {path}", { path }));
      } catch (e) {
        say(String(e).replace(/^Error:\s*/, ""), "err");
      }
    });
    presets.append(h("div", { class: "row" }, importBtn, exportBtn, file));
  }

  function drawAll() {
    drawLooks();
    drawSlots();
    drawColours();
    drawPresets();
    kick();
  }

  const search = h("input", { type: "search", placeholder: t("Search the wardrobe"), style: "flex:1", "aria-label": t("Search the wardrobe") }) as HTMLInputElement;
  search.addEventListener("input", () => {
    query = folded(search.value.trim());
    drawLooks();
    drawSlots();
  });
  const favToggle = h("button", { text: t("Favourites only"), "aria-pressed": false }) as HTMLButtonElement;
  favToggle.addEventListener("click", () => {
    favouritesOnly = !favouritesOnly;
    favToggle.classList.toggle("primary", favouritesOnly);
    favToggle.setAttribute("aria-pressed", String(favouritesOnly));
    drawLooks();
    drawSlots();
  });
  const removeAll = h("button", { text: t("Remove all") }) as HTMLButtonElement;
  removeAll.addEventListener("click", () => {
    prefs = { ...prefs, appearance: { ...prefs.appearance, layers: [] } };
    commit();
  });
  const random = h("button", { text: t("Surprise me") });
  random.addEventListener("click", () => {
    const r = randomLook(Math.random, can);
    settings.groovyLook = r.look;
    viewing = r.look;
    prefs = { ...prefs, appearance: r.appearance };
    commit();
    engine.triggerEmote("happy", 1.2);
  });
  const reset = h("button", { class: "danger", text: t("Reset to default") }) as HTMLButtonElement;
  confirmClick(reset, t("Click again: presets are kept"), () => {
    settings.groovyLook = "night";
    viewing = "night";
    prefs = { ...prefs, appearance: DEFAULT_APPEARANCE };
    commit();
  });

  // His name (round 5 item 6): chosen at hatching, changed here at any time.
  const nameInput = h("input", { type: "text", class: "hub-w-name", value: nameOf(settings), maxlength: 40, "aria-label": t("His name"), placeholder: DEFAULT_NAME }) as HTMLInputElement;
  const nameWhy = h("span", { class: "hub-b-detail" });
  nameInput.addEventListener("change", () => {
    const check = checkName(nameInput.value);
    if (!check.ok) {
      nameWhy.textContent = t(check.why);
      return;
    }
    nameWhy.textContent = "";
    nameInput.value = check.name;
    settings.assistant = { ...settings.assistant, name: check.name };
    save();
  });
  const nameRow = h("label", { class: "hub-w-name-row" }, h("span", { class: "hub-label", text: t("His name") }), nameInput, nameWhy);

  drawAll();
  removeAll.className = "hub-w-remove";
  const stage = h("div", { class: "hub-w-stage-box" }, preview, previewNote);
  return h(
    "section",
    { id: "wardrobe", class: "hub-w", "data-own": "" },
    message,
    lookSeg,
    h("div", { class: "hub-card hub-w-top", "aria-label": t("Look and growth") },
      stage,
      h("div", { class: "hub-w-timeline" }, growthRow, nameRow),
      stripBox,
    ),
    h("div", { class: "hub-w-right", "aria-label": t("Wardrobe") },
      h("div", { class: "hub-w-bar" }, slotTabs, removeAll),
      slots,
    ),
    h("div", { class: "hub-w-more" },
      h("span", { class: "hub-label", text: t("Find") }),
      h("div", { class: "hub-card hub-w-find" }, h("div", { class: "row" }, search, favToggle), looks),
      h("span", { class: "hub-label", text: t("Colours") }),
      h("div", { class: "hub-card hub-w-pad" }, colours),
      h("span", { class: "hub-label", text: t("Presets") }),
      h("div", { class: "hub-card hub-w-pad" }, presets),
      h("div", { class: "row" }, random, reset, h("span", { class: "hint", text: t("Click him to say hi.") })),
    ),
  );
}
