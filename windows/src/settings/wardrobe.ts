// Settings → Wardrobe Studio: Groovy's look, his Head, Face and Neck items
// (each slot on its own, with its own None — FIXES_ROUND3 item 5) and
// colours, with a live preview, search, favourites, presets, randomise,
// reset and wardrobe packs (docs/WARDROBE_PACKS.md). Every change is saved at
// once, so the real Groovy on the island and the desktop follows it live.

import { Bridge } from "../core/bridge";
import type { Settings } from "../core/state";
import { clear, h } from "../views/dom";
import { BotEngine } from "../mochi/engine";
import { ACCESSORIES, OUTFIT_KEYS, type Outfit } from "../mochi/wardrobe";
import { GROWS_INTO, SKINS, SKIN_KEYS, parseSkin } from "../mochi/skins";
import { chooseLook } from "../core/lookchoice";
import { itemSprite, lookSprite, spriteCanvas } from "../mochi/sprites";
import { MAX_STAGE, parseGrowth, progressLine, shownStage } from "../mochi/growth";
import {
  DEFAULT_APPEARANCE, SLOTS, SLOT_OF, clearSlot, combine, parseWardrobe, wear, worn as wornOf, randomLook, readPack, writePack,
  type Appearance, type Slot, type WardrobePrefs,
} from "../mochi/appearance";
import { N_, t } from "../i18n/i18n";
import { confirmClick } from "./parts";

