// Thin wrapper over the Tauri commands/events. Every call is a no-op when the
// page is opened in a plain browser, so the island can be iterated on with
// `npm run dev` alone.

import { invoke } from "@tauri-apps/api/core";
import { emitTo, listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { Settings } from "./state";
import type { RecapHistory, RecapPrefs } from "../recap/summary";
import type { SpotifyState } from "./spotify";
import { showError } from "../views/toast";

export const IS_TAURI =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/**
 * Commands run because the user clicked something: when one fails, the error
 * is shown (views/toast.ts). Every other call() is background work — polling,
 * reading state — and fails quietly into null. callOrThrow() leaves the error
 * to the caller.
 */
const USER_ACTIONS = new Set([
  "save_settings", "open_url", "reveal_sounds_folder", "reload_sounds", "open_in_vscode", "open_session",
  "open_claude_desktop", "open_file_in_vscode", "quit_app", "open_settings_window", "approval_decision",
  "approval_answer", "approval_decline", "models_cancel_download", "models_stop", "models_reveal_folder",
  "open_studio_window", "studio_stop", "music_control", "music_seek", "music_volume", "music_set_mode",
  "chat_reset", "local_set_key", "refresh_integration", "github_refresh", "set_paused", "recap_set_enabled",
  "recap_set_hide_projects", "recap_clear", "recap_reveal_saved", "desktop_mochi_fly_out", "desktop_mochi_fly_home",
  "spotify_control", "spotify_open", "phone_link_send_diff",
]);

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T | null> {
  if (!IS_TAURI) return null;
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    console.error(`[groovy] ${cmd} failed`, err);
    if (USER_ACTIONS.has(cmd)) showError(err);
    return null;
  }
}

// ── Assistant: memory, history, reminders, local models, system ──────────────

export type MemoryKind = "preference" | "fact" | "project" | "task";

export interface MemoryEntry {
  id: number;
  kind: MemoryKind;
  content: string;
  pinned: boolean;
  createdAt: number;
  updatedAt: number;
}

export interface ConversationInfo {
  id: number;
  title: string;
  startedAt: number;
  updatedAt: number;
  messages: number;
}

export interface HistoryMessage {
  role: "user" | "assistant";
  content: string;
  provider: string;
  createdAt: number;
}

export interface TaskItem {
  id: number;
  text: string;
  /** Unix seconds; null for a plain to-do. */
  dueAt: number | null;
  done: boolean;
  notified: boolean;
  createdAt: number;
}

export interface GgufInfo {
  version: number;
  architecture: string;
  name: string;
  quantization: string;
  contextLength: number;
  blockCount: number;
  embeddingLength: number;
  headCount: number;
  headCountKv: number;
  hasChatTemplate: boolean;
}

export interface LocalModelInfo {
  file: string;
  path: string;
  size: number;
  info: GgufInfo | null;
  error: string | null;
  ramNeeded: number;
  catalogId: string | null;
}

export interface CatalogModel {
  id: string;
  label: string;
  file: string;
  url: string;
  sha256: string;
  size: number;
  license: string;
  tier: "lightweight" | "balanced" | "performance";
  goodFor: string;
  /** Measured memory once loaded, MB; 0 when not measured. */
  ramMb: number;
  /** Measured answer speed, tokens a second; 0 when not measured. */
  tokensPerSecond: number;
}

export interface ModelsStatus {
  modelsDir: string;
  runtimeInstalled: string | null;
  runtimePath: string | null;
  runtimes: { id: string; label: string; size: number; available: boolean }[];
  running: string | null;
  /** Bytes the running llama-server takes. */
  runningMemory: number | null;
  /** Local AI was chosen in the AI setup: the engine may start. */
  localAllowed: boolean;
  selected: string;
  local: LocalModelInfo[];
  catalog: CatalogModel[];
  memory: { total: number; available: number };
  cpus: number;
  profile: { context: number; threads: number; animation: string };
  recommendedProfile: "lightweight" | "balanced" | "performance";
  downloading: boolean;
  /** Catalogue ids that suit this PC (≤ 40 % of its memory once loaded). */
  fits: string[];
  /** The "Light local" model for this PC. */
  lightPick: string;
  /** The last start was refused for want of free memory: what it needed, in bytes. */
  shortMemory: number | null;
}

export interface MemoryHog {
  name: string;
  bytes: number;
}

export interface SystemStatus {
  os: string;
  cpuName: string;
  cpus: number;
  cpuUsage: number;
  memoryTotal: number;
  memoryAvailable: number;
  groovyMemory: number;
  disks: { mount: string; total: number; available: number }[];
  battery: { percent: number; charging: boolean } | null;
  uptimeSecs: number;
  /** Bytes since start-up; two readings make a speed. */
  netReceived: number;
  netSent: number;
}

// ── Music (music.rs) ──────────────────────────────────────────────────────────

export interface MusicTrack {
  id: number;
  title: string;
  artist: string;
  album: string;
  genre: string;
  duration: number;
  trackNo: number | null;
  hasArt: boolean;
  favourite: boolean;
  playCount: number;
}

export interface MusicFolder { id: number; path: string; tracks: number; exists: boolean }
export interface MusicPlaylist { id: number; name: string; tracks: number; createdAt: number }
export interface ScanReport { found: number; added: number; updated: number; removed: number }
export type RepeatMode = "off" | "all" | "one";

export interface MusicState {
  queue: number[];
  index: number | null;
  playing: boolean;
  position: number;
  duration: number;
  volume: number;
  shuffle: boolean;
  repeat: RepeatMode;
  track: MusicTrack | null;
  error: string | null;
}

export type ToolPermission = "allow" | "ask" | "deny";
export interface ToolInfo { id: string; label: string; description: string; permission: ToolPermission }
export interface ToolLogEntry { tool: string; args: string; outcome: string; at: number }

