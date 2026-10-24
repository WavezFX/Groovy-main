// The Coding Studio window (studio.html): a project's files, an editor with
// every save kept in the file's history, a terminal where a command runs only
// when Run is clicked, git one click at a time, and the AI — which proposes
// files and commands that are shown as diffs and Run buttons, never applied or
// run by themselves. Code goes to an online provider only after an OK.
// The backend is src-tauri/src/studio.rs.

import "./studio.css";
import { invoke } from "@tauri-apps/api/core";
import { Bridge, IS_TAURI, onEvent } from "../core/bridge";
import { DEFAULT_SETTINGS } from "../core/state";
import { fromEdit, fromNew, type FileDiff } from "../core/diff";
import { applyDocumentLanguage, isRtl, loadLanguage, onLanguageChange, resolveLanguage, setLanguage, systemLanguages, t } from "../i18n/i18n";
import { h, clear } from "../views/dom";
import { installErrorReporting } from "../views/toast";
import { languageOf, parseProblems, parseProposals, type Proposals } from "./ai";

interface Workspace { id: number; path: string; name: string; exists: boolean; git: boolean }
interface Entry { name: string; path: string; dir: boolean; size: number }
interface FileText { text: string; binary: boolean; size: number }
interface Version { id: number; reason: string; at: number; existed: boolean; size: number }
interface Hit { path: string; line: number; text: string }
interface AskTarget { provider: string; local: boolean }

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!IS_TAURI) throw new Error("not running inside Groovy");
  return invoke<T>(cmd, args);
}

const errorText = (e: unknown) => (typeof e === "string" ? e : e instanceof Error ? e.message : String(e));

const root = document.getElementById("studio-root")!;

let workspaces: Workspace[] = [];
let ws: Workspace | null = null;
let openPath: string | null = null;
let savedText = "";
const expanded = new Set<string>([""]);

const status = h("div", { class: "st-status" });
const say = (text: string, warn = false) => {
  status.textContent = text;
  status.classList.toggle("warn", warn);
};

// ── Layout ───────────────────────────────────────────────────────────────────

const wsSelect = h("select", { class: "st-select", "aria-label": t("Project") }) as HTMLSelectElement;
const tree = h("div", { class: "st-tree" });
const searchInput = h("input", { class: "st-input", placeholder: t("Search in the project") }) as HTMLInputElement;
const searchResults = h("div", { class: "st-hits" });

const fileTitle = h("div", { class: "st-file-title" });
const editor = h("textarea", { class: "st-editor", spellcheck: "false", "aria-label": t("Editor") }) as HTMLTextAreaElement;
const history = h("div", { class: "st-history" });

const cmdInput = h("input", { class: "st-input mono", placeholder: "npm test" }) as HTMLInputElement;
const output = h("pre", { class: "st-output" });
const problems = h("div", { class: "st-problems" });
let running: number | null = null;
let outputText = "";

const question = h("textarea", { class: "st-question", placeholder: t("Ask about the open file, or ask for a change…") }) as HTMLTextAreaElement;
const includeOutput = h("input", { type: "checkbox" }) as HTMLInputElement;
const answer = h("div", { class: "st-answer" });
const proposalsBox = h("div", { class: "st-proposals" });
const consentBox = h("div", {});

const gitOut = h("pre", { class: "st-output small" });
const commitMsg = h("input", { class: "st-input", placeholder: t("Commit message") }) as HTMLInputElement;

function button(text: string, onclick: () => void, kind = ""): HTMLButtonElement {
  return h("button", { class: `st-btn ${kind}`.trim(), text, onclick }) as HTMLButtonElement;
}

// ── Workspaces and files ─────────────────────────────────────────────────────

