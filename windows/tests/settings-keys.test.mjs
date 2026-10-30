// Round 3D item 6: no setting was lost in the new Settings window. The 0.5.1
// settings file (tests/fixtures/settings-0.5.1.json: the one Rust's tests
// used then, every value changed from its default) loads with its values kept
// or moved where they now live, and every key of today's settings is either
// edited somewhere in the Settings window or is Groovy's own bookkeeping, with
// the reason. "Edited somewhere" is checked in the code: the page is in the
// sidebar, the file is loaded by the Settings window, and the named function
// in it writes the key (or calls the command that does).
// (settings.rs, an_old_settings_file_loads_with_its_values: Rust's side.)

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { DEFAULT_SETTINGS } from "../src/core/state.ts";
import { ALL_PAGES, TABS } from "../src/hub/nav.ts";
import { migrateSettingsOutfit, parseWardrobe, worn } from "../src/mochi/appearance.ts";
import { lookGrowth, lookStarted, migrateGrownLook, migrateToPerLook } from "../src/mochi/growth.ts";
import { WINDOWS, reachable } from "../scripts/command-access.mjs";

const OLD = JSON.parse(readFileSync("tests/fixtures/settings-0.5.1.json", "utf8"));

/**
 * Key → [page (or page:tab), file, the top-level function in it that edits
 * it, and what it writes when that isn't the key itself (a command)].
 */