// ── Coding Studio (studio.rs) ─────────────────────────────────────────────────
export interface StudioWorkspace { id: number; path: string; name: string; exists: boolean; git: boolean }
export interface StudioEntry { name: string; path: string; dir: boolean; size: number }
export interface StudioFile { text: string; binary: boolean; size: number }
export interface StudioVersion { id: number; reason: string; at: number; existed: boolean; size: number }
export interface StudioHit { path: string; line: number; text: string }
export interface StudioFinished { run: number; code: number | null; timedOut: boolean; seconds: number }

export interface BootInfo {
  settings: Settings;
  /** Logical screen rect of the monitor the island lives on. */
  screen: { x: number; y: number; width: number; height: number; scale: number };
  version: string;
  hookPath: string;
  /** False where the OS has no global cursor (Wayland): see Island.followPageCursor. */
  cursorPoll: boolean;
  /** CI's smoke test: hold the island open on Home (F) or the player (E). */
  smokeView?: "home" | "player" | "frames" | null;
}

/** Ollama on this computer (ollama.rs). */
export interface OllamaStatus {
  running: boolean;
  installed: boolean;
  models: string[];
  canInstall: boolean;
}

/** A step of the one-click offline brain setup. */
export interface OllamaStep {
  stage: "install" | "start" | "model" | "done";
  done: number;
  total: number;
}

/** Which Whisper models are on this computer, and whether the chosen one is. */
export interface VoiceStatus {
  /** The model in Settings is downloaded and voice can start. */
  ready: boolean;
  /** Every model on disk, by name ("tiny.en", "base.en"). */
  installed: string[];
  /** Whether this build has Whisper at all (the "voice" Cargo feature). */
  available: boolean;
}

/** The phone link (src-tauri/src/phone_link): how it is, never its secrets. */
export interface PhoneLinkStatus {
  enabled: boolean;
  running: boolean;
  port: number;
  host: string;
  name: string;
  clients: number;
  error: string | null;
  /** The relay connection ("Away from home Wi-Fi"). */
  relayState: PhoneRelayState;
}

export type PhoneRelayState = "off" | "connecting" | "waiting" | "linked" | "accessRefused" | "unreachable" | "roomTaken" | "tooMany";

/** The relay switch and address. The access key is write-only: only whether one is stored comes back. */
export interface PhoneRelayStatus {
  enabled: boolean;
  url: string;
  hasAccess: boolean;
  state: PhoneRelayState;
}

/** Session details for the phone: the switch (off by default). */
export interface PhoneDetailsStatus {
  enabled: boolean;
}

/** Chat from the phone: the switch and the models the phone may use ("provider/model"). */
export interface PhoneChatStatus {
  enabled: boolean;
  models: string[];
}

/** What the settings window shows to pair a phone. Only the settings window may ask. */
export interface PhoneLinkPairing {
  link: string;
  qrSvg: string;
  host: string;
  port: number;
  name: string;
  /** The relay's host when this link also carries the relay fields. */
  relay: string | null;
}

/** One session as the phone sees it; Rust stamps `updatedAt`. */
export interface PhoneLinkSession {
  pillId: string;
  agent: string;
  state: string;
  statusText: string;
  stepIndex: number;
  stepCount: number;
  /** Details, for a phone that has the capability (the user's switch). */
  steps?: string[];
  finalLine?: string;
  /** A folder name, never a path. */
  project?: string;
  color?: string;
  /** The files this session changed, for a phone that has `diffs` (Rust cuts them to a name and a count). */
  files?: PhoneLinkFile[];
}

export interface PhoneLinkFile {
  id: number;
  /** A file name; Rust keeps only its last segment. */
  name: string;
  added: number;
  removed: number;
  tooLarge?: boolean;
  isNew?: boolean;
}

/** One file's diff for the phone that asked: lines as [kind, text], kind "+", "-", " " or "@" (a hunk's first line). */
export interface PhoneLinkDiff {
  pillId: string;
  fileId: number;
  name: string;
  added: number;
  removed: number;
  tooLarge: boolean;
  gone: boolean;
  lines: [string, string][];
}

/** One plan window for the phone: used percentage (as the island has it) and when it resets (epoch ms). */
export interface PhoneLinkWindow {
  usedPct: number;
  resetsAt: number;
}

export interface PhoneLinkPlan {
  fiveHour?: PhoneLinkWindow;
  sevenDay?: PhoneLinkWindow;
  resetCredits?: number;
  planType?: string;
  updatedAt: number;
}

export interface PhoneLinkUsage {
  claude: PhoneLinkPlan | null;
  codex: PhoneLinkPlan | null;
}

/** What one service pill says, for a phone whose user allowed it (Rust cuts every text to size). */
export interface PhoneLinkService {
  id: string;
  title: string;
  headline: string;
  reason?: string;
  items: { label: string; detail: string }[];
}

export interface PhoneServicesStatus {
  /** The services allowed for the phone. */
  services: string[];
}

export interface PhoneUsageStatus {
  enabled: boolean;
}

export interface PhoneDiffsStatus {
  enabled: boolean;
}

export interface PhoneLinkApproval {
  requestId: string;
  sessionId: string;
  pillId: string;
  tool: string;
  command: string;
}

/** The question Claude Code is waiting on, for a phone that may answer it (phone_link/hub.rs). */
export interface PhoneLinkQuestion {
  requestId: string;
  sessionId: string;
  pillId: string;
  questions: { question: string; options: { label: string; description: string }[]; multiSelect: boolean }[];
}

export interface PhoneAnswersStatus {
  enabled: boolean;
}

