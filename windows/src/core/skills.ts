// The Skills Manager, as the Control Centre sees it (src-tauri/src/skills).

import { invoke } from "@tauri-apps/api/core";

const IS_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function callOrThrow<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!IS_TAURI) throw new Error("not running inside Groovy");
  return invoke<T>(cmd, args);
}

export type Scope = "project" | "global";

export interface SkillsStatus {
  nodeVersion: string | null;
  nodeOk: boolean;
  nodeMin: string;
  cliVersion: string;
  /** [cli id, display name] */
  agents: [string, string][];
  platform: "windows" | "linux";
}

export interface InstalledSkill {
  name: string;
  path: string;
  scope: Scope;
  agents: string[];
  source: string | null;
  sourceUrl: string | null;
  sourceType: string | null;
  description: string;
  skillMd: string | null;
  edited: boolean;
  allowedTools: string[];
}

export interface Candidate {
  id: string;
  name: string;
  source: string;
  installs: number;
}

export interface Frontmatter {
  name: string | null;
  description: string | null;
  allowedTools: string[];
}

export interface Preview {
  source: string;
  skill: string;
  url: string;
  content: string;
  frontmatter: Frontmatter;
  files: string[];
  scripts: string[];
  installs: number | null;
  token: string;
}

export interface InstallRec {
  source: string;
  skill: string;
  reason: string;
  confidence: number;
  installs: number;
}

export interface Recommendations {
  install: InstallRec[];
  remove: { skill: string; reason: string }[];
  keep: string[];
  by: "ai" | "rules" | "";
  at: number;
  problems: string[];
  dismissed: string[];
  canUndo: boolean;
  ai: boolean;
}

export interface ProjectProfile {
  name: string;
  path: string;
  manifests: string[];
  dependencies: string[];
  scripts: string[];
  languages: [string, number][];
  frameworks: string[];
  testFrameworks: string[];
  linters: string[];
  hasClaudeMd: boolean;
  hasAgentsMd: boolean;
  ci: string[];
  activeAreas: string[];
  hash: string;
}

export interface AddResult {
  name: string | null;
  status: "installed" | "skipped" | "failed";
  path: string | null;
  reason: string | null;
  error: string | null;
}

export interface UpdateReport {
  updated: string[];
  kept: string[];
  output: string;
}

/** What « OK Groovy » asked to install or remove: the page confirms it. */
export interface VoiceConfirm {
  action: "install" | "remove";
  skill: string;
  source: string;
  project: string | null;
}

export const installId = (r: { source: string; skill: string }) => `install:${r.source}@${r.skill}`;
export const removeId = (skill: string) => `remove:${skill}`;

/** Recommendations still to show: not dismissed, not installed already. */
export function pending(recs: Recommendations, installed: InstalledSkill[]) {
  const have = new Set(installed.map((s) => s.name));
  return {
    install: recs.install.filter((r) => !recs.dismissed.includes(installId(r)) && !have.has(r.skill)),
    remove: recs.remove.filter((r) => !recs.dismissed.includes(removeId(r.skill)) && have.has(r.skill)),
  };
}

/** The warnings shown with a preview before installing. */
export function risks(p: Pick<Preview, "frontmatter" | "scripts">): { tools: string[]; scripts: string[] } {
  return { tools: p.frontmatter.allowedTools, scripts: p.scripts };
}

export const SkillsBridge = {
  status: () => callOrThrow<SkillsStatus>("skills_status"),
  /** What « OK Groovy » asked to install or remove, once (skills/manage.rs). */
  takeConfirm: () => callOrThrow<VoiceConfirm | null>("skills_take_confirm").catch(() => null),
  installNode: () => callOrThrow<string>("skills_install_node"),
  projects: () => callOrThrow<string[]>("skills_projects"),
  pickProject: () => callOrThrow<string | null>("skills_pick_project"),
  list: (project: string, scope: Scope = "project") => callOrThrow<InstalledSkill[]>("skills_list", { project, scope }),
  search: (query: string) => callOrThrow<Candidate[]>("skills_search", { query }),
  preview: (source: string, skill: string, installs?: number) => callOrThrow<Preview>("skills_preview", { source, skill, installs }),
  install: (request: { project: string; source: string; skills: string[]; agents: string[]; scope: Scope; tokens: string[] }) =>
    callOrThrow<AddResult[]>("skills_install", { request }),
  remove: (project: string, skills: string[], scope: Scope = "project") => callOrThrow<void>("skills_remove", { project, skills, scope }),
  update: (project: string, overwrite: string[] = []) => callOrThrow<UpdateReport>("skills_update", { project, overwrite }),
  undo: (project: string) => callOrThrow<string>("skills_undo", { project }),
  init: (project: string, name: string) => callOrThrow<string>("skills_init", { project, name }),
  fork: (project: string, skill: string) => callOrThrow<string>("skills_fork", { project, skill }),
  read: (path: string, project?: string) => callOrThrow<string>("skills_read", { path, project }),
  save: (path: string, content: string, project?: string) => callOrThrow<void>("skills_save", { path, content, project }),
  analyze: (project: string) => callOrThrow<ProjectProfile>("skills_analyze", { project }),
  recommendations: (project: string, refresh = false) => callOrThrow<Recommendations>("skills_recommendations", { project, refresh }),
  dismiss: (project: string, id: string) => callOrThrow<void>("skills_dismiss", { project, id }),
};
