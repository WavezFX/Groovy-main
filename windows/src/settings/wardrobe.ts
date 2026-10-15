// Settings → Wardrobe Studio: Groovy's look, outfit, layered accessories and
// colours, with a live preview, search, favourites, presets, randomise,
// reset and wardrobe packs (docs/WARDROBE_PACKS.md). Every change is saved at
// once, so the real Groovy on the island and the desktop follows it live.

import { Bridge } from "../core/bridge";
import type { Settings } from "../core/state";
import { clear, h } from "../views/dom";
import { BotEngine } from "../mochi/engine";
import { drawWardrobeIcon } from "../mochi/outfits";
import { OUTFIT_KEYS, OUTFIT_SELECTIONS, parseOutfit, resolveOutfit, seasonalOutfit, WARDROBE_STRINGS, type Outfit, type OutfitSelection } from "../mochi/wardrobe";
import { SKINS, SKIN_KEYS, parseSkin } from "../mochi/skins";
import {
  DEFAULT_APPEARANCE, SLOT_OF, combine, layersOn, parseWardrobe, wear, randomLook, readPack, writePack,
  type Appearance, type Slot, type WardrobePrefs,
} from "../mochi/appearance";
import { N_, t } from "../i18n/i18n";
import { confirmClick } from "./parts";