export const Bridge = {
  boot: () => call<BootInfo>("boot"),
  /** The system's languages as the webview sees them, for Rust's own texts (i18n.rs). */
  setSystemLanguages: (languages: string[]) => call<void>("set_system_languages", { languages }),

  saveSettings: (settings: Settings) => call<void>("save_settings", { settings }),

  /** Shrink the window down to the invisible wake strip (hidden) or back to full. */
  setCollapsed: (collapsed: boolean) => call<void>("set_collapsed", { collapsed }),

  /**
   * Pushes the island shape in window coordinates. Rust flips click-through from
   * its own cursor poll, so the flag is never a frame behind a click.
   */
  /** CI's frame-time test: the frames go to frame-test.json for the workflow. */
  frameReport: (report: unknown) => call<void>("frame_report", { report }),
  setIslandRect: (x: number, y: number, width: number, height: number, radius: number) =>
    call<void>("set_island_rect", { x, y, width, height, radius }),
  /** The growth clock's record only (mochi/growth.ts); Rust keeps the furthest. */
  saveGrowth: (growth: unknown) => call<void>("save_growth", { growth }),
  /** The window is sized to the island (round 3 §8 B): grown before it grows, shrunk after. */
  fitIsland: (width: number, height: number) => call<void>("fit_island", { width, height }),
  /** The island is glass now (Windows' Acrylic behind it), or solid. */
  islandGlass: () => call<string>("island_glass"),

  /** Give the window keyboard focus (chat field) and take it away again. */
  /**
   * `take`: also take the keyboard now (the chat). Without it the island only
   * becomes clickable-to-focus: activating the window costs a long frame.
   */
  focusWindow: (focused: boolean, take = true) => call<void>("focus_window", { focused, take }),

  reposition: () => call<void>("reposition"),
  fitZoom: (dpr: number) => call<boolean>("fit_zoom", { dpr }),

  /** Displays the island can be pinned to: `key` is what `settings.screen` stores. */
  listMonitors: () => call<{ key: string; label: string }[]>("list_monitors"),

  openUrl: (url: string) => call<void>("open_url", { url }),
  /** Groovy's right-click menu, at the pointer (lib.rs context_menu). */
  contextMenu: () => call<void>("context_menu"),

  /** Which of these sounds have a file of the user's own in the sounds folder. */
  customSounds: (names: readonly string[]) => call<string[]>("custom_sounds", { names }),
  /** The user's file for one sound, as raw bytes. */
  customSound: (name: string) => call<ArrayBuffer>("custom_sound", { name }),
  revealSoundsFolder: () => call<void>("reveal_sounds_folder"),
  /** Tells the island to read the sounds folder again ("sounds-changed"). */
  reloadSounds: () => call<void>("reload_sounds"),

  /** "Open terminal" → opens the folder in VS Code when `code` is on PATH. */
  openInVSCode: (path: string | null) => call<boolean>("open_in_vscode", { path }),

  /**
   * "Open terminal": the window the session runs in when Rust found it
   * (Windows; Linux on X11 and KDE Plasma), else the folder in VS Code.
   */
  openSession: (sessionId: string | null, path: string | null) =>
    call<boolean>("open_session", { sessionId, path }),

  /** The Claude desktop app, for the Claude Desktop pill (Windows only). */
  openClaudeDesktop: () => call<boolean>("open_claude_desktop"),

  /** The diff card's ↗: an existing file, in VS Code; never launched by its type. */
  openFileInVSCode: (path: string) => call<boolean>("open_file_in_vscode", { path }),

  quit: () => call<void>("quit_app"),

  /** Opens the Control Centre, on `page` when given ("island", "reminders"…). */
  openSettingsWindow: (page?: string) => call<void>("open_settings_window", { page: page ?? null }),
  /** "My island" (island_tiles.rs): apps that can go on it, opening a tile, mail pop-ups. */
  islandApps: () => call<{ name: string; target: string }[]>("island_apps"),
  islandLaunch: (kind: string, target: string) => callOrThrow<void>("island_launch", { kind, target }),
  islandPickFolder: () => callOrThrow<string | null>("island_pick_folder"),
  mailAccess: () => call<"allowed" | "denied" | "unspecified" | "unsupported">("mail_access"),
  mailOpen: (appId: string) => callOrThrow<void>("mail_open", { appId }),

  /** Writes to %LOCALAPPDATA%\Groovy\groovy.log, next to the Rust lines. */
  log: (message: string) => call<void>("log_line", { message }),

  // ── Claude Code hooks ─────────────────────────────────────────────────────
  hooksStatus: () => call<HookStatus>("hooks_status"),
  /** Pill ID → whether that agent's hooks reach Groovy (read-only, Mac #183). */
  agentHooksStatus: () => call<Record<string, boolean>>("agent_hooks_status"),
  /** Diff to show before anything is written. `install: false` previews removal. */
  hooksPreview: (install: boolean) => callOrThrow<HookPreview>("hooks_preview", { install }),
  /**
   * Writes ~/.claude/settings.json — only ever after an explicit click, and only
   * when the file still matches the preview the user looked at.
   */
  hooksApply: (install: boolean, fingerprint: string) =>
    callOrThrow<string>("hooks_apply", { install, fingerprint }),

  // ── Other agents (Gemini CLI, Codex, Cursor…) ─────────────────────────────
  agentHooksList: () => call<AgentHookStatus[]>("agent_hooks_list"),
  /** Diff to show before anything is written. `install: false` previews removal. */
  agentHooksPreview: (agent: string, install: boolean) =>
    callOrThrow<AgentHookPlan>("agent_hooks_preview", { agent, install }),
  /**
   * Writes the agent's config — only ever after an explicit click, and only when
   * it still matches the preview the user looked at. Returns the backups taken.
   */
  agentHooksApply: (agent: string, install: boolean, fingerprint: string) =>
    callOrThrow<string>("agent_hooks_apply", { agent, install, fingerprint }),
  // ── Plan usage: the status line relay, installed apart from the hooks ──────
  /** Diff of the status line change. `install: false` previews taking the relay out. */
  statusLinePreview: (install: boolean) => callOrThrow<HookPreview>("status_line_preview", { install }),
  /** Same rules as hooksApply: an explicit click, and only for the diff that was shown. */
  statusLineApply: (install: boolean, fingerprint: string) =>
    callOrThrow<string>("status_line_apply", { install, fingerprint }),
  /**
   * Asks the Codex CLI (`codex app-server`) for its plan limits, as Codex's
   * /status does. The raw `account/rateLimits/read` result, or null when Codex
   * is missing, not signed in or slow (15 s).
   */
  codexPlanUsage: () => call<unknown>("codex_plan_usage"),

  approvalDecision: (requestId: string, decision: "allow" | "deny") =>
    call<void>("approval_decision", { requestId, decision }),
  /** "The card is up" — until this lands the relay only waits a moment. */
  approvalAck: (requestId: string) => call<void>("approval_ack", { requestId }),
  /** Answers a question Claude Code asked: question text → chosen label. */
  approvalAnswer: (requestId: string, answers: Record<string, string | string[]>) =>
    call<void>("approval_answer", { requestId, answers }),

  /** "Nobody can act on this" — Claude Code asks in the terminal right away. */
  approvalDecline: (requestId: string) => call<void>("approval_decline", { requestId }),

  // ── Memory and chat history (memory.rs) ───────────────────────────────────
  memoryList: (kind?: MemoryKind) => callOrThrow<MemoryEntry[]>("memory_list", { kind: kind ?? null }),
  memoryAdd: (kind: MemoryKind, content: string) => callOrThrow<MemoryEntry>("memory_add", { kind, content }),
  memoryUpdate: (id: number, kind: MemoryKind, content: string, pinned: boolean) =>
    callOrThrow<MemoryEntry>("memory_update", { id, kind, content, pinned }),
  memoryDelete: (id: number) => callOrThrow<void>("memory_delete", { id }),
  /** Long-term memory only (one kind, or all); chat history is not touched. */
  memoryClear: (kind?: MemoryKind) => callOrThrow<number>("memory_clear", { kind: kind ?? null }),
  /** Writes a JSON export of everything and returns its path. */
  memoryExport: () => callOrThrow<string>("memory_export"),
  historyList: () => callOrThrow<ConversationInfo[]>("history_list"),
  historyMessages: (id: number) => callOrThrow<HistoryMessage[]>("history_messages", { id }),
  historyDelete: (id: number) => callOrThrow<void>("history_delete", { id }),
  /** Chat history only; long-term memory is not touched. */
  historyClear: () => callOrThrow<number>("history_clear"),
  usageToday: () => call<number>("usage_today"),

  // ── Reminders and to-dos (tasks.rs) ───────────────────────────────────────
  taskList: (includeDone: boolean) => callOrThrow<TaskItem[]>("task_list", { includeDone }),
  taskAdd: (text: string, dueAt: number | null) => callOrThrow<TaskItem>("task_add", { text, dueAt }),
  taskSetDone: (id: number, done: boolean) => callOrThrow<TaskItem>("task_set_done", { id, done }),
  taskDelete: (id: number) => callOrThrow<void>("task_delete", { id }),
  /** "Call mum tomorrow 9am" → the text and when; null when no time is understood. */
  taskParse: (text: string) => call<{ text: string; dueAt: number } | null>("task_parse", { text }),
  taskEdit: (id: number, text: string, dueAt: number | null) => callOrThrow<void>("task_edit", { id, text, dueAt }),
  /** A PC action from the island's This PC panel, through the tools' permissions (pc.rs). */
  pcDo: (action: "volume" | "brightness" | "wifi" | "bluetooth" | "dnd" | "end", value: number | null, app: string | null) =>
    callOrThrow<{ answer: string; pending: { id: number; tool: string; label: string } | null }>("pc_do", { action, value, app }),
  taskClearDone: () => callOrThrow<number>("task_clear_done"),

  // ── Local models (models.rs) ──────────────────────────────────────────────
  modelsStatus: () => call<ModelsStatus>("models_status"),
  /** Which subscription CLIs are installed (brain_ai.rs). */
  aiClis: () => call<{ claudeCode: boolean; codex: boolean }>("ai_clis"),
  modelsInstallRuntime: (id: string) => callOrThrow<string>("models_install_runtime", { id }),
  modelsDownload: (id: string) => callOrThrow<string>("models_download", { id }),
  modelsCancelDownload: () => call<void>("models_cancel_download"),
  modelsDelete: (file: string) => callOrThrow<void>("models_delete", { file }),
  modelsStart: () => callOrThrow<string>("models_start"),
  modelsLoadAnyway: () => callOrThrow<string>("models_load_anyway"),
  memoryHogs: () => call<MemoryHog[]>("memory_hogs"),
  modelsStop: () => call<void>("models_stop"),
  modelsRevealFolder: () => call<void>("models_reveal_folder"),

  systemStatus: () => call<SystemStatus>("system_status"),

  // ── Coding Studio (studio.rs) ─────────────────────────────────────────────
  openStudioWindow: () => call<void>("open_studio_window"),
  studioWorkspaces: () => callOrThrow<StudioWorkspace[]>("studio_workspaces"),
  studioAddWorkspace: () => callOrThrow<StudioWorkspace | null>("studio_add_workspace"),
  studioNewProject: (name: string) => callOrThrow<StudioWorkspace | null>("studio_new_project", { name }),
  studioRemoveWorkspace: (id: number) => callOrThrow<void>("studio_remove_workspace", { id }),
  studioList: (ws: number, path: string) => callOrThrow<StudioEntry[]>("studio_list", { ws, path }),
  studioRead: (ws: number, path: string) => callOrThrow<StudioFile>("studio_read", { ws, path }),
  studioWrite: (ws: number, path: string, text: string, reason?: string) => callOrThrow<void>("studio_write", { ws, path, text, reason: reason ?? null }),
  studioDelete: (ws: number, path: string) => callOrThrow<void>("studio_delete", { ws, path }),
  studioHistory: (ws: number, path: string) => callOrThrow<StudioVersion[]>("studio_history", { ws, path }),
  studioRevert: (ws: number, id: number) => callOrThrow<string>("studio_revert", { ws, id }),
  studioSearch: (ws: number, query: string) => callOrThrow<StudioHit[]>("studio_search", { ws, query }),
  /** Only ever called from a Run button that shows the command: that click is the approval. */
  studioRun: (ws: number, command: string) => callOrThrow<number>("studio_run", { ws, command, approved: true, timeoutMinutes: null }),
  studioStop: (run: number) => call<boolean>("studio_stop", { run }),
  studioGit: (ws: number, what: "status" | "diff" | "log" | "remote") => callOrThrow<string>("studio_git", { ws, what }),
  studioGitAction: (ws: number, action: "init" | "stage" | "commit" | "push", message?: string) =>
    callOrThrow<string>("studio_git_action", { ws, action, message: message ?? null }),
  studioAiTarget: () => call<{ provider: string; local: boolean }>("studio_ai_target"),
  studioAsk: (ws: number, question: string, files: string[], output: string | null, consent: boolean) =>
    callOrThrow<string>("studio_ask", { ws, question, files, output, consent }),

  // ── Music (music.rs) ──────────────────────────────────────────────────────
  /** Opens the folder picker; null when cancelled. */
  musicAddFolder: () => callOrThrow<ScanReport | null>("music_add_folder"),
  musicRescan: () => callOrThrow<ScanReport>("music_rescan"),
  musicFolders: () => callOrThrow<MusicFolder[]>("music_folders"),
  musicRemoveFolder: (id: number) => callOrThrow<void>("music_remove_folder", { id }),
  musicSearch: (query: string, favourites: boolean) => callOrThrow<MusicTrack[]>("music_search", { query, favourites }),
  musicSetFavourite: (id: number, on: boolean) => callOrThrow<void>("music_set_favourite", { id, on }),
  musicArt: (id: number) => call<string | null>("music_art", { id }),
  musicPlaylists: () => callOrThrow<MusicPlaylist[]>("music_playlists"),
  musicPlaylistCreate: (name: string) => callOrThrow<number>("music_playlist_create", { name }),
  musicPlaylistRename: (id: number, name: string) => callOrThrow<void>("music_playlist_rename", { id, name }),
  musicPlaylistDelete: (id: number) => callOrThrow<void>("music_playlist_delete", { id }),
  musicPlaylistTracks: (id: number) => callOrThrow<MusicTrack[]>("music_playlist_tracks", { id }),
  musicPlaylistAdd: (id: number, tracks: number[]) => callOrThrow<void>("music_playlist_add", { id, tracks }),
  musicPlaylistRemove: (id: number, position: number) => callOrThrow<void>("music_playlist_remove", { id, position }),
  musicState: () => call<MusicState>("music_state"),
  musicPlay: (queue: number[], index: number) => callOrThrow<void>("music_play", { queue, index }),
  musicControl: (action: "toggle" | "pause" | "resume" | "next" | "previous" | "stop") => call<void>("music_control", { action }),
  musicSeek: (position: number) => call<void>("music_seek", { position }),
  musicVolume: (volume: number) => call<void>("music_volume", { volume }),
  musicSetMode: (shuffle: boolean, repeat: RepeatMode) => call<void>("music_set_mode", { shuffle, repeat }),
  /** Writes a wardrobe pack (JSON text) to the exports folder and returns its path. */
  wardrobeExport: (json: string) => callOrThrow<string>("wardrobe_export", { json }),

  // ── Phone link (Groovy for Android) ───────────────────────────────────────
  phoneLinkStatus: () => call<PhoneLinkStatus>("phone_link_status"),
  /** Only ever from the switch in Settings. Throws with the reason if it cannot start. */
  phoneLinkSetEnabled: (enabled: boolean) => callOrThrow<PhoneLinkStatus>("phone_link_set_enabled", { enabled }),
  phoneLinkPairing: () => callOrThrow<PhoneLinkPairing>("phone_link_pairing"),
  /** A new code: the phone that had the old one is disconnected. */
  phoneLinkNewPairing: () => callOrThrow<PhoneLinkPairing>("phone_link_new_pairing"),
  phoneDetailsStatus: () => call<PhoneDetailsStatus>("phone_details_status"),
  /** Off by default; only from the switch in Settings. Phones are reconnected so they learn about it. */
  phoneDetailsSetEnabled: (enabled: boolean) => callOrThrow<PhoneDetailsStatus>("phone_details_set_enabled", { enabled }),
  phoneAnswersStatus: () => call<PhoneAnswersStatus>("phone_answers_status"),
  /** Off by default; only from the switch in Settings. Phones are reconnected so they learn about it. */
  phoneAnswersSetEnabled: (enabled: boolean) => callOrThrow<PhoneAnswersStatus>("phone_answers_set_enabled", { enabled }),
  phoneDiffsStatus: () => call<PhoneDiffsStatus>("phone_diffs_status"),
  /** Off by default; only from the switch in Settings. Phones are reconnected so they learn about it. */
  phoneDiffsSetEnabled: (enabled: boolean) => callOrThrow<PhoneDiffsStatus>("phone_diffs_set_enabled", { enabled }),
  /** The answer to a phone's request for a file's diff (the "phone-link-getdiff" event); `conn` says which phone. */
  phoneLinkSendDiff: (conn: number, diff: PhoneLinkDiff) => call<void>("phone_link_send_diff", { conn, diff }),
  phoneServicesStatus: () => call<PhoneServicesStatus>("phone_services_status"),
  /** None by default; only from the ticks in Settings. Phones are reconnected so they learn about it. */
  phoneServicesSet: (services: string[]) => callOrThrow<PhoneServicesStatus>("phone_services_set", { services }),
  phoneLinkPublishServices: (services: PhoneLinkService[]) => call<void>("phone_link_publish_services", { services }),
  phoneRelayStatus: () => call<PhoneRelayStatus>("phone_relay_status"),
  /** Off by default; needs a valid wss:// address and a stored access key. */
  phoneRelaySet: (enabled: boolean, url: string) => callOrThrow<PhoneRelayStatus>("phone_relay_set", { enabled, url }),
  /** Write-only: stored in the OS keystore, never sent back. An empty value clears it. */
  phoneRelaySetAccess: (access: string) => callOrThrow<PhoneRelayStatus>("phone_relay_set_access", { access }),
  phoneUsageStatus: () => call<PhoneUsageStatus>("phone_usage_status"),
  /** Off by default; only from the switch in Settings. Phones are reconnected so they learn about it. */
  phoneUsageSetEnabled: (enabled: boolean) => callOrThrow<PhoneUsageStatus>("phone_usage_set_enabled", { enabled }),
  phoneLinkPublishUsage: (usage: PhoneLinkUsage) => call<void>("phone_link_publish_usage", { usage }),
  phoneChatStatus: () => call<PhoneChatStatus>("phone_chat_status"),
  /** Off by default; only from the switch in Settings. Phones are reconnected so they learn about it. */
  phoneChatSetEnabled: (enabled: boolean) => callOrThrow<PhoneChatStatus>("phone_chat_set_enabled", { enabled }),
  phoneChatSetModels: (models: string[]) => callOrThrow<PhoneChatStatus>("phone_chat_set_models", { models }),
  phoneLinkPublish: (sessions: PhoneLinkSession[], approval: PhoneLinkApproval | null) =>
    call<void>("phone_link_publish", { sessions, approval }),
  phoneLinkPublishQuestion: (question: PhoneLinkQuestion | null) => call<void>("phone_link_publish_question", { question }),
  /** What Groovy wears ("auto" or an outfit), for phones that asked for it. */
  phoneLinkPublishPrefs: (outfit: string) => call<void>("phone_link_publish_prefs", { outfit }),

  // ── Chat, files, secrets ──────────────────────────────────────────────────
  /** One chat turn. The API key and any file bytes never leave Rust. */
  chatSend: (query: string, context: ChatContext | null, spoken = false) =>
    callOrThrow<{ text: string; action?: { id: number; tool: string; label: string } }>("chat_send", { query, context, spoken }),
  /** "Test my voice setup": "say hello" to the brain the calls use. */
  voiceProbe: () => callOrThrow<{ answer: string; brain: string }>("voice_probe"),
  /** One line in %LOCALAPPDATA%\Groovy\logs\voice.log. */
  voiceLog: (line: string) => call<void>("voice_log", { line }),
  /** Answers a tool's approval question: "once" | "always" | "deny". Returns what happened. */
  toolsDecide: (id: number, decision: "once" | "always" | "deny") => callOrThrow<string>("tools_decide", { id, decision }),
  toolsList: () => callOrThrow<ToolInfo[]>("tools_list"),
  toolsSetPermission: (id: string, permission: ToolPermission) => callOrThrow<void>("tools_set_permission", { id, permission }),
  toolsLog: () => callOrThrow<ToolLogEntry[]>("tools_log"),
  toolsLogClear: () => callOrThrow<void>("tools_log_clear"),
  chatReset: () => call<void>("chat_reset"),

  // ── The offline brain (ollama.rs) ─────────────────────────────────────────
  ollamaStatus: () => call<OllamaStatus>("ollama_status"),
  /** Installs/starts Ollama and downloads a model; progress as "ollama-setup" events. */
  ollamaSetup: () => callOrThrow<string>("ollama_setup"),

  // ── Moving the island along the top of the screen (island.rs) ─────────────
  /** Where the island sits (0 left … 1 right) and how many logical px it can travel. */
  islandTrack: () => call<[number, number]>("island_track"),
  islandMove: (fraction: number, save: boolean) => call<void>("island_move", { fraction, save }),

  // ── Voice (voice.rs, livekit.rs) ──────────────────────────────────────────
  /** 16 kHz mono 16-bit samples to text; `wake` asks for the quick pass. */
  /** A full-screen app or game has the screen (round 3C item 2). */
  fullscreenBusy: () => call<boolean>("fullscreen_busy"),
  // Settings → Browser (round 4 item 4, browser.rs).
  browsers: () =>
    call<{ installed: { id: string; name: string }[]; default: string | null; engines: { id: string; name: string }[] }>("browsers"),
  // Talk to Groovy (round 4 item 3, calls.rs).
  voiceCallActive: (on: boolean) => call<void>("voice_call_active", { on }),
  voiceBasic: (text: string) => call<string | null>("voice_basic", { text }),
  callSummary: (turns: [string, string][]) => callOrThrow<void>("call_summary", { turns }),
  instantVoiceOptions: () => call<("openai" | "gemini")[]>("instant_voice_options"),
  instantOpenaiAnswer: (offer: string) => callOrThrow<string>("instant_openai_answer", { offer }),
  instantGeminiToken: () => callOrThrow<{ token: string; setup: unknown }>("instant_gemini_token"),
  voiceTranscribe: (samples: number[], wake: boolean) =>
    callOrThrow<string>("voice_transcribe", { samples, wake }),
  voiceStatus: () => call<VoiceStatus>("voice_status"),
  /** Downloads a Whisper model; progress arrives as "voice-model-progress" events. */
  voiceDownloadModel: (model: string) => callOrThrow<void>("voice_download_model", { model }),
  voiceDeleteModel: (model: string) => callOrThrow<void>("voice_delete_model", { model }),
  /** A short-lived access token for the user's LiveKit room. */
  livekitToken: () => callOrThrow<{ url: string; token: string }>("livekit_token"),
  /**
   * The models a provider offers, for the picker in the chat view. Rust asks
   * the provider only when it has a key (or a server address).
   */
  chatModels: (provider: string) => callOrThrow<ModelInfo[]>("chat_models", { provider }),
  /** Settings → Local models → Connect: does the server answer, and with which models? */
  /** The custom server's key, bound to the address it is entered for. */
  localSetKey: (url: string, key: string) => call<void>("local_set_key", { url, key }),

  localConnect: (provider: "ollama" | "lmstudio" | "custom", url: string) =>
    callOrThrow<LocalServer>("local_connect", { provider, url }),
  /** Copies a dropped file into the inbox. */
  ingestFile: (path: string) => callOrThrow<DroppedFile>("ingest_file", { path }),
  /** Only ever tells you whether a key exists — never its value. */
  secretPresent: (key: string) => call<boolean>("secret_present", { key }),
  secretSet: (key: string, value: string) => callOrThrow<void>("secret_set", { key, value }),
  secretClear: (key: string) => callOrThrow<void>("secret_clear", { key }),

  // ── Integrations ──────────────────────────────────────────────────────────
  refreshIntegration: (id: string) => call<void>("refresh_integration", { id }),
  /** The GitHub card is on screen: refetch that part if it is stale. */
  githubRefresh: (section: "pulse" | "activity") => call<void>("github_refresh", { section }),
  /** Opens the configured n8n instance in the browser. */
  openN8n: () => call<void>("open_n8n").catch(showError),

  /** Tray → Pause. Stops the integration pollers, not just the island. */
  setPaused: (paused: boolean) => call<void>("set_paused", { paused }),

  // ── Global shortcuts ──────────────────────────────────────────────────────
  /** How each global shortcut went when Rust last registered them. */
  shortcutsStatus: () => call<ShortcutsReport>("shortcuts_status"),
  /** Lets go of every global shortcut while Settings records a new one. */
  shortcutsSuspend: (suspended: boolean) => call<void>("shortcuts_suspend", { suspended }),

  // ── Weekly recap ──────────────────────────────────────────────────────────
  /** Turns and decisions from `since` (Unix seconds) on, plus the recap prefs. */
  recapHistory: (since: number) => call<RecapHistory>("recap_history", { since: Math.floor(since) }),
  recapPrefs: () => call<RecapPrefs>("recap_prefs"),
  recapSetEnabled: (enabled: boolean) => call<void>("recap_set_enabled", { enabled }),
  recapSetHideProjects: (hide: boolean) => call<void>("recap_set_hide_projects", { hide }),
  /** `week` is the Monday (YYYY-MM-DD) the recap opened on its own for. */
  recapMarkShown: (week: string) => call<void>("recap_mark_shown", { week }),
  recapClear: () => call<void>("recap_clear"),
  /** Writes the PNG (a data URL) into Pictures or Downloads; returns its path. */
  recapSavePng: (data: string, week: string) => callOrThrow<string>("recap_save_png", { data, week }),
  /** Opens the folder of the image saved last. */
  recapRevealSaved: () => call<void>("recap_reveal_saved"),

  // ── Groovy on the desktop (src-tauri/src/desktop.rs) ───────────────────────
  desktopInfo: () => call<DesktopInfo>("desktop_mochi_info"),
  /** Dragged out of the island: (x, y) is the pointer in island-window coordinates. */
  desktopPickUp: (x: number, y: number) => call<boolean>("desktop_mochi_pick_up", { x, y }),
  /** Linux: the pointer moved during that drag (Windows carries him from Rust). */
  desktopCarry: (x: number, y: number) => call<void>("desktop_mochi_carry", { x, y }),
  desktopCarryEnd: (x: number, y: number) => call<void>("desktop_mochi_carry_end", { x, y }),
  /** A drag started on the desktop Groovy; resolves to his top-left corner. */
  desktopDragBegin: () => call<[number, number] | null>("desktop_mochi_drag_begin"),
  /** X11: top-left corner, physical pixels. */
  desktopDragMove: (x: number, y: number) => call<void>("desktop_mochi_drag_move", { x, y }),
  desktopDragEnd: (x: number, y: number) => call<void>("desktop_mochi_drag_end", { x, y }),
  /** From the island to his spot. False: no spot on any connected display.
   *  `anywhere`: the first-visit corner then, rather than staying home. */
  desktopFlyOut: (anywhere = false) => call<boolean>("desktop_mochi_fly_out", { anywhere }),
  /** To the island, then hidden. `forget`: he lives in the island again. */
  desktopFlyHome: (forget: boolean) => call<boolean>("desktop_mochi_fly_home", { forget }),
  /** Asleep, the cursor poll stops. */
  desktopSetAsleep: (asleep: boolean) => call<void>("desktop_mochi_set_asleep", { asleep }),

  /** An agent session started in `cwd`: the Skills Manager may look at the project again. */
  skillsSessionStarted: (cwd: string) => call<void>("skills_session_started", { cwd }),

  // ── Spotify (src-tauri/src/spotify.rs) ────────────────────────────────────
  /** Reads the player again (the card came on screen) and reports it. */
  spotifyRefresh: () => call<SpotifyState | null>("spotify_refresh"),
  /** True when Spotify took it. `value`: seconds for seek, 0/1 for shuffle and repeat, 0…100 for volume. */
  spotifyControl: (action: SpotifyAction, value?: number) =>
    call<boolean>("spotify_control", { action, value: value ?? null }),
  /** Brings Spotify forward or starts it; without it, its download page. */
  spotifyOpen: () => call<boolean>("spotify_open"),
  /** Whether there is a Spotify to launch (Settings). */
  spotifyInstalled: () => call<boolean>("spotify_installed"),
};