async function loadWorkspaces(select?: number) {
  workspaces = await call<Workspace[]>("studio_workspaces").catch(() => []);
  clear(wsSelect);
  if (!workspaces.length) wsSelect.append(h("option", { value: "", text: t("No project yet") }));
  for (const w of workspaces) wsSelect.append(h("option", { value: String(w.id), text: w.exists ? w.name : `${w.name} (${t("missing")})` }));
  ws = workspaces.find((w) => w.id === select) ?? workspaces.find((w) => w.exists) ?? null;
  if (ws) wsSelect.value = String(ws.id);
  expanded.clear();
  expanded.add("");
  closeFile();
  await drawTree();
}

wsSelect.addEventListener("change", () => {
  ws = workspaces.find((w) => String(w.id) === wsSelect.value) ?? null;
  expanded.clear();
  expanded.add("");
  closeFile();
  void drawTree();
});

async function drawDir(path: string, depth: number, into: HTMLElement) {
  if (!ws) return;
  let entries: Entry[];
  try {
    entries = await call<Entry[]>("studio_list", { ws: ws.id, path });
  } catch (e) {
    into.append(h("div", { class: "st-note", text: errorText(e) }));
    return;
  }
  for (const e of entries) {
    const item = h("button", {
      class: `st-entry${e.dir ? " dir" : ""}${e.path === openPath ? " on" : ""}`,
      style: `padding-inline-start:${8 + depth * 14}px`,
      text: `${e.dir ? (expanded.has(e.path) ? "▾ " : "▸ ") : ""}${e.name}`,
      title: e.path,
    });
    item.addEventListener("click", () => {
      if (e.dir) {
        if (expanded.has(e.path)) expanded.delete(e.path);
        else expanded.add(e.path);
        void drawTree();
      } else {
        void openFile(e.path);
      }
    });
    into.append(item);
    if (e.dir && expanded.has(e.path)) await drawDir(e.path, depth + 1, into);
  }
}

async function drawTree() {
  clear(tree);
  if (!ws) {
    tree.append(h("div", { class: "st-note", text: t("Open a project folder to start.") }));
    return;
  }
  if (!ws.exists) {
    tree.append(h("div", { class: "st-note", text: t("This folder is gone.") }));
    return;
  }
  await drawDir("", 0, tree);
}

function closeFile() {
  openPath = null;
  savedText = "";
  editor.value = "";
  editor.disabled = true;
  fileTitle.textContent = t("No file open");
  clear(history);
}

async function openFile(path: string, line?: number) {
  if (!ws) return;
  if (openPath && editor.value !== savedText && !confirmDiscard()) return;
  try {
    const f = await call<FileText>("studio_read", { ws: ws.id, path });
    openPath = path;
    if (f.binary) {
      editor.value = "";
      editor.disabled = true;
      fileTitle.textContent = `${path} — ${t("not a text file, or too large to edit here")}`;
    } else {
      editor.value = f.text;
      savedText = f.text;
      editor.disabled = false;
      const lang = languageOf(path);
      fileTitle.textContent = lang ? `${path} · ${lang}` : path;
      if (line) jumpTo(line);
    }
    clear(history);
    void drawTree();
  } catch (e) {
    say(errorText(e), true);
  }
}

function confirmDiscard(): boolean {
  return window.confirm(t("Discard the changes to this file?"));
}

function jumpTo(line: number) {
  const lines = editor.value.split("\n");
  const at = lines.slice(0, Math.max(0, line - 1)).reduce((n, l) => n + l.length + 1, 0);
  editor.focus();
  editor.setSelectionRange(at, at + (lines[line - 1]?.length ?? 0));
  editor.scrollTop = Math.max(0, (line - 5) * 18);
}

async function save() {
  if (!ws || !openPath || editor.disabled) return;
  try {
    await call<void>("studio_write", { ws: ws.id, path: openPath, text: editor.value, reason: "edited in the Studio" });
    savedText = editor.value;
    say(t("Saved. The previous version is in the file's history."));
  } catch (e) {
    say(errorText(e), true);
  }
}