const SLOT_LABELS: Record<Slot, string> = {
  head: N_("Head"),
  face: N_("Face"),
  neck: N_("Neck"),
  costume: N_("Costume"),
};

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
  const PREVIEW = 132;
  const preview = h("canvas", { class: "studio-preview", width: PREVIEW, height: PREVIEW, "aria-label": t("Preview") }) as HTMLCanvasElement;
  const engine = new BotEngine();
  let last = performance.now();
  let raf = 0;
  const dressPreview = () => {
    const outfit = resolveOutfit(parseOutfit(settings.mochiOutfit), new Date());
    const worn = combine(outfit, prefs.appearance.layers);
    engine.setSkin(parseSkin(settings.groovyLook), true);
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
  // ── Outfit (one), then accessories by slot ──
  const outfits = h("div", { class: "studio-grid" });
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

  const iconCanvas = (sel: OutfitSelection, size = 30) => {
    const c = h("canvas") as HTMLCanvasElement;
    const dpr = Math.min(2, window.devicePixelRatio || 1);
    c.width = size * dpr;
    c.height = size * dpr;
    c.style.width = `${size}px`;
    c.style.height = `${size}px`;
    const ctx = c.getContext("2d");
    if (ctx) {
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      drawWardrobeIcon(ctx, size, sel, seasonalOutfit(new Date()), WARDROBE_STRINGS.autoBadge);
    }
    return c;
  };

  function drawLooks() {
    clear(looks);
    const current = parseSkin(settings.groovyLook);
    for (const look of SKINS) {
      if (!matches(look, SKIN_KEYS[look])) continue;
      const portrait = h("canvas") as HTMLCanvasElement;
      const dpr = Math.min(2, window.devicePixelRatio || 1);
      portrait.width = 34 * dpr;
      portrait.height = 34 * dpr;
      portrait.style.width = "34px";
      portrait.style.height = "34px";
      const ctx = portrait.getContext("2d");
      if (ctx) {
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        const e = new BotEngine();
        e.setSkin(look, false);
        e.draw(ctx, 34, 34);
      }
      const b = h("button", { class: look === current ? "studio-item on" : "studio-item", title: t(SKIN_KEYS[look]), "aria-pressed": look === current }, portrait, h("span", { text: t(SKIN_KEYS[look]) }), star(look)) as HTMLButtonElement;
      b.addEventListener("click", () => {
        settings.groovyLook = look;
        save();
        drawAll();
      });
      looks.append(b);
    }
  }

  function drawOutfits() {
    clear(outfits);
    const current = parseOutfit(settings.mochiOutfit);
    for (const sel of OUTFIT_SELECTIONS) {
      if (!matches(sel, OUTFIT_KEYS[sel])) continue;
      const b = h("button", { class: sel === current ? "studio-item on" : "studio-item", title: t(OUTFIT_KEYS[sel]), "aria-pressed": sel === current }, iconCanvas(sel), h("span", { text: t(OUTFIT_KEYS[sel]) }), sel === "auto" || sel === "none" ? null : star(sel)) as HTMLButtonElement;
      b.addEventListener("click", () => {
        settings.mochiOutfit = sel;
        // Accessories that no longer fit with the new outfit are dropped.
        const main = resolveOutfit(sel, new Date());
        prefs = { ...prefs, appearance: { ...prefs.appearance, layers: layersOn(main, prefs.appearance.layers) } };
        commit();
      });
      outfits.append(b);
    }
  }

  function drawSlots() {
    clear(slots);
    const main = resolveOutfit(parseOutfit(settings.mochiOutfit), new Date());
    const worn = combine(main, prefs.appearance.layers);
    for (const slot of ["head", "face", "neck"] as Slot[]) {
      const row = h("div", { class: "studio-grid" });
      const items = (Object.keys(SLOT_OF) as Exclude<Outfit, "none">[]).filter((o) => SLOT_OF[o] === slot && matches(o, OUTFIT_KEYS[o]));
      if (items.length === 0) continue;
      for (const item of items) {
        const on = worn.includes(item);
        const fromOutfit = on && item === main;
        const taken = !on && worn.some((w) => SLOT_OF[w as Exclude<Outfit, "none">] === slot || SLOT_OF[w as Exclude<Outfit, "none">] === "costume");
        const b = h("button", {
          class: on ? "studio-item on" : "studio-item",
          title: fromOutfit ? t("Part of the outfit: click to keep it as your own") : taken ? t("Click to wear it instead") : t(OUTFIT_KEYS[item]),
          "aria-pressed": on,
        }, iconCanvas(item), h("span", { text: t(OUTFIT_KEYS[item]) })) as HTMLButtonElement;
        b.addEventListener("click", () => {
          const next = wear(parseOutfit(settings.mochiOutfit), prefs.appearance.layers, item, new Date());
          settings.mochiOutfit = next.selection;
          prefs = { ...prefs, appearance: { ...prefs.appearance, layers: next.layers } };
          commit();
          const name = t(OUTFIT_KEYS[item]);
          if (next.did === "kept") say(t("{item} is now yours: the outfit is set to None.", { item: name }));
          else if (next.did === "replaced") say(t("{item} replaces what was worn there.", { item: name }));
          else if (next.did === "costumeOff") say(t("The costume is off to wear {item}.", { item: name }));
        });
        row.append(b);
      }
      slots.append(h("div", { class: "subhead", text: t(SLOT_LABELS[slot]) }), row);
    }
    if (worn[0] && SLOT_OF[worn[0] as Exclude<Outfit, "none">] === "costume") {
      slots.append(h("div", { class: "hint", text: t("A costume is worn on its own: click an accessory to wear it instead.") }));
    }
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
      const preset = { name: n, look: parseSkin(settings.groovyLook), outfit: parseOutfit(settings.mochiOutfit), appearance: prefs.appearance };
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
        settings.mochiOutfit = p.outfit;
        prefs = { ...prefs, appearance: p.appearance };
        commit();
      });
      const del = h("button", { class: "danger", text: t("Delete") }) as HTMLButtonElement;
      confirmClick(del, t("Click again to delete"), () => {
        prefs = { ...prefs, presets: prefs.presets.filter((x) => x !== p) };
        commit();
      });
      presets.append(h("div", { class: "pill-row full" }, h("div", { class: "name" }, h("div", { text: p.name }), h("div", { class: "hint", text: `${t(SKIN_KEYS[p.look])} · ${t(OUTFIT_KEYS[p.outfit as OutfitSelection] ?? OUTFIT_KEYS.none)}` })), wear, del));
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
    drawOutfits();
    drawSlots();
    drawColours();
    drawPresets();
    kick();
  }

  const search = h("input", { type: "search", placeholder: t("Search the wardrobe"), style: "flex:1", "aria-label": t("Search the wardrobe") }) as HTMLInputElement;
  search.addEventListener("input", () => {
    query = folded(search.value.trim());
    drawLooks();
    drawOutfits();
    drawSlots();
  });
  const favToggle = h("button", { text: t("Favourites only"), "aria-pressed": false }) as HTMLButtonElement;
  favToggle.addEventListener("click", () => {
    favouritesOnly = !favouritesOnly;
    favToggle.classList.toggle("primary", favouritesOnly);
    favToggle.setAttribute("aria-pressed", String(favouritesOnly));
    drawLooks();
    drawOutfits();
    drawSlots();
  });
  const random = h("button", { text: t("Surprise me") });
  random.addEventListener("click", () => {
    const r = randomLook();
    settings.groovyLook = r.look;
    settings.mochiOutfit = r.outfit;
    prefs = { ...prefs, appearance: r.appearance };
    commit();
    engine.triggerEmote("happy", 1.2);
  });
  const reset = h("button", { class: "danger", text: t("Reset to default") }) as HTMLButtonElement;
  confirmClick(reset, t("Click again: presets are kept"), () => {
    settings.groovyLook = "night";
    settings.mochiOutfit = "auto";
    prefs = { ...prefs, appearance: DEFAULT_APPEARANCE };
    commit();
  });

  drawAll();
  return h(
    "section",
    { id: "wardrobe" },
    h("h2", {}, h("span", { text: t("Wardrobe Studio") })),
    message,
    h(
      "div",
      { class: "studio" },
      h("div", { class: "studio-side" }, preview, h("div", { class: "hint", text: t("Click him to say hi.") }), random, reset),
      h(
        "div",
        { class: "studio-main" },
        h("div", { class: "row" }, search, favToggle),
        h("div", { class: "subhead", text: t("Look") }),
        looks,
        h("div", { class: "subhead", text: t("Outfit") }),
        outfits,
        h("div", { class: "subhead", text: t("Accessories") }),
        slots,
        h("div", { class: "subhead", text: t("Colours") }),
        colours,
        h("div", { class: "subhead", text: t("Presets") }),
        presets,
      ),
    ),
  );
}
