// Settings → Home (round 3D item 1): Groovy at a glance. His look and stage,
// which brain answers, whether he's listening, how much memory he uses, three
// quick toggles, and anything that needs the user, each with a Fix button.

import { Bridge } from "../../core/bridge";
import { FlowBridge } from "../../core/flow";
import { parseGrowth, progressLine, shownStage } from "../../mochi/growth";
import { lookSprite, spriteCanvas } from "../../mochi/sprites";
import { GROWS_INTO, SKIN_KEYS, parseSkin } from "../../mochi/skins";
import { t } from "../../i18n/i18n";
import { h } from "../../views/dom";
import type { HubContext } from "../shell";
import { button, card, divider, label, row, toggle } from "../ui";

export interface Warning {
  text: string;
  fix: string;
  /** Where Fix goes: a page id (nav.ts), with a tab. */
  page: string;
  tab?: string;
}

/** Everything on Home that wants attention, most important first. */
export function warnings(input: {
  aiReady: boolean;
  okOn: boolean;
  micError: string | null;
  speechReady: boolean;
  dictationOn: boolean;
}): Warning[] {
  const out: Warning[] = [];
  if (!input.aiReady) out.push({ text: t("No AI is set up: Groovy can only do the basics."), fix: t("Set up a brain"), page: "brains" });
  if ((input.okOn || input.dictationOn) && !input.speechReady) out.push({ text: t("The speech model isn't downloaded: he can't hear you yet."), fix: t("Download it"), page: "speechsetup" });
  if (input.okOn && input.micError) out.push({ text: t("The microphone didn't open: {error}", { error: input.micError }), fix: t("Choose a microphone"), page: "calls" });
  return out;
}

const mb = (bytes: number) => `${Math.round(bytes / (1024 * 1024))} MB`;

export async function homePage(page: HTMLElement, ctx: HubContext) {
  const [status, boot, quick, sys] = await Promise.all([ctx.refresh(), Bridge.boot(), FlowBridge.quickStatus(), Bridge.systemStatus()]);
  const settings = boot?.settings;
  const name = "Groovy";
  page.append(ctx.hero("home", t("Home"), t("Groovy at a glance, and anything that needs you.")));

  // ── Him ──
  const look = parseSkin(settings?.groovyLook);
  const g = parseGrowth(settings?.growth);
  const portrait = h("span", { class: "hub-home-portrait" });
  requestAnimationFrame(() => portrait.append(spriteCanvas(lookSprite(look, shownStage(g), 56, Math.min(2, window.devicePixelRatio || 1)), 56)));
  page.append(card(
    h("div", { class: "hub-home-him" }, portrait, h("div", {},
      h("div", { class: "hub-home-name", text: name }),
      h("div", { class: "hub-note", text: `${t(SKIN_KEYS[look])} → ${t(GROWS_INTO[look])} · ${progressLine(g)}` }),
    ), button(t("Wardrobe"), () => ctx.navigate("wardrobe"), "", true)),
  ));

  // ── Brain, voice, memory ──
  const ready = quick?.ai.find((a) => a.state === "ready") ?? null;
  const ok = quick?.ok;
  const speechReady = status.models.parakeet.state === "ready";
  page.append(label(t("Now")));
  page.append(card(
    row(t("Brain"), ready ? t("{name} answers.", { name: ready.name }) : t("No AI yet."), button(t("Brains"), () => ctx.navigate("brains"), "", true)),
    divider(),
    row(t("Voice"), ok?.on ? (ok.listening ? t("Listening for « OK Groovy ».") : t("« OK Groovy » is on, not listening yet.")) : t("« OK Groovy » is off."), button(t("Listening"), () => ctx.navigate("listening"), "", true)),
    divider(),
    row(t("Memory use"), sys ? t("Groovy uses {mb} of {total}.", { mb: mb(sys.groovyMemory), total: mb(sys.memoryTotal) }) : "…", button(t("System"), () => ctx.navigate("system"), "", true)),
  ));

  // ── Quick toggles ──
  page.append(label(t("Quick toggles")));
  page.append(card(
    row("« OK Groovy »", null, toggle(status.prefs.voice, (on) => void ctx.setPrefs((p) => (p.voice = on)))),
    divider(),
    row(t("Dictation"), null, toggle(status.prefs.enabled, (on) => void ctx.setPrefs((p) => (p.enabled = on)))),
    divider(),
    row(t("Do not disturb"), t("Windows' own Focus: notifications wait."), button(t("Open"), () => void Bridge.pcDo("dnd", null, null).catch(() => {}), "", true)),
  ));

  // ── Needs you ──
  const list = warnings({ aiReady: !!ready, okOn: !!ok?.on, micError: ok?.micError ?? null, speechReady, dictationOn: status.prefs.enabled });
  page.append(label(t("Needs you")));
  page.append(list.length
    ? card(...list.flatMap((w, i) => [i ? divider() : null, row(w.text, null, button(w.fix, () => ctx.navigate(w.page, w.tab), "primary", true))]))
    : card(h("div", { class: "hub-empty" }, h("span", { class: "hub-empty-icon", text: "✓" }), h("span", { text: t("Nothing needs you. All set.") }))));
}
