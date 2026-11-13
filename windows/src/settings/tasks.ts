// Settings → Reminders: to-dos and timed reminders (tasks.rs), stored on this
// computer. A reminder that comes due pops up on the island, once.

import { showError } from "../views/toast";
import { Bridge, type TaskItem } from "../core/bridge";
import { clear, h } from "../views/dom";
import { t } from "../i18n/i18n";
import { confirmClick } from "./parts";

/** A `datetime-local` value ("2026-10-15T09:30") as Unix seconds in local time; null if empty or invalid. */
export function parseLocal(value: string): number | null {
  if (!value) return null;
  const ms = new Date(value).getTime();
  return Number.isFinite(ms) ? Math.floor(ms / 1000) : null;
}

export function dueLabel(task: TaskItem, now = Date.now() / 1000): string {
  if (task.dueAt === null) return "";
  const date = new Date(task.dueAt * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
  return task.dueAt < now && !task.done ? t("overdue · {date}", { date }) : date;
}

export function tasksSection(): HTMLElement {
  const list = h("div", { class: "stack" });
  const message = h("div", {});
  const text = h("input", { type: "text", maxlength: 500, placeholder: t("e.g. Call the dentist"), style: "flex:1" }) as HTMLInputElement;
  const due = h("input", { type: "datetime-local" }) as HTMLInputElement;
  const add = h("button", { class: "primary", text: t("Add") }) as HTMLButtonElement;
  let showDone = false;

  const submit = async () => {
    if (!text.value.trim()) return;
    const dueAt = parseLocal(due.value);
    try {
      await Bridge.taskAdd(text.value, dueAt);
      text.value = "";
      due.value = "";
      clear(message);
      await draw();
    } catch (e) {
      clear(message);
      message.append(h("div", { class: "notice err", text: String(e).replace(/^Error:\s*/, "") }));
    }
  };
  add.addEventListener("click", () => void submit());
  text.addEventListener("keydown", (e) => {
    if (e.key === "Enter") void submit();
  });

  function row(task: TaskItem): HTMLElement {
    const check = h("input", { type: "checkbox", "aria-label": t("Done") }) as HTMLInputElement;
    check.checked = task.done;
    check.addEventListener("change", async () => {
      await Bridge.taskSetDone(task.id, check.checked).catch(showError);
      await draw();
    });
    const del = h("button", { class: "danger", text: t("Delete") }) as HTMLButtonElement;
    confirmClick(del, t("Click again to delete"), async () => {
      await Bridge.taskDelete(task.id).catch(showError);
      await draw();
    });
    const overdue = task.dueAt !== null && task.dueAt < Date.now() / 1000 && !task.done;
    return h(
      "div",
      { class: task.done ? "pill-row full done" : "pill-row full" },
      check,
      h("div", { class: "name" }, h("div", { text: task.text }), task.dueAt !== null ? h("div", { class: overdue ? "hint warn" : "hint", text: dueLabel(task) }) : null),
      del,
    );
  }

  const toggleDone = h("button", { text: t("Show done") }) as HTMLButtonElement;
  toggleDone.addEventListener("click", () => {
    showDone = !showDone;
    toggleDone.textContent = showDone ? t("Hide done") : t("Show done");
    void draw();
  });
  const clearDone = h("button", { text: t("Clear done") }) as HTMLButtonElement;
  clearDone.addEventListener("click", async () => {
    await Bridge.taskClearDone().catch(() => 0);
    await draw();
  });

  async function draw() {
    clear(list);
    try {
      const tasks = await Bridge.taskList(showDone);
      if (tasks.length === 0) list.append(h("div", { class: "hint", text: t("Nothing to do. Enjoy it.") }));
      for (const task of tasks) list.append(row(task));
    } catch (e) {
      list.append(h("div", { class: "notice err", text: String(e).replace(/^Error:\s*/, "") }));
    }
  }

  void draw();
  return h(
    "section",
    { id: "reminders" },
    h("h2", {}, h("span", { text: t("Reminders and to-dos") })),
    message,
    h("div", { class: "row" }, text, due, add),
    h("div", { class: "hint", text: t("Leave the time empty for a plain to-do. Reminders work offline; one that came due while Groovy was closed shows at the next start.") }),
    list,
    h("div", { class: "row" }, toggleDone, clearDone),
  );
}
