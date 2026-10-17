// Settings → System: what this computer has and what Groovy uses (system.rs),
// read when the section is drawn or refreshed, never in the background.

import { Bridge, type SystemStatus } from "../core/bridge";
import { clear, h } from "../views/dom";
import { t } from "../i18n/i18n";
import { formatBytes } from "./models";

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
  return h("section", { id: "system" }, h("h2", {}, h("span", { text: t("System") }), h("span", { class: "spacer" }), refresh), body);
}