editor.addEventListener("keydown", (e) => {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
    e.preventDefault();
    void save();
  }
  // Tab inserts spaces instead of leaving the editor.
  if (e.key === "Tab" && !e.shiftKey) {
    e.preventDefault();
    const s = editor.selectionStart;
    editor.setRangeText("  ", s, editor.selectionEnd, "end");
  }
});

async function showHistory() {
  if (!ws || !openPath) return;
  clear(history);
  const versions = await call<Version[]>("studio_history", { ws: ws.id, path: openPath }).catch((e) => {
    say(errorText(e), true);
    return [] as Version[];
  });
  if (!versions.length) history.append(h("div", { class: "st-note", text: t("No earlier version.") }));
  for (const v of versions) {
    history.append(h("div", { class: "st-row" },
      h("span", { text: `${new Date(v.at * 1000).toLocaleString()} · ${v.reason} · ${v.existed ? `${v.size} B` : t("did not exist")}` }),
      button(t("Restore"), () => {
        void call<string>("studio_revert", { ws: ws!.id, id: v.id }).then(async () => {
          say(t("Restored. The version it replaced is in the history too."));
          const p = openPath!;
          openPath = null;
          await openFile(p);
          await drawTree();
        }, (e) => say(errorText(e), true));
      })));
  }
}

async function deleteFile() {
  if (!ws || !openPath) return;
  if (!window.confirm(t("Delete this file? A copy is kept in its history."))) return;
  try {
    await call<void>("studio_delete", { ws: ws.id, path: openPath });
    closeFile();
    await drawTree();
    say(t("Deleted. Restore it from the history if you need it back."));
  } catch (e) {
    say(errorText(e), true);
  }
}

// ── Search ───────────────────────────────────────────────────────────────────

searchInput.addEventListener("keydown", (e) => {
  if (e.key !== "Enter" || !ws) return;
  const q = searchInput.value.trim();
  clear(searchResults);
  if (!q) return;
  void call<Hit[]>("studio_search", { ws: ws.id, query: q }).then((hits) => {
    if (!hits.length) searchResults.append(h("div", { class: "st-note", text: t("Nothing found.") }));
    for (const hit of hits.slice(0, 200)) {
      const b = h("button", { class: "st-hit", title: hit.text });
      b.append(h("b", { text: `${hit.path}:${hit.line}` }), h("span", { text: hit.text.trim().slice(0, 120) }));
      b.addEventListener("click", () => void openFile(hit.path, hit.line));
      searchResults.append(b);
    }
  }, (e) => say(errorText(e), true));
});

// ── Terminal ─────────────────────────────────────────────────────────────────

async function run(command: string) {
  if (!ws || !command.trim()) return;
  if (running !== null) {
    say(t("A command is already running."), true);
    return;
  }
  outputText = `$ ${command}\n`;
  output.textContent = outputText;
  clear(problems);
  try {
    // The click on Run is the approval.
    running = await call<number>("studio_run", { ws: ws.id, command, approved: true });
  } catch (e) {
    say(errorText(e), true);
  }
}

function drawProblems() {
  clear(problems);
  for (const p of parseProblems(outputText)) {
    const b = h("button", { class: "st-hit" });
    b.append(h("b", { text: `${p.path}:${p.line}` }), h("span", { text: p.message.slice(0, 140) }));
    b.addEventListener("click", () => void openFile(p.path, p.line));
    problems.append(b);
  }
}

cmdInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") void run(cmdInput.value);
});

// ── Git ──────────────────────────────────────────────────────────────────────

async function gitView(what: string) {
  if (!ws) return;
  gitOut.textContent = "…";
  gitOut.textContent = await call<string>("studio_git", { ws: ws.id, what }).catch((e) => errorText(e));
}

async function gitAction(action: string) {
  if (!ws) return;
  if (action === "push" && !window.confirm(t("Push the commits to the remote?"))) return;
  gitOut.textContent = "…";
  gitOut.textContent = await call<string>("studio_git_action", { ws: ws.id, action, message: commitMsg.value }).catch((e) => errorText(e));
  if (action === "commit") commitMsg.value = "";
  if (action === "init") await loadWorkspaces(ws.id);
}