export type SpotifyAction = "playPause" | "next" | "previous" | "seek" | "shuffle" | "repeat" | "volume";

export type ShortcutStatus =
  | "active" | "off" | "inUse" | "duplicate" | "invalid"
  | "typesCharacter" | "unsupported" | "notPorted"
  /** Wayland: handed to the desktop's portal, no answer yet / left out by it. */
  | "pending" | "refused";

export interface ShortcutsReport {
  /** `trigger`: on Wayland, the keys the desktop says run it, in its words. */
  actions: { id: string; status: ShortcutStatus; typed?: string; trigger?: string }[];
  /** "wayland" or "no-display" when no key grab can be made. */
  blocked: string | null;
  /** `<executable> --shortcut`: append an action id for a desktop shortcut. */
  command: string;
  /** Wayland: the shortcuts went to the desktop's GlobalShortcuts portal. */
  portal?: "pending" | "active" | null;
}

/** How the desktop Groovy's window works here (platform::DesktopMode). */
export type DesktopMode = "poll" | "window" | "layer" | "off";

export interface DesktopInfo {
  mode: DesktopMode;
  /** He was on the desktop when the app last quit. */
  onDesktop: boolean;
}

/** An event for one window only (island ⇄ desktop Groovy). Never throws. */
export async function emitToWindow(label: string, event: string, payload?: unknown) {
  if (!IS_TAURI) return;
  try {
    await emitTo(label, event, payload);
  } catch (err) {
    console.error(`[groovy] emit ${event} failed`, err);
  }
}

