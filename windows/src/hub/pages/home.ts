// Settings → Home (round 3D item 1): Groovy at a glance. His look and stage,
// which brain answers, whether he's listening, how much memory he uses, three
// quick toggles, and anything that needs the user, each with a Fix button.

import { Bridge } from "../../core/bridge";
import { FlowBridge } from "../../core/flow";
import { lookGrowth, parseGrowth, progressLine, shownStage } from "../../mochi/growth";
import { lookSprite, spriteCanvas } from "../../mochi/sprites";
import { GROWS_INTO, parseSkin } from "../../mochi/skins";
import { t } from "../../i18n/i18n";
import { h } from "../../views/dom";
import type { HubContext } from "../shell";
import { btn, emptyState, keyLabel, toggle } from "../ui";

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


/** "Claude", "Gemini"… with where it runs, for "Thinking with". */
function brainKind(name: string): string {
  const n = name.toLowerCase();
  if (n.includes("code") || n.includes("codex")) return t("subscription");
  if (n.includes("local") || n.includes("ollama") || n.includes("pc")) return t("this PC");
  return t("cloud");
}

function tile(labelText: string, value: HTMLElement | string, detail: HTMLElement | string | null): HTMLElement {
  return h("div", { class: "hub-home-tile" },
    h("span", { class: "hub-label", text: labelText }),
    typeof value === "string" ? h("span", { class: "hub-home-tile-value", text: value }) : value,
    detail ? (typeof detail === "string" ? h("span", { class: "hub-home-tile-detail", text: detail }) : detail) : null,
  );
}

/** A row of a Home card: a dot (Needs attention) or nothing, the words, the control. */
function homeRow(name: string, detail: string | null, control: HTMLElement, dot?: string): HTMLElement {
  return h("div", { class: "hub-home-row" },
    dot ? h("span", { class: "hub-home-dot", style: `background:${dot}` }) : null,
    h("div", { class: "hub-b-text" }, h("span", { class: "hub-b-name", text: name }), detail ? h("span", { class: "hub-b-detail", text: detail }) : null),
    control,
  );
}