// ── AI ───────────────────────────────────────────────────────────────────────

function drawDiff(diff: FileDiff): HTMLElement {
  const box = h("pre", { class: "st-diff" });
  if (diff.tooLarge) {
    box.textContent = `+${diff.added} −${diff.removed} (${t("too large to show")})`;
    return box;
  }
  for (const hunk of diff.hunks) {
    box.append(h("div", { class: "st-hunk", text: `@@ −${hunk.origStart} +${hunk.newStart} @@` }));
    for (const l of hunk.lines) {
      const sign = l.kind === "added" ? "+" : l.kind === "removed" ? "−" : " ";
      box.append(h("div", { class: `st-dl ${l.kind}`, text: `${sign} ${l.text}` }));
    }
  }
  return box;
}

async function drawProposals(p: Proposals) {
  clear(proposalsBox);
  if (!ws) return;
  for (const f of p.files) {
    let before: FileText | null = null;
    try {
      before = await call<FileText>("studio_read", { ws: ws.id, path: f.path });
    } catch {
      before = null;
    }
    const diff = before && !before.binary ? fromEdit(before.text, f.content, f.path) : fromNew(f.content, f.path);
    const apply = button(t("Apply"), () => {
      apply.disabled = true;
      void call<void>("studio_write", { ws: ws!.id, path: f.path, text: f.content, reason: "AI proposal" }).then(async () => {
        apply.textContent = t("Applied");
        await drawTree();
        if (openPath === f.path) {
          editor.value = f.content;
          savedText = f.content;
        }
      }, (e) => {
        apply.disabled = false;
        say(errorText(e), true);
      });
    }, "primary");
    proposalsBox.append(h("div", { class: "st-proposal" },
      h("div", { class: "st-row" }, h("b", { text: `${f.path}${before ? "" : ` (${t("new file")})`} +${diff.added} −${diff.removed}` }), apply),
      drawDiff(diff)));
  }
  for (const c of p.commands) {
    proposalsBox.append(h("div", { class: "st-row" }, h("code", { text: c }), button(t("Run"), () => {
      cmdInput.value = c;
      void run(c);
    })));
  }
  if (p.files.length || p.commands.length) {
    proposalsBox.prepend(h("div", { class: "st-note", text: t("Nothing is changed or run until you click Apply or Run.") }));
  }
}

async function ask(consented = false) {
  if (!ws) return;
  const q = question.value.trim();
  if (!q) return;
  const files = openPath ? [openPath] : [];
  const target = await call<AskTarget>("studio_ai_target").catch(() => ({ provider: "?", local: true }));
  clear(consentBox);
  if (!target.local && !consented) {
    consentBox.append(h("div", { class: "st-consent" },
      h("div", { text: files.length
        ? t("This sends your question and {0} to {1}, online.", { 0: files.join(", "), 1: target.provider })
        : t("This sends your question to {0}, online.", { 0: target.provider }) }),
      h("div", { class: "st-row" }, button(t("Send"), () => void ask(true), "primary"), button(t("Cancel"), () => clear(consentBox)))));
    return;
  }
  answer.textContent = "…";
  clear(proposalsBox);
  try {
    const full = await call<string>("studio_ask", {
      ws: ws.id, question: q, files, output: includeOutput.checked ? outputText.slice(-8000) : null, consent: consented || target.local,
    });
    answer.textContent = full;
    await drawProposals(parseProposals(full));
  } catch (e) {
    answer.textContent = "";
    say(errorText(e), true);
  }
}

// ── Boot ─────────────────────────────────────────────────────────────────────

function panel(title: string, ...children: (HTMLElement | null)[]): HTMLElement {
  return h("section", { class: "st-panel" }, h("h2", { text: title }), ...children.filter((c): c is HTMLElement => c !== null));
}