export interface IntegrationUpdate {
  id: string;
  data: Record<string, unknown>;
  error: string | null;
  event: { success: boolean; label: string; detail: string | null } | null;
}

export type ChatContext =
  | { kind: "file"; name: string; path: string }
  | { kind: "window"; appName: string; title: string; url?: string };

export interface ModelInfo {
  id: string;
  label: string;
}

export interface LocalServer {
  /** The address as it is stored. */
  url: string;
  models: string[];
  /** The address is this machine. */
  loopback: boolean;
}

export interface DroppedFile {
  name: string;
  path: string;
  size: number;
}

export interface HookStatus {
  installed: boolean;
  /** Groovy's status line relay (plan usage) is the status line in settings.json. */
  planRelayInstalled: boolean;
  settingsPath: string;
  hookPath: string;
  hookReady: boolean;
}

/** One agent other than Claude Code, as agents.rs reports it. */
export interface AgentHookStatus {
  /** The `--agent` name; its pill is `agent_<id>`. */
  id: string;
  name: string;
  installed: boolean;
  /** The file (or files, one per line) Groovy writes. */
  path: string;
  hookReady: boolean;
  /** The island can allow or deny this agent's permission requests. */
  approvals: boolean;
  /** What to do once it is written. */
  note: string;
}

export interface AgentHookPlan {
  diff: string;
  /** Where each existing file is copied first, one per line; "" when none. */
  backup: string;
  path: string;
  fingerprint: string;
}

