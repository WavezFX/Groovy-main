// Skills: agent skills (SKILL.md packages) for a project, through Vercel's
// `skills` CLI (src-tauri/src/skills). Pick a project, see what Groovy makes
// of it, what could help and what could go, search the skills.sh catalogue,
// edit or create a skill, undo the last change. Nothing installs before the
// user has seen the skill's SKILL.md, source, scripts and allowed tools, and
// clicked Install.

import { h, clear } from "../../views/dom";
import { t } from "../../i18n/i18n";
import { onEvent } from "../../core/bridge";
import {
  SkillsBridge, installId, pending, removeId, risks,
  type InstalledSkill, type Preview, type ProjectProfile, type Recommendations, type SkillsStatus, type VoiceConfirm,
} from "../../core/skills";
import type { HubContext } from "../shell";
import { button, card, divider, label, row, select, shortCount } from "../ui";

const LAST = "groovy.skills.project";

function lastProject(): string | null {
  try {
    return localStorage.getItem(LAST);
  } catch {
    return null;
  }
}

function rememberProject(p: string) {
  try {
    localStorage.setItem(LAST, p);
  } catch {
    /* private window: not remembered */
  }
}

function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}

const chips = (items: string[]) => h("div", { class: "hub-chips" }, ...items.map((i) => h("span", { class: "hub-chip", text: i })));