function render() {
  clear(root);
  document.title = t("Coding Studio — Groovy");
  const side = h("aside", { class: "st-side" },
    h("div", { class: "st-row" }, wsSelect),
    h("div", { class: "st-row" },
      button(t("Open folder…"), () => void call<Workspace | null>("studio_add_workspace").then((w) => w && loadWorkspaces(w.id), (e) => say(errorText(e), true))),
      button(t("New project…"), () => {
        const name = window.prompt(t("Project name"));
        if (name) void call<Workspace | null>("studio_new_project", { name }).then((w) => w && loadWorkspaces(w.id), (e) => say(errorText(e), true));
      })),
    searchInput, searchResults, tree);
  const main = h("main", { class: "st-main" },
    h("div", { class: "st-row" }, fileTitle, h("span", { class: "spacer" }),
      button(t("Save"), () => void save(), "primary"), button(t("History"), () => void showHistory()), button(t("Delete"), () => void deleteFile(), "danger")),
    editor, history,
    panel(t("Terminal"),
      h("div", { class: "st-row" }, cmdInput, button(t("Run"), () => void run(cmdInput.value), "primary"),
        button(t("Stop"), () => running !== null && void call<boolean>("studio_stop", { run: running }))),
      h("div", { class: "st-note", text: t("Commands run in the project folder only when you click Run, and stop after 10 minutes.") }),
      output, problems));
  const right = h("aside", { class: "st-ai" },
    panel(t("AI"), question,
      h("label", { class: "st-check" }, includeOutput, h("span", { text: ` ${t("Include the terminal output")}` })),
      h("div", { class: "st-row" }, button(t("Ask"), () => void ask(), "primary")),
      consentBox, answer, proposalsBox),
    panel("Git",
      h("div", { class: "st-row" }, button(t("Status"), () => void gitView("status")), button(t("Diff"), () => void gitView("diff")), button(t("Log"), () => void gitView("log"))),
      h("div", { class: "st-row" }, button("init", () => void gitAction("init")), button(t("Stage all"), () => void gitAction("stage"))),
      h("div", { class: "st-row" }, commitMsg, button(t("Commit"), () => void gitAction("commit"))),
      h("div", { class: "st-row" }, button(t("Push"), () => void gitAction("push"))),
      gitOut));
  root.append(h("div", { class: "st" }, side, main, right), status);
}

async function main() {
  installErrorReporting("page");
  const boot = await Bridge.boot();
  const settings = { ...DEFAULT_SETTINGS, ...(boot?.settings ?? {}) };
  await loadLanguage(resolveLanguage(settings.language, systemLanguages()));
  setLanguage(resolveLanguage(settings.language, systemLanguages()));
  applyDocumentLanguage();
  document.documentElement.dir = isRtl() ? "rtl" : "ltr";
  render();
  closeFile();
  onLanguageChange(() => {
    document.documentElement.dir = isRtl() ? "rtl" : "ltr";
    render();
  });
  void onEvent<{ run: number; stream: string; text: string }>("studio-output", (o) => {
    if (o.run !== running) return;
    outputText += o.text;
    if (outputText.length > 200_000) outputText = outputText.slice(-150_000);
    output.textContent = outputText;
    output.scrollTop = output.scrollHeight;
  });
  void onEvent<{ run: number; code: number | null; timedOut: boolean; seconds: number }>("studio-finished", (f) => {
    if (f.run !== running) return;
    running = null;
    outputText += `\n[${f.timedOut ? t("stopped: too long") : t("exit code {0}", { 0: String(f.code ?? "?") })} · ${f.seconds.toFixed(1)} s]\n`;
    output.textContent = outputText;
    drawProblems();
  });
  void onEvent<string>("studio-delta", (soFar) => {
    answer.textContent = soFar;
  });
  window.addEventListener("beforeunload", (e) => {
    if (openPath && editor.value !== savedText) e.preventDefault();
  });
  await loadWorkspaces();
}

void main();