export interface HookPreview {
  diff: string;
  backup: string;
  settingsPath: string;
  /** Hand back to hooksApply so only the reviewed diff is ever written. */
  fingerprint: string;
}

/** Same as `call`, but surfaces the error so the UI can show what went wrong. */
async function callOrThrow<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!IS_TAURI) throw new Error("not running inside Groovy");
  return invoke<T>(cmd, args);
}

export type BridgeEvent =
  | { name: "cursor"; payload: { x: number; y: number } }
  | { name: "tray"; payload: string }
  | { name: "hook"; payload: Record<string, unknown> }
  | { name: "screen-changed"; payload: null };

export interface DragDropPayload {
  type: "enter" | "over" | "drop" | "leave";
  paths?: string[];
}

interface WebView2Bridge {
  postMessageWithAdditionalObjects(message: unknown, objects: ArrayLike<unknown>): void;
}

/**
 * Files dragged onto the island. Only reaches us when the window takes the mouse.
 *
 * On Windows WebView2 takes the drop itself, as in Edge — Tauri's drop handling
 * never sees drags from the classic Explorer folder view (see
 * src-tauri/src/webview_drop.rs). Enter/over/leave come straight from the page;
 * the drop hands the File objects to Rust, which answers with their real paths
 * as a `file-drag` event. Elsewhere (Linux) Tauri's own drag events still do it.
 */