const REACH = {
  soundEnabled: ["speaking:sounds", "src/settings/main.ts", "soundsSection"],
  soundVolume: ["speaking:sounds", "src/settings/main.ts", "soundsSection"],
  autoCloseInterval: ["island", "src/settings/islandpage.ts", "islandPage"],
  openOnHover: ["island", "src/settings/islandpage.ts", "islandPage"],
  activeIntegrations: ["island", "src/settings/tilesgrid.ts", "tilesCard"],
  mainPill: ["island", "src/settings/main.ts", "activePillsSection"],
  screen: ["general", "src/settings/main.ts", "generalSection"],
  autostart: ["general", "src/settings/main.ts", "generalSection"],
  islandX: ["general", "src/settings/main.ts", "generalSection", "Bridge\\.islandMove\\("],
  keepVisible: ["general", "src/settings/main.ts", "generalSection"],
  shortcuts: ["general", "src/settings/main.ts", "shortcutsSection"],
  language: ["general", "src/settings/main.ts", "languageRow"],
  hooksInstalled: ["brains:subscriptions", "src/settings/main.ts", "HOOKS_CHANGE", "Bridge\\.hooksApply"],
  showPlanInNotch: ["brains:subscriptions", "src/settings/main.ts", "planSection"],
  planRelayInstalled: ["brains:subscriptions", "src/settings/main.ts", "planSection"],
  showCodexPlanInNotch: ["brains:subscriptions", "src/settings/main.ts", "planSection"],
  model: ["brains:cloud", "src/settings/main.ts", "apiSection"],
  chatProvider: ["brains", "src/settings/brains.ts", "brainsPage"],
  chatModels: ["brains:local", "src/settings/main.ts", "localSection"],
  ollamaUrl: ["brains:local", "src/settings/main.ts", "localSection", "settings\\[field\\] = server\\.url"],
  lmstudioUrl: ["brains:local", "src/settings/main.ts", "localSection", "settings\\[field\\] = server\\.url"],
  customUrl: ["brains:local", "src/settings/main.ts", "localSection", "settings\\[field\\] = server\\.url"],
  assistant: ["brains", "src/settings/assistant.ts", "assistantSection"],
  groovyLook: ["wardrobe", "src/settings/wardrobe.ts", "wardrobeSection"],
  wardrobe: ["wardrobe", "src/settings/wardrobe.ts", "wardrobeSection"],
  growthRule: ["wardrobe", "src/settings/wardrobe.ts", "wardrobeSection"],
  pillColors: ["island", "src/settings/main.ts", "pickColor"],
  islandTiles: ["island", "src/settings/tilesgrid.ts", "tilesCard"],
  islandStyle: ["island", "src/settings/islandpage.ts", "islandPage"],
  glassStrength: ["island", "src/settings/islandpage.ts", "islandPage"],
  voiceEnabled: ["speaking", "src/settings/voice.ts", "voiceSection"],
  voiceEngine: ["speaking", "src/settings/voice.ts", "voiceSection"],
  voiceModel: ["speaking", "src/settings/voice.ts", "voiceSection"],
  livekitUrl: ["speaking", "src/settings/voice.ts", "voiceSection"],
  livekitRoom: ["speaking", "src/settings/voice.ts", "voiceSection"],
  voiceCall: ["speaking", "src/settings/voice.ts", "voiceSection"],
  voiceSpeak: ["speaking", "src/settings/voice.ts", "voiceSection"],
  voiceTts: ["speaking", "src/settings/voice.ts", "voiceSection"],
  voiceLanguage: ["speaking", "src/settings/voice.ts", "voiceSection"],
  voiceMic: ["speaking", "src/settings/voice.ts", "voiceSection"],
  voiceInterrupt: ["speaking", "src/settings/voice.ts", "voiceSection"],
  listeningView: ["speaking", "src/settings/voice.ts", "voiceSection"],
  instantVoice: ["speaking", "src/settings/voice.ts", "voiceSection"],
  callSummary: ["speaking", "src/settings/voice.ts", "voiceSection"],
  unloadAfter: ["system", "src/settings/system.ts", "systemPage"],
  wakeToName: ["listening", "src/hub/pages/voice.ts", "listeningPage"],
  doNotDisturb: ["home", "src/hub/pages/home.ts", "homePage"],
  breakWater: ["reminders:breaks", "src/settings/breaks.ts", "breaksSection"],
  breakStretch: ["reminders:breaks", "src/settings/breaks.ts", "breaksSection"],
  breakEyes: ["reminders:breaks", "src/settings/breaks.ts", "breaksSection"],
  browser: ["browser", "src/settings/main.ts", "browserSection"],
  searchEngine: ["browser", "src/settings/main.ts", "browserSection"],
  phoneLink: ["browser:phone", "src/settings/phone.ts", "phoneSection", "Bridge\\.phoneLinkSetEnabled\\("],
  phoneChat: ["browser:phone", "src/settings/phone.ts", "chatBlock", "Bridge\\.phoneChatSetEnabled\\("],
  phoneChatModels: ["browser:phone", "src/settings/phone.ts", "chatBlock", "Bridge\\.phoneChatSetModels\\("],
  phoneDetails: ["browser:phone", "src/settings/phone.ts", "detailsBlock", "Bridge\\.phoneDetailsSetEnabled\\("],
  phoneAnswers: ["browser:phone", "src/settings/phone.ts", "answersBlock", "Bridge\\.phoneAnswersSetEnabled\\("],
  phoneDiffs: ["browser:phone", "src/settings/phone.ts", "diffsBlock", "Bridge\\.phoneDiffsSetEnabled\\("],
  phoneUsage: ["browser:phone", "src/settings/phone.ts", "usageBlock", "Bridge\\.phoneUsageSetEnabled\\("],
  phoneServices: ["browser:phone", "src/settings/phone.ts", "servicesBlock", "Bridge\\.phoneServicesSet\\("],
  phoneRelay: ["browser:phone", "src/settings/phone.ts", "relayBlock", "Bridge\\.phoneRelaySetAccess\\(|Bridge\\.phoneRelaySet\\("],
  phoneRelayUrl: ["browser:phone", "src/settings/phone.ts", "relayBlock", "Bridge\\.phoneRelaySet\\("],
};

/** Groovy's own bookkeeping: no control, by design. */
const INTERNAL = {
  absenceInterval: "a timer of the Mac app; Windows Groovy never had a control for it (the island closes on leave: autoCloseInterval)",
  mochiOutfit: "0.5.1's single outfit: moved once into the wardrobe's layers (checked below)",
  lookAt: "when the look was last chosen, so two windows agree which choice is newer",
  smallSpeechSet: "which speech model set was installed, kept by the speech setup",
  closeOnLeaveOffered: "a one-time notice was shown",
  bargeInDefault: "a one-time default was applied",
  hoverDefault: "a one-time default was applied",
  growth: "the single growth record of earlier versions: moved once to the look he wears (checked below)",
  growths: "each look's growth, counted by the growth clock",
  seasonalEarned: "seasonal wardrobe items earned while Groovy ran in their season",
  hatchDone: "he hatched with a chosen look and name",
  desktopMochi: "where he sits on the desktop: set by taking him out of the island and dragging him",
};

/** The keys the Rust struct has that the page's Settings type doesn't list (Rust keeps them). */
const RUST_ONLY = ["islandX", "desktopMochi", "phoneLink", "phoneChat", "phoneChatModels", "phoneDetails", "phoneAnswers", "phoneDiffs", "phoneUsage", "phoneServices", "phoneRelay", "phoneRelayUrl"];

