// Settings → System: what this computer has and what Groovy uses (system.rs),
// read when the section is drawn or refreshed, never in the background.

import { Bridge, type SystemStatus } from "../core/bridge";
import type { Settings } from "../core/state";
import { clear, h } from "../views/dom";
import { N_, t } from "../i18n/i18n";
import { formatBytes } from "./models";
import { btn, dangerBtn } from "../hub/ui";

export const SYSTEM_STRINGS = {
  freeNow: N_("Free memory now"),
  nowPeak: N_("now · highest since Settings opened {peak}"),
  idleTarget: N_("Idle target {mb}"),
  limit: N_("Limit {mb}"),
  performance: N_("Performance"),
  help: N_("Help and diagnostics"),
  cpuLine: N_("{pct}% for the whole PC · animations pause when hidden"),
  good: N_("Good"),
  busy: N_("Busy"),
  unload: N_("Unload models when idle"),
  after: N_("After {time}"),
  keepLoaded: N_("Kept loaded"),
  notCounted: N_("Not counted"),
  notCountedLine: N_("Ollama, LM Studio and Spotify are your own apps"),
  logs: N_("Logs"),
  logsLine: N_("Kept on this PC only"),
  openFolder: N_("Open folder"),
  feedback: N_("Send feedback"),
  feedbackLine: N_("Opens a pre-filled report"),
  report: N_("Report"),
  reset: N_("Reset Groovy"),
  resetLine: N_("Keeps his growth; asks first"),
  resetButton: N_("Reset"),
  sure: N_("Sure?"),
  details: N_("This PC"),
};

const IDLE_MB = 300;
const LIMIT_MB = 400;
const MB = 1024 * 1024;
const UNLOAD: Record<string, [string, string]> = {
  "30s": [N_("30 s"), N_("30 seconds")],
  "2m": [N_("2 min"), N_("2 minutes")],
  "5m": [N_("5 min"), N_("5 minutes")],
  keep: [N_("Keep"), N_("Keep loaded")],
};

/** The memory card and the performance and help rows (docs/design/settings-v2/System, round 3D item 3). */
export function systemPage(settings: Settings, save: () => void, version: string, resetAll: () => void): HTMLElement {
  const root = h("div", { class: "hub-s", "data-own": "" });
  let peak = 0;

  const card = h("div", { class: "hub-card hub-s-mem" });
  const drawMemory = (s: SystemStatus | null) => {
    clear(card);
    const used = s?.groovyMemory ?? 0;
    peak = Math.max(peak, used);
    const scale = (b: number) => Math.min(1, b / MB / LIMIT_MB);
    const bar = h("div", { class: "hub-s-bar" },
      h("i", { class: used / MB > IDLE_MB ? "warn" : "", style: `transform:scaleX(${scale(used).toFixed(3)})` }),
      h("b", { class: "hub-s-idle", style: `left:${(IDLE_MB / LIMIT_MB) * 100}%` }),
      h("b", { class: "hub-s-limit" }));
    card.append(
      h("div", { class: "hub-s-big" }, h("strong", { text: s ? formatBytes(used) : "—" }), s ? h("span", { text: t(SYSTEM_STRINGS.nowPeak, { peak: formatBytes(peak) }) }) : null),
      bar,
      h("div", { class: "hub-s-scale" }, h("span", { text: "0" }), h("span", { class: "hub-s-mid", text: t(SYSTEM_STRINGS.idleTarget, { mb: `${IDLE_MB} MB` }) }), h("span", { text: t(SYSTEM_STRINGS.limit, { mb: `${LIMIT_MB} MB` }) })),
    );
    cpu.textContent = s ? t(SYSTEM_STRINGS.cpuLine, { pct: s.cpuUsage.toFixed(1) }) : "";
    cpuState.textContent = s && s.cpuUsage > 50 ? t(SYSTEM_STRINGS.busy) : t(SYSTEM_STRINGS.good);
    cpuState.className = s && s.cpuUsage > 50 ? "hub-s-state warn" : "hub-s-state";
  };

  const cpu = h("span", { class: "hub-b-detail" });
  const cpuState = h("span", { class: "hub-s-state" });
  const row = (name: string, line: HTMLElement | string, control: HTMLElement | null, cls = "") =>
    h("div", { class: `hub-home-row ${cls}` }, h("div", { class: "hub-b-text" }, h("span", { class: "hub-b-name", text: name }), typeof line === "string" ? h("span", { class: "hub-b-detail", text: line }) : line), control);

  const unloadKey = UNLOAD[settings.unloadAfter] ? settings.unloadAfter : "2m";
  const unload = h("select", { "aria-label": t(SYSTEM_STRINGS.unload) },
    ...Object.entries(UNLOAD).map(([v, [short]]) => h("option", { value: v, text: t(short) }))) as HTMLSelectElement;
  unload.value = unloadKey;
  const unloadLine = h("span", { class: "hub-b-detail" });
  const lineFor = (v: string) => (v === "keep" ? t(SYSTEM_STRINGS.keepLoaded) : t(SYSTEM_STRINGS.after, { time: t(UNLOAD[v][1]) }));
  unloadLine.textContent = lineFor(unloadKey);
  unload.addEventListener("change", () => {
    settings.unloadAfter = unload.value as typeof settings.unloadAfter;
    unloadLine.textContent = lineFor(unload.value);
    save();
  });

  const report = () => {
    const body = encodeURIComponent(`Groovy ${version}\n${navigator.userAgent}\n\nWhat happened:\n`);
    void Bridge.openUrl(`https://github.com/WavezFX/Groovy-main/issues/new?title=${encodeURIComponent("Feedback")}&body=${body}`);
  };

  const free = btn(t(SYSTEM_STRINGS.freeNow), async () => {
    free.disabled = true;
    await Bridge.freeMemoryNow();
    await load();
    free.disabled = false;
  });
  root.append(
    h("div", { class: "hub-s-actions" }, free),
    card,
    h("div", { class: "hub-home-two" },
      h("div", { class: "hub-home-col" }, h("span", { class: "hub-label", text: t(SYSTEM_STRINGS.performance) }),
        h("div", { class: "hub-card hub-b-list" },
          row(t("CPU"), cpu, cpuState),
          row(t(SYSTEM_STRINGS.unload), unloadLine, unload),
          row(t(SYSTEM_STRINGS.notCounted), t(SYSTEM_STRINGS.notCountedLine), null))),
      h("div", { class: "hub-home-col" }, h("span", { class: "hub-label", text: t(SYSTEM_STRINGS.help) }),
        h("div", { class: "hub-card hub-b-list" },
          row(t(SYSTEM_STRINGS.logs), t(SYSTEM_STRINGS.logsLine), btn(t(SYSTEM_STRINGS.openFolder), () => void Bridge.revealLogsFolder())),
          row(t(SYSTEM_STRINGS.feedback), t(SYSTEM_STRINGS.feedbackLine), btn(t(SYSTEM_STRINGS.report), report)),
          row(t(SYSTEM_STRINGS.reset), t(SYSTEM_STRINGS.resetLine), dangerBtn(t(SYSTEM_STRINGS.resetButton), t(SYSTEM_STRINGS.sure), resetAll), "hub-s-danger")))),
  );

  async function load() {
    drawMemory(await Bridge.systemStatus());
  }
  void load();
  // The action sits in the page header, as designed, once the page is on screen.
  requestAnimationFrame(() => {
    const head = root.closest(".hub-page")?.querySelector(".hub-head");
    if (head && !head.querySelector(".hub-head-actions")) head.append(h("div", { class: "hub-head-actions" }, free));
  });
  return root;
}