export async function skillsPage(page: HTMLElement, ctx: HubContext) {
  page.append(ctx.hero("skills", t("Skills"), t("Instruction packages for your coding agents: what this project could use, and what it could drop.")));
  const body = h("div", {});
  page.append(body);

  let status: SkillsStatus;
  try {
    status = await SkillsBridge.status();
  } catch (e) {
    body.append(h("div", { class: "hub-warn", text: errorText(e) }));
    return;
  }
  if (!status.nodeOk) {
    const found = status.nodeVersion ? t("Found {0}.", { 0: status.nodeVersion }) : t("Node.js is not installed.");
    const install = button(t("Install Node.js"), () => {
      install.disabled = true;
      void SkillsBridge.installNode().then(
        (r) => (r === "installed" ? ctx.navigate("skills") : (install.disabled = false)),
        () => (install.disabled = false),
      );
    }, "primary");
    body.append(card(
      row(t("Skills need Node.js {0} or newer", { 0: status.nodeMin }), found, install),
      h("div", { class: "hub-note", text: t("Groovy runs Vercel's skills CLI with Node.js. The Skills page stays off until it is installed.") }),
    ));
    return;
  }

  const projects = await SkillsBridge.projects().catch(() => [] as string[]);
  let project: string | null = lastProject();
  if (project && !projects.includes(project)) project = projects[0] ?? null;
  project ??= projects[0] ?? null;

  const message = h("div", { class: "hub-note" });
  const detail = h("div", {});
  const content = h("div", {});
  const say = (text: string, warn = false) => {
    message.className = warn ? "hub-warn" : "hub-note";
    message.textContent = text;
  };

  const picker = h("div", { class: "hub-row" });
  const drawPicker = () => {
    clear(picker);
    const options: [string, string][] = projects.map((p) => [p, p]);
    if (!options.length) options.push(["", t("No project yet")]);
    picker.append(
      h("div", { class: "text" }, h("div", { class: "title", text: t("Project") })),
      select(options, project ?? "", (p) => {
        project = p;
        rememberProject(p);
        void load(false);
      }),
      button(t("Pick a folder…"), () => {
        void SkillsBridge.pickProject().then((p) => {
          if (!p) return;
          if (!projects.includes(p)) projects.unshift(p);
          project = p;
          rememberProject(p);
          drawPicker();
          void load(false);
        }, (e) => say(errorText(e), true));
      }, "", true),
    );
  };
  drawPicker();
  body.append(card(picker), message, detail, content);

  // ── Preview, then install ────────────────────────────────────────────────
  const showPreview = async (source: string, skill: string, installs?: number) => {
    if (!project) return;
    clear(detail);
    say(t("Reading the skill…"));
    let p: Preview;
    try {
      p = await SkillsBridge.preview(source, skill, installs);
    } catch (e) {
      say(errorText(e), true);
      return;
    }
    say("");
    const agents = new Set(["claude-code"]);
    const agentBoxes = h("div", { class: "hub-chips" }, ...status.agents.map(([id, name]) => {
      const box = h("input", { type: "checkbox" }) as HTMLInputElement;
      box.checked = agents.has(id);
      box.addEventListener("change", () => (box.checked ? agents.add(id) : agents.delete(id)));
      return h("label", { class: "hub-chip" }, box, h("span", { text: ` ${name}` }));
    }));
    const { tools, scripts } = risks(p);
    const go = button(t("Install"), () => {
      if (!project || !agents.size) return;
      go.disabled = true;
      void SkillsBridge.install({ project, source: p.source, skills: [p.skill], agents: [...agents], scope: "project", tokens: [p.token] }).then((results) => {
        const failed = results.find((r) => r.status === "failed");
        say(failed ? (failed.error ?? t("The install failed.")) : t("{0} is installed.", { 0: p.skill }), !!failed);
        clear(detail);
        void load(false);
      }, (e) => {
        go.disabled = false;
        say(errorText(e), true);
      });
    }, "primary");
    detail.append(card(
      row(p.skill, `${p.source}${p.installs != null ? ` · ${t("{0} installs", { 0: shortCount(p.installs) })}` : ""}`),
      h("div", { class: "hub-note", text: p.url }),
      tools.length ? h("div", { class: "hub-warn", text: t("Lets agents use without asking: {0}", { 0: tools.join(", ") }) }) : null,
      scripts.length ? h("div", { class: "hub-warn", text: t("Contains files agents may run: {0}", { 0: scripts.join(", ") }) }) : null,
      divider(),
      h("pre", { class: "hub-snippet-text", style: "white-space:pre-wrap;max-height:320px;overflow:auto", text: p.content }),
      divider(),
      row(t("Install for"), null),
      agentBoxes,
      h("div", { class: "hub-note", text: t("Skills are instructions your agents will follow. Read it before installing.") }),
      h("div", { class: "hub-row" }, go, button(t("Cancel"), () => clear(detail))),
    ));
    detail.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  const confirmRemove = (skill: string) => {
    if (!project) return;
    clear(detail);
    const go = button(t("Remove"), () => {
      go.disabled = true;
      void SkillsBridge.remove(project!, [skill]).then(() => {
        say(t("{0} was removed. A copy is in .groovy/skills-trash.", { 0: skill }));
        clear(detail);
        void load(false);
      }, (e) => {
        go.disabled = false;
        say(errorText(e), true);
      });
    }, "danger");
    detail.append(card(
      row(t("Remove {0}?", { 0: skill }), t("Groovy keeps a copy first, so Undo brings it back.")),
      h("div", { class: "hub-row" }, go, button(t("Cancel"), () => clear(detail))),
    ));
    detail.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  const openEditor = async (path: string) => {
    clear(detail);
    let text: string;
    try {
      text = await SkillsBridge.read(path, project ?? undefined);
    } catch (e) {
      say(errorText(e), true);
      return;
    }
    const area = h("textarea", { class: "hub-textarea", style: "min-height:320px;font-family:monospace" }) as HTMLTextAreaElement;
    area.value = text;
    const save = button(t("Save"), () => {
      void SkillsBridge.save(path, area.value, project ?? undefined).then(() => {
        say(t("Saved. Updates will leave your version alone."));
        clear(detail);
        void load(false);
      }, (e) => say(errorText(e), true));
    }, "primary");
    detail.append(card(row(t("Edit skill"), path), area, h("div", { class: "hub-row" }, save, button(t("Cancel"), () => clear(detail)))));
    detail.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  // ── The project ──────────────────────────────────────────────────────────
  let loading = 0;
  async function load(refresh: boolean) {
    clear(content);
    if (!project) {
      content.append(h("div", { class: "hub-note", text: t("Pick a project folder to see its skills.") }));
      return;
    }
    const token = ++loading;
    say(refresh ? t("Analysing the project and searching skills.sh…") : t("Reading the project…"));
    let profile: ProjectProfile, installed: InstalledSkill[], recs: Recommendations;
    try {
      [profile, installed, recs] = await Promise.all([
        SkillsBridge.analyze(project),
        SkillsBridge.list(project),
        SkillsBridge.recommendations(project, refresh),
      ]);
    } catch (e) {
      if (token === loading) say(errorText(e), true);
      return;
    }
    if (token !== loading) return;
    say(recs.problems.length ? recs.problems.join(" ") : "", recs.problems.length > 0);
    const p = project;

    // What Groovy makes of it.
    content.append(label(t("This project")));
    const langs = profile.languages.slice(0, 5).map(([e, n]) => `.${e} ${n}`);
    content.append(card(
      row(profile.name, [...profile.frameworks, ...profile.testFrameworks].join(" · ") || t("No framework recognised")),
      langs.length ? chips(langs) : null,
      profile.activeAreas.length ? h("div", { class: "hub-note", text: t("Busy lately: {0}", { 0: profile.activeAreas.join(", ") }) }) : null,
      h("div", { class: "hub-note", text: [profile.hasClaudeMd ? "CLAUDE.md ✓" : "", profile.hasAgentsMd ? "AGENTS.md ✓" : "", ...profile.linters].filter(Boolean).join(" · ") }),
    ));

    // Recommended.
    const { install, remove } = pending(recs, installed);
    const actions = h("div", { class: "hub-row" },
      button(t("Analyse now"), () => void load(true), "", true),
      recs.canUndo ? button(t("Undo"), () => void SkillsBridge.undo(p).then(() => { say(t("Undone.")); void load(false); }, (e) => say(errorText(e), true)), "", true) : null,
      installed.some((s) => s.source) ? button(t("Update all"), () => void SkillsBridge.update(p).then((r) => {
        say(r.kept.length ? t("Updated: {0}. Kept your edited versions of: {1}.", { 0: r.updated.join(", ") || "—", 1: r.kept.join(", ") }) : t("Updated: {0}.", { 0: r.updated.join(", ") || "—" }));
        void load(false);
      }, (e) => say(errorText(e), true)), "", true) : null,
    );
    content.append(label(t("Recommended")), actions);
    const recCard = card();
    if (!install.length) recCard.append(h("div", { class: "hub-note", text: recs.at ? t("Nothing new to suggest.") : t("Click Analyse now to get suggestions.") }));
    install.forEach((r, i) => {
      if (i) recCard.append(divider());
      recCard.append(row(r.skill, `${r.reason} — ${r.source} · ${t("{0} installs", { 0: shortCount(r.installs) })} · ${Math.round(r.confidence * 100)}%`,
        button(t("Review"), () => void showPreview(r.source, r.skill, r.installs), "primary", true),
        button(t("Dismiss"), () => void SkillsBridge.dismiss(p, installId(r)).then(() => load(false)), "", true)));
    });
    if (recs.at) recCard.append(h("div", { class: "hub-note", text: recs.by === "ai" ? t("Ranked by Groovy's AI against this project.") : t("Matched by name with this project's dependencies (no AI available).") }));
    content.append(recCard);

    if (remove.length) {
      const rm = card();
      remove.forEach((r, i) => {
        if (i) rm.append(divider());
        rm.append(row(r.skill, r.reason,
          button(t("Remove"), () => confirmRemove(r.skill), "danger", true),
          button(t("Keep"), () => void SkillsBridge.dismiss(p, removeId(r.skill)).then(() => load(false)), "", true)));
      });
      content.append(label(t("Could remove")), rm);
    }

    // Installed.
    content.append(label(t("Installed")));
    const inst = card();
    if (!installed.length) inst.append(h("div", { class: "hub-note", text: t("No skills in this project yet.") }));
    installed.forEach((s, i) => {
      if (i) inst.append(divider());
      const meta = [s.source ?? t("Local"), s.agents.join(", "), s.edited ? t("edited") : ""].filter(Boolean).join(" · ");
      inst.append(row(s.name, `${s.description}${s.description ? " — " : ""}${meta}`,
        s.skillMd ? button(t("Edit"), () => void openEditor(s.skillMd!), "", true) : null,
        s.source ? button(t("Fork"), () => void SkillsBridge.fork(p, s.name).then((path) => openEditor(path), (e) => say(errorText(e), true)), "", true) : null,
        button(t("Remove"), () => confirmRemove(s.name), "danger", true)));
    });
    content.append(inst);

    // Search the catalogue.
    content.append(label(t("Search skills.sh")));
    const input = h("input", { class: "hub-input", placeholder: t("testing, react, docs…") }) as HTMLInputElement;
    const results = h("div", {});
    const search = () => {
      const q = input.value.trim();
      if (!q) return;
      clear(results);
      void SkillsBridge.search(q).then((found) => {
        if (!found.length) results.append(h("div", { class: "hub-note", text: t("No skill found.") }));
        found.forEach((c, i) => {
          if (i) results.append(divider());
          results.append(row(c.name, `${c.source} · ${t("{0} installs", { 0: shortCount(c.installs) })}`,
            installed.some((s) => s.name === c.name) ? h("span", { class: "hub-tag", text: t("Installed") }) : button(t("Review"), () => void showPreview(c.source, c.name, c.installs), "", true)));
        });
      }, (e) => say(errorText(e), true));
    };
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") search();
    });
    content.append(card(h("div", { class: "hub-row" }, input, button(t("Search"), search, "", true)), results));

    // A new skill of the project's own.
    content.append(label(t("New skill")));
    const name = h("input", { class: "hub-input", placeholder: "my-skill" }) as HTMLInputElement;
    content.append(card(
      h("div", { class: "hub-row" }, name, button(t("Create"), () => {
        void SkillsBridge.init(p, name.value.trim()).then((path) => openEditor(path), (e) => say(errorText(e), true));
      }, "", true)),
      h("div", { class: "hub-note", text: t("Created in .claude/skills in this project, for Claude Code. Lower case, digits and dashes.") }),
    ));
    content.append(h("div", { class: "hub-note", text: t("Runs Vercel's skills CLI {0} with its telemetry off. Searches go to skills.sh; only GitHub may see a token, and only yours.", { 0: status.cliVersion }) }));
  }

  // « OK Groovy » asked to install or remove: show it here to confirm.
  const confirm = (c: VoiceConfirm | null) => {
    if (!c) return;
    if (c.project && c.project !== project) {
      project = c.project;
      drawPicker();
      void load(false);
    }
    if (c.action === "remove") confirmRemove(c.skill);
    else if (c.source) void showPreview(c.source, c.skill);
    else say(t("Search for {0} below, then review it to install.", { 0: c.skill }));
  };
  const off = await onEvent<null>("skills-confirm", () => void SkillsBridge.takeConfirm().then(confirm));
  // Asked while this page was not up yet (the window was opening).
  void SkillsBridge.takeConfirm().then(confirm);
  const offChanged = await onEvent<string>("skills-changed", (p) => {
    if (p === project && !detail.childElementCount) void load(false);
  });
  ctx.onLeave = () => {
    off();
    offChanged();
  };
  await load(false);
}