export async function onDragDrop(handler: (e: DragDropPayload) => void) {
  if (!IS_TAURI) return () => {};
  const webview = (window as unknown as { chrome?: { webview?: WebView2Bridge } }).chrome?.webview;
  if (!webview) {
    return getCurrentWebview().onDragDropEvent((event) => {
      handler(event.payload as DragDropPayload);
    });
  }
  const hasFiles = (e: DragEvent) => e.dataTransfer?.types.includes("Files") ?? false;
  // dragenter/dragleave fire for every element crossed; only the outermost pair counts.
  let depth = 0;

  const onEnter = (e: DragEvent) => {
    if (!hasFiles(e)) return;
    e.preventDefault();
    if (depth++ === 0) handler({ type: "enter" });
  };
  const onOver = (e: DragEvent) => {
    if (!hasFiles(e)) return;
    // Without this WebView2 refuses the drop — or navigates to the file.
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "copy";
    handler({ type: "over" });
  };
  const onLeave = (e: DragEvent) => {
    if (!hasFiles(e) || depth === 0) return;
    if (--depth === 0) handler({ type: "leave" });
  };
  const onDrop = (e: DragEvent) => {
    e.preventDefault();
    depth = 0;
    const files = e.dataTransfer?.files;
    if (!files || files.length === 0) {
      handler({ type: "drop", paths: [] });
      return;
    }
    webview.postMessageWithAdditionalObjects("groovy-file-drop", files);
  };

  window.addEventListener("dragenter", onEnter);
  window.addEventListener("dragover", onOver);
  window.addEventListener("dragleave", onLeave);
  window.addEventListener("drop", onDrop);
  const unlisten = await listen<DragDropPayload>("file-drag", (e) => handler(e.payload));
  return () => {
    window.removeEventListener("dragenter", onEnter);
    window.removeEventListener("dragover", onOver);
    window.removeEventListener("dragleave", onLeave);
    window.removeEventListener("drop", onDrop);
    unlisten();
  };
}

export async function onEvent<T>(name: string, handler: (payload: T) => void) {
  if (!IS_TAURI) return () => {};
  return listen<T>(name, (e) => handler(e.payload));
}