export function meter(used: number, total: number): HTMLElement {
  const pct = total > 0 ? Math.max(0, Math.min(100, (used / total) * 100)) : 0;
  const cls = pct > 90 ? "meter err" : pct > 75 ? "meter warn" : "meter";
  return h("div", { class: cls, role: "meter", "aria-valuenow": Math.round(pct), "aria-valuemin": 0, "aria-valuemax": 100 }, h("i", { style: `width:${pct.toFixed(1)}%` }));
}

export function systemSection(): HTMLElement {
  const body = h("div", { class: "stack" });
  const refresh = h("button", { text: t("Refresh") }) as HTMLButtonElement;

  function draw(s: SystemStatus, usage: number | null) {
    clear(body);
    const used = s.memoryTotal - s.memoryAvailable;
    body.append(
      h("div", { class: "hint", text: `${s.os} · ${s.cpuName} · ${t("{cpus} threads", { cpus: s.cpus })}` }),
      h("div", { class: "row" }, h("label", { text: t("CPU") }), meter(s.cpuUsage, 100), h("span", { class: "hint", text: `${Math.round(s.cpuUsage)}%` })),
      h(
        "div",
        { class: "row" },
        h("label", { text: t("Memory") }),
        meter(used, s.memoryTotal),
        h("span", { class: "hint", text: t("{used} of {total}", { used: formatBytes(used), total: formatBytes(s.memoryTotal) }) }),
      ),
      h("div", { class: "row" }, h("label", { text: t("Groovy uses") }), h("span", { class: "hint", text: formatBytes(s.groovyMemory) })),
    );
    for (const d of s.disks) {
      body.append(
        h(
          "div",
          { class: "row" },
          h("label", { text: t("Disk {name}", { name: d.mount }) }),
          meter(d.total - d.available, d.total),
          h("span", { class: "hint", text: t("{free} free", { free: formatBytes(d.available) }) }),
        ),
      );
    }
    body.append(
      h(
        "div",
        { class: "row" },
        h("label", { text: t("Battery") }),
        h("span", {
          class: "hint",
          text: s.battery ? `${s.battery.percent}%${s.battery.charging ? ` · ${t("charging")}` : ""}` : t("No battery"),
        }),
      ),
    );
    if (usage !== null) {
      body.append(h("div", { class: "row" }, h("label", { text: t("Online messages today") }), h("span", { class: "hint", text: String(usage) })));
    }
  }

  async function load() {
    refresh.disabled = true;
    const [s, usage] = await Promise.all([Bridge.systemStatus(), Bridge.usageToday()]);
    refresh.disabled = false;
    if (s) draw(s, usage);
    else {
      clear(body);
      body.append(h("div", { class: "hint", text: t("Open Groovy's settings from the app to see the system status.") }));
    }
  }
  refresh.addEventListener("click", () => void load());
  void load();
  return h("section", { id: "system" }, h("h2", {}, h("span", { text: t(SYSTEM_STRINGS.details) }), h("span", { class: "spacer" }), refresh), body);
}