export async function homePage(page: HTMLElement, ctx: HubContext) {
  const [status, boot, quick, sys] = await Promise.all([ctx.refresh(), Bridge.boot(), FlowBridge.quickStatus(), Bridge.systemStatus()]);
  const settings = boot?.settings;
  const version = h("span", { class: "hub-home-version", text: boot?.version ? `Groovy ${boot.version}` : "Groovy" });
  page.append(ctx.hero("home", t("Home"), null, [version]));

  // ── Him: his look, how far he has grown, and what runs him ──
  const look = parseSkin(settings?.groovyLook);
  const g = settings ? lookGrowth(settings) : parseGrowth(undefined);
  const portrait = h("span", { class: "hub-home-portrait" });
  requestAnimationFrame(() => portrait.append(spriteCanvas(lookSprite(look, shownStage(g), 150, Math.min(2, window.devicePixelRatio || 1)), 150)));
  const ready = (quick?.ai ?? []).filter((x) => x.state === "ready");
  const thinking = ready[0]
    ? tile(t("Thinking with"), `${ready[0].name} · ${brainKind(ready[0].name)}`, ready.length > 1 ? t("Falls back to {list}", { list: ready.slice(1, 3).map((x) => x.name).join(t(", then ")) }) : null)
    : tile(t("Thinking with"), t("No AI yet"), t("Set one up in Brains"));
  const voice = quick?.voice ?? "system";
  const speaking = tile(t("Speaking with"),
    voice === "fish" ? "Fish Audio" : voice === "kokoro" ? t("Kokoro · offline") : t("System voice"),
    voice === "fish" ? t("Kokoro offline if needed") : null);
  const used = sys ? sys.groovyMemory / (1024 * 1024) : 0;
  const memValue = h("span", { class: "hub-home-tile-value" }, `${Math.round(used)} MB `, h("span", { class: "hub-home-of", text: t("of 400 MB limit") }));
  const memBar = h("div", { class: "hub-home-bar" }, h("i", { style: `transform:scaleX(${Math.min(1, used / 400).toFixed(3)})` }));
  page.append(h("div", { class: "hub-card hub-home-him", "aria-label": "Groovy" },
    portrait,
    h("div", { class: "hub-home-him-text" },
      h("div", { class: "hub-home-him-head" },
        h("div", { class: "hub-home-title", text: t("Groovy is growing into {next}", { next: t(GROWS_INTO[look]) }) }),
        h("div", { class: "hub-home-line", text: progressLine(g) }),
      ),
      h("div", { class: "hub-home-tiles" }, thinking, speaking, tile(t("Memory"), memValue, memBar)),
    ),
    btn(t("Wardrobe"), () => ctx.navigate("wardrobe")),
  ));

  // ── Quick toggles · Needs attention ──
  const flip = (on: boolean, change: (v: boolean) => void) => toggle(on, change);
  const s = settings;
  const toggles = h("div", { class: "hub-card hub-b-list" },
    homeRow("OK Groovy", t("Listen for the wake word"), flip(status.prefs.voice, (on) => void ctx.setPrefs((p) => (p.voice = on)))),
    homeRow(t("Dictation"), t("Hold {key} to talk", { key: keyLabel(status.prefs.key, status.platform) }), flip(status.prefs.enabled, (on) => void ctx.setPrefs((p) => (p.enabled = on)))),
    homeRow(t("Do not disturb"), t("No toasts or sounds"), flip(!!s?.doNotDisturb, (on) => {
      if (!s) return;
      s.doNotDisturb = on;
      void Bridge.saveSettings(s);
    })),
    homeRow(t("Open island on hover"), t("Otherwise open on click"), flip(s?.openOnHover !== false, (on) => {
      if (!s) return;
      s.openOnHover = on;
      void Bridge.saveSettings(s);
    })),
  );
  const ok = quick?.ok;
  const speechReady = status.models.parakeet.state === "ready" || status.engineReady;
  const list = warnings({ aiReady: ready.length > 0, okOn: !!ok?.on, micError: ok?.micError ?? null, speechReady, dictationOn: status.prefs.enabled });
  const attention = h("div", { class: "hub-card hub-b-list" }, ...(list.length
    ? list.map((w, i) => homeRow(w.text, null, btn(w.fix, () => ctx.navigate(w.page, w.tab), i === 0 ? "primary" : "secondary"), i === 0 ? "#F5B84B" : "#7FD4FF"))
    : [emptyState("check", t("Nothing needs you. All set."))]));
  page.append(h("div", { class: "hub-home-two" },
    h("div", { class: "hub-home-col", "aria-label": t("Quick toggles") }, h("span", { class: "hub-label", text: t("Quick toggles") }), toggles),
    h("div", { class: "hub-home-col", "aria-label": t("Needs attention") }, h("span", { class: "hub-label", text: t("Needs attention") }), attention),
  ));

  // ── Jump to ──
  const jump = (page_: string, name: string, line: string) =>
    h("button", { class: "hub-card hub-home-jump", type: "button", onclick: () => ctx.navigate(page_) },
      h("span", { class: "hub-home-jump-name", text: name }), h("span", { class: "hub-b-detail", text: line }));
  page.append(h("div", { class: "hub-home-col", "aria-label": t("Jump to") },
    h("span", { class: "hub-label", text: t("Jump to") }),
    h("div", { class: "hub-home-jumps" },
      jump("brains", t("Brains"), t("Cloud, subscriptions, on this PC")),
      jump("speaking", t("Voice"), t("How he sounds and listens")),
      jump("island", t("Island"), t("Tiles, glass and accounts")),
      jump("system", t("System"), t("Memory, CPU and logs")),
    ),
  ));
}