const pageExists = (place) => {
  const [id, tab] = place.split(":");
  return ALL_PAGES.some((p) => p.id === id) && (!tab || (TABS[id] ?? []).some((x) => x.id === tab));
};

/** The top-level function or const in `text` that holds position `at`. */
function enclosing(text, at) {
  const defs = [...text.slice(0, at).matchAll(/^(?:export )?(?:async )?function (\w+)|^(?:export )?const (\w+)\b/gm)];
  const last = defs.at(-1);
  return last ? last[1] ?? last[2] : null;
}

test("every key of today's settings is edited on a Settings page, or is Groovy's own bookkeeping", () => {
  const all = [...new Set([...Object.keys(DEFAULT_SETTINGS), ...RUST_ONLY, ...Object.keys(OLD)])];
  const missing = all.filter((k) => !(k in REACH) && !(k in INTERNAL));
  assert.deepEqual(missing, [], "a setting with no place in the Settings window");
  for (const k of Object.keys(REACH)) assert.ok(!(k in INTERNAL), `${k} is in both lists`);
  for (const k of Object.keys({ ...REACH, ...INTERNAL })) assert.ok(all.includes(k), `${k} isn't a setting any more`);
});

test("each place named is real: the page is in the sidebar, its code is loaded by Settings, and that code writes the key", () => {
  const loaded = reachable(WINDOWS.settings);
  const problems = [];
  for (const [key, [place, file, fn, writes]] of Object.entries(REACH)) {
    if (!pageExists(place)) problems.push(`${key}: no page ${place}`);
    if (!loaded.has(file)) problems.push(`${key}: ${file} isn't loaded by the Settings window`);
    const text = readFileSync(file, "utf8");
    const re = new RegExp(writes ?? `\\.${key}\\s*=[^=]|[{,(]\\s*${key}:|["'\`]${key}["'\`]`, "g");
    const where = new Set([...text.matchAll(re)].map((m) => enclosing(text, m.index)));
    if (!where.has(fn)) problems.push(`${key}: ${fn} in ${file} doesn't write it (written in: ${[...where].join(", ") || "nothing"})`);
    // The function is used, not dead code.
    const uses = [...loaded].some((f) => new RegExp(`\\b${fn}\\b`).test(readFileSync(f, "utf8").replace(new RegExp(`(?:function|const) ${fn}\\b`), "")));
    if (!uses) problems.push(`${key}: nothing calls ${fn}`);
  }
  assert.deepEqual(problems, []);
});

test("the 0.5.1 settings file loads: every value is kept, or moved where it lives now", () => {
  // As the island reads it at launch: Rust's settings (kept as they were,
  // settings.rs tests that), then the page's one-time moves.
  const s = structuredClone({ ...DEFAULT_SETTINGS, ...OLD });
  migrateSettingsOutfit(s);
  migrateGrownLook(s);
  migrateToPerLook(s);
  const moved = { mochiOutfit: "none", groovyLook: "frost" };
  for (const [k, v] of Object.entries(OLD)) {
    if (k in moved) assert.deepEqual(s[k], moved[k], k);
    else assert.deepEqual(s[k], v, `${k} kept`);
  }
  // The old outfit moved into the wardrobe: 0.5.1's witch hat is one of the
  // Mochi items the owner retired (round 3E item 7), so it becomes nothing; one
  // with a new counterpart is worn as it, on its slot. Tide is Frost grown.
  assert.deepEqual(worn(parseWardrobe(s.wardrobe).appearance.layers), []);
  const scarf = structuredClone({ ...DEFAULT_SETTINGS, ...OLD, mochiOutfit: "scarf" });
  migrateSettingsOutfit(scarf);
  assert.deepEqual(worn(parseWardrobe(scarf.wardrobe).appearance.layers), ["leafScarf"]);
  // 0.5.1 had no growth: the look he wears starts from nothing, the others aren't started.
  assert.equal(lookStarted(s, "frost"), true);
  assert.deepEqual([lookGrowth(s, "frost").reached, lookGrowth(s, "frost").activeSeconds], [0, 0]);
  assert.equal(lookStarted(s, "night") || lookStarted(s, "ember"), false);
  // Every key it had is still a setting.
  for (const k of Object.keys(OLD)) assert.ok(k in REACH || k in INTERNAL, k);
});