const SLOT_LABELS: Record<Slot, string> = {
  head: N_("Head"),
  face: N_("Face"),
  neck: N_("Neck"),
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

  // ── Live preview ──
  const PREVIEW = 200;
  const preview = h("canvas", { class: "studio-preview", width: PREVIEW, height: PREVIEW, "aria-label": t("Preview") }) as HTMLCanvasElement;
  const engine = new BotEngine();
  let last = performance.now();
  let raf = 0;
  const dressPreview = () => {
    const worn = wornOf(prefs.appearance.layers);
    engine.setSkin(parseSkin(settings.groovyLook), true);
    engine.setStage(shownStage(parseGrowth(settings.growth)), true);
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

  const matches = (id: string, label: string) =>
    (!favouritesOnly || prefs.favourites.includes(id)) && (!query || folded(t(label)).includes(query) || folded(id).includes(query));

  // Drawn once, then copied (round 3B item 4: the wardrobe lagged).
  const iconCanvas = (sel: Outfit, size = 30) => spriteCanvas(itemSprite(sel, size, Math.min(2, window.devicePixelRatio || 1)), size);

  // The look (Night, Ember, Frost) and how far he has grown (docs/design/settings-v2/Groovy).
  const lookSeg = h("div", { class: "hub-w-seg", role: "radiogroup", "aria-label": t("Look") });
  function drawLooks() {
    clear(looks);
    clear(lookSeg);
    const current = parseSkin(settings.groovyLook);
    const g = parseGrowth(settings.growth);
    const shown = shownStage(g);
    for (const look of SKINS) {
      const b = h("button", { class: look === current ? "on" : "", type: "button", role: "radio", "aria-checked": String(look === current), text: t(SKIN_KEYS[look]) });
      b.addEventListener("click", () => {
        chooseLook(settings, look);
        save();
        drawAll();
      });
      lookSeg.append(b);
    }
    // The stages: Seed, 1 d, 3 d… each lit once reached.
    const marks = h("div", { class: "hub-w-stages", style: `grid-template-columns:repeat(${MAX_STAGE + 1},minmax(0,1fr))` });
    for (let i = 0; i <= MAX_STAGE; i++) {
      marks.append(h("div", { class: i <= shown ? "hub-w-stage on" : "hub-w-stage" }, h("span", {}), h("small", { text: t(STAGE_MARKS[i] ?? "") })));
    }
    growthRow.replaceChildren(
      h("div", { class: "hub-w-growth-head" },
        h("span", { class: "hub-w-growth-name", text: t("Growing into {name}", { name: t(GROWS_INTO[current]) }) }),
        h("span", { class: "hub-b-detail", text: t("Stage {n} of {max}", { n: shown, max: MAX_STAGE }) })),
      marks,
      h("span", { class: "hub-b-detail", text: progressLine(g) }),
    );
    // Favourite looks, as before, for the search.
    for (const look of SKINS) {
      if (!query && !favouritesOnly) break;
      if (!matches(look, SKIN_KEYS[look])) continue;
      const portrait = spriteCanvas(lookSprite(look, shown, 34, Math.min(2, window.devicePixelRatio || 1)), 34);
      const b = h("button", { class: look === current ? "studio-item on" : "studio-item", title: t(SKIN_KEYS[look]), "aria-pressed": look === current }, portrait, h("span", { text: t(SKIN_KEYS[look]) }), star(look)) as HTMLButtonElement;
      b.addEventListener("click", () => {
        chooseLook(settings, look);
        save();
        drawAll();
      });
      looks.append(b);
    }
  }

  // Head / Face / Neck as tabs over one grid of items, as approved.
  let slotShown: Slot = SLOTS[0];
  const slotTabs = h("div", { class: "hub-w-seg", role: "tablist", "aria-label": t("Wardrobe") });
  function drawSlots() {
    clear(slots);
    clear(slotTabs);
    const worn = wornOf(prefs.appearance.layers);
    for (const slot of SLOTS) {
      const b = h("button", { class: slot === slotShown ? "on" : "", type: "button", role: "tab", "aria-selected": String(slot === slotShown), text: t(SLOT_LABELS[slot]) });
      b.addEventListener("click", () => {
        slotShown = slot;
        drawSlots();
      });
      slotTabs.append(b);
    }
    const grid = h("div", { class: "hub-card hub-w-items" });
    const items = ACCESSORIES.filter((o) => SLOT_OF[o] === slotShown && matches(o, OUTFIT_KEYS[o]));
    // This slot's None: empties it, the others stay.
    const empty = !worn.some((w) => w !== "none" && SLOT_OF[w] === slotShown);
    const none = h("button", { class: empty ? "hub-w-item on" : "hub-w-item", type: "button", "aria-pressed": String(empty) }, iconCanvas("none", 34), h("span", { text: t("None") })) as HTMLButtonElement;
    none.addEventListener("click", () => {
      prefs = { ...prefs, appearance: { ...prefs.appearance, layers: clearSlot(prefs.appearance.layers, slotShown) } };
      commit();
    });
    grid.append(none);
    for (const item of items) {
      const on = worn.includes(item);
      const b = h("button", { class: on ? "hub-w-item on" : "hub-w-item", type: "button", title: t(OUTFIT_KEYS[item]), "aria-pressed": String(on) },
        iconCanvas(item, 34), h("span", { text: t(OUTFIT_KEYS[item]) }), star(item)) as HTMLButtonElement;
      b.addEventListener("click", () => {
        const next = wear(prefs.appearance.layers, item);
        prefs = { ...prefs, appearance: { ...prefs.appearance, layers: next.layers } };
        commit();
        if (next.did === "replaced") say(t("{item} replaces what was worn there.", { item: t(OUTFIT_KEYS[item]) }));
      });
      grid.append(b);
    }
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
        prefs = { ...prefs, appearance: p.appearance };
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
    const r = randomLook();
    settings.groovyLook = r.look;
    prefs = { ...prefs, appearance: r.appearance };
    commit();
    engine.triggerEmote("happy", 1.2);
  });
  const reset = h("button", { class: "danger", text: t("Reset to default") }) as HTMLButtonElement;
  confirmClick(reset, t("Click again: presets are kept"), () => {
    settings.groovyLook = "night";
    prefs = { ...prefs, appearance: DEFAULT_APPEARANCE };
    commit();
  });

  drawAll();
  removeAll.className = "hub-w-remove";
  const stage = h("div", { class: "hub-w-stage-box" }, preview);
  return h(
    "section",
    { id: "wardrobe", class: "hub-w", "data-own": "" },
    message,
    h("div", { class: "hub-w-grid" },
      h("div", { class: "hub-card hub-w-left", "aria-label": t("Look and growth") }, stage, lookSeg, growthRow),
      h("div", { class: "hub-w-right", "aria-label": t("Wardrobe") },
        h("div", { class: "hub-w-bar" }, slotTabs, removeAll),
        slots,
      ),
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
