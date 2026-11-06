// Settings → Permissions: what Groovy may do on this computer when asked
// (tools.rs) — each tool allowed, asked first, or off — and the log of
// everything the tools did, were asked, or were refused.

import { showError } from "../views/toast";
import { Bridge, type ToolInfo, type ToolPermission } from "../core/bridge";
import { clear, h } from "../views/dom";
import { N_, t } from "../i18n/i18n";
import { confirmClick } from "./parts";

const CHOICES: [ToolPermission, string][] = [
  ["allow", N_("Allowed")],
  ["ask", N_("Ask me first")],
  ["deny", N_("Off")],
];

export function permissionsSection(): HTMLElement {
  const list = h("div", { class: "stack" });
  const logList = h("div", { class: "stack audit" });

  function row(tool: ToolInfo): HTMLElement {
    const select = h("select", { "aria-label": t(tool.label) }) as HTMLSelectElement;
    for (const [v, label] of CHOICES) select.append(h("option", { value: v, text: t(label) }));
    select.value = tool.permission;
    select.addEventListener("change", async () => {
      try {
        await Bridge.toolsSetPermission(tool.id, select.value as ToolPermission);
        await drawLog();
      } catch {
        select.value = tool.permission;
      }
    });
    return h("div", { class: "pill-row full" }, h("div", { class: "name" }, h("div", { text: t(tool.label) }), h("div", { class: "hint", text: t(tool.description) })), select);
  }

  async function drawTools() {
    clear(list);
    try {
      for (const tool of await Bridge.toolsList()) list.append(row(tool));
    } catch (e) {
      list.append(h("div", { class: "hint", text: String(e).replace(/^Error:\s*/, "") }));
    }
  }

  async function drawLog() {
    clear(logList);
    try {
      const entries = await Bridge.toolsLog();
      if (entries.length === 0) logList.append(h("div", { class: "hint", text: t("Nothing yet.") }));
      for (const e of entries.slice(0, 60)) {
        const when = new Date(e.at * 1000).toLocaleString(undefined, { dateStyle: "short", timeStyle: "short" });
        logList.append(h("div", { class: "audit-row" }, h("span", { class: "hint", text: when }), h("code", { text: e.tool }), h("span", { text: e.outcome }), h("span", { class: "hint path", text: e.args })));
      }
    } catch (e) {
      logList.append(h("div", { class: "hint", text: String(e).replace(/^Error:\s*/, "") }));
    }
  }

  const clearLog = h("button", { class: "danger", text: t("Clear the log") }) as HTMLButtonElement;
  confirmClick(clearLog, t("Click again to clear"), async () => {
    await Bridge.toolsLogClear().catch(showError);
    await drawLog();
  });
  const refresh = h("button", { text: t("Refresh") });
  refresh.addEventListener("click", () => void drawLog());

  void drawTools();
  void drawLog();
  return h(
    "section",
    { id: "permissions" },
    h("h2", {}, h("span", { text: t("Permissions") })),
    h("div", { class: "hint", text: t("What Groovy may do on this computer when you ask in the chat or by voice. Nothing here runs commands or opens files you didn't ask for.") }),
    list,
    h("div", { class: "subhead", text: t("Activity log") }),
    logList,
    h("div", { class: "row" }, refresh, clearLog),
  );
}
