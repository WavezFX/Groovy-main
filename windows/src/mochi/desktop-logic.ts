// Groovy on the desktop: the pure rules and his life cycle out there (round 4E
// item 1: rewritten for Groovy; the flight's own curve is in
// src-tauri/src/desktop.rs, a pull back then a glide). No DOM, no Tauri: the
// island wires it up in src/island/desktop.ts, the desktop window draws him in
// src/desktop/main.ts, and Rust owns the window.
//
// Coordinates are y-down everywhere on a PC, so the Mac's DesktopSpace flip is
// not needed: a point on the desktop is used as the OS reports it.

import type { Appearance } from "./appearance";
import type { BotEmoteName, BotStateName } from "../core/layout";
import type { Outfit } from "./wardrobe";
import type { Skin } from "./skins";

/** What the island tells the desktop window, whenever it changes. */
export interface DesktopSnapshot {
  state: BotStateName;
  /** He is always the main Groovy, so always dressed (try-ons included). */
  outfit: Outfit;
  /** Groovy's look (try-ons included). */
  look: Skin;
  /** How far the look has grown, 0–5 (mochi/growth.ts). */
  stage: number;
  /** Accessories worn with the outfit, and his colours (mochi/appearance.ts). */
  layers: Outfit[];
  appearance: Appearance;
  soundEnabled: boolean;
  soundVolume: number;
  /** Tray → Pause: he dozes off and stays asleep. */
  paused: boolean;
  /** Music plays: he dances (the compact island's rules, core/spotify.ts). */
  dancing: boolean;
}

/** Events between the two windows. Rust adds `desktop-mochi-dropped`. */
export const DESKTOP_EVENTS = {
  /** island → desktop window */
  state: "desktop-mochi-state",
  emote: "desktop-mochi-emote",
  /** desktop window → island */
  ready: "desktop-mochi-ready",
  home: "desktop-mochi-home",
  wardrobe: "desktop-mochi-wardrobe",
  dizzy: "desktop-mochi-dizzy",
  /** Rust → island: a drag ended, `{ from: "island" | "desktop", home }`. */
  dropped: "desktop-mochi-dropped",
  /** Rust → desktop window */
  cursor: "desktop-cursor",
  visible: "desktop-visible",
  flight: "desktop-flight",
  /** Rust → desktop window: pointing at a mark, `{ x, y }` (a unit vector) or null (round 4C item 5). */
  point: "desktop-point",
  /** Rust → island: he left the island to point at a mark (true) and came back (false). */
  pointing: "desktop-mochi-pointing",
} as const;

// ── Sizes and timings ─────────────────────────────────────────────────────────

/** Side of the desktop window, logical pixels. */
export const PANEL_SIZE = 120;
/** Seconds without agent work before he may doze off. */
export const SLEEP_TIMEOUT = 120;
/** He dozes off only while the pointer is at least this far from him. */
export const SLEEP_MOUSE_DISTANCE = 150;
/** Clickable body radius, as a share of the window side. */
export const BODY_RADIUS_FRACTION = 0.24;
/** An alert: his surprise plays out (eyes, taut outline, flared wisps) before he flies to the island. */
export const RETRACT_DELAY_MS = 520;
/** The alert answered: a beat in the island before he flies back out. */
export const RETURN_DELAY_MS = 650;
/** A single click is a poke unless a second one follows this soon. */
export const DOUBLE_CLICK_MS = 300;
/** A press that moves farther than this (px) is a drag, not a click. */
export const DRAG_THRESHOLD = 3;

// ── Rules ─────────────────────────────────────────────────────────────────────

export interface Point {
  x: number;
  y: number;
}

/** Asleep: no agent work for longer than SLEEP_TIMEOUT, and the pointer not near him. */
export function shouldSleep(sinceAgentActive: number, mouseDistanceToCenter: number): boolean {
  if (sinceAgentActive <= SLEEP_TIMEOUT) return false;
  return mouseDistanceToCenter >= SLEEP_MOUSE_DISTANCE;
}

/** Whether a window-local point is on his round body (not just the square window). */
export function isOverBody(local: Point, panelSize = PANEL_SIZE): boolean {
  const half = panelSize / 2;
  return Math.hypot(local.x - half, local.y - half) <= panelSize * BODY_RADIUS_FRACTION;
}

/** The point his eyes look from: the window's centre, in the cursor's space. */
export function lookOrigin(panelMinX: number, panelMinY: number, panelSize = PANEL_SIZE): Point {
  const half = panelSize / 2;
  return { x: panelMinX + half, y: panelMinY + half };
}

/**
 * Where his eyes point (x right +, y up +, each −1…1): a soft saturation, so
 * a pointer close by moves them a lot and a far one nearly pins them.
 */
export function gaze(bot: Point, mouse: Point): { lookX: number; lookY: number } {
  const soft = (d: number, k: number) => d / (Math.abs(d) + k);
  return { lookX: soft(mouse.x - bot.x, 240), lookY: -soft(mouse.y - bot.y, 190) };
}

/** An alert that went up during a flight sends him straight back once he lands. */
export function shouldRetractOnLanding(alertActive: boolean): boolean {
  return alertActive;
}

/** Agent work keeps him awake; idle and sleeping don't count. */
export function agentActive(state: BotStateName): boolean {
  return !(state === "idle" || state === "sleeping");
}

/**
 * What calls him back to the island: its card waiting for an answer (any
 * agent's permission, or a question Claude Code asked, folded or not), or an
 * agent asking a question by notification.
 */
export function alertActive(s: {
  pendingApproval: { questions?: unknown } | null;
  tasks: ReadonlyArray<{ state: BotStateName }>;
}): boolean {
  return s.pendingApproval != null || s.tasks.some((t) => t.state === "question");
}

/**
 * Wayland gives no global cursor, so his window hears the pointer only while it
 * is over him; it then counts as "near" for this long.
 */
export const POINTER_MEMORY_MS = 4000;

/**
 * The pointer's distance from his centre, for shouldSleep: measured when the
 * cursor is polled (Windows), otherwise near for a while after it touched him.
 */
export function pointerDistance(
  cursorPoll: boolean,
  cursor: Point | null,
  sincePointerMs: number,
  panelSize = PANEL_SIZE,
): number {
  if (!cursorPoll) return sincePointerMs < POINTER_MEMORY_MS ? 0 : Infinity;
  if (!cursor) return Infinity;
  const half = panelSize / 2;
  return Math.hypot(cursor.x - half, cursor.y - half);
}

/**
 * Layer-shell drag: the surface covers the display, so the pointer is in
 * display coordinates once the compositor applied that; until then it is
 * still relative to the small surface at `origin`.
 */
export function layerDragTopLeft(origin: Point, client: Point, grab: Point, overlayApplied: boolean): Point {
  const ox = overlayApplied ? 0 : origin.x;
  const oy = overlayApplied ? 0 : origin.y;
  return { x: ox + client.x - grab.x, y: oy + client.y - grab.y };
}

/** X11 drag: global screen coordinates; the window moves in physical pixels. */
export function windowDragTopLeft(origin: Point, screen: Point, screenAtPress: Point, dpr: number): Point {
  return { x: origin.x + dpr * (screen.x - screenAtPress.x), y: origin.y + dpr * (screen.y - screenAtPress.y) };
}

// ── Life cycle ────────────────────────────────────────────────────────────────

export type DesktopPhase =
  /** No window on screen: he is in the island. */
  | "home"
  /** Flying from the island to his spot. */
  | "flyingOut"
  /** On the desktop. */
  | "onDesktop"
  /** An alert just went up: surprised, about to fly to the island. */
  | "retracting"
  /** The alert was answered while he was on his way to the island. */
  | "alertResolvedDuringRetract"
  /** In the island for the alert; out again once it is answered. */
  | "atNotchForAlert";

/** Everything the life cycle does to the world, injected so it can be tested. */
export interface DesktopPorts {
  /**
   * Shows the window at the island and flies it to his spot. False: no spot.
   * `anywhere`: a spot on a display that is gone gives way to the first-visit
   * corner (the user asked him out).
   */
  flyOut(anywhere: boolean): Promise<boolean>;
  /** Flies the window to the island and hides it. `forget`: he lives there again. */
  flyHome(forget: boolean): Promise<boolean>;
  /** The island's own Groovy hides while he is out. */
  setAway(away: boolean): void;
  /** An emote on the desktop Groovy. */
  emote(emote: BotEmoteName, duration?: number): void;
  play(sound: string): void;
  alertActive(): boolean;
  /** The island must be on screen for him to show the alert there. */
  revealIsland(): void;
  later(fn: () => void, ms: number): void;
}

/** The phases in which he is out on the desktop (or just about to leave it). */
const OUT: ReadonlySet<DesktopPhase> = new Set(["onDesktop", "retracting", "alertResolvedDuringRetract"]);

export class DesktopMochiController {
  phase: DesktopPhase = "home";
  /** He lives on the desktop (the `onDesktop` preference). */
  enabled = false;

  private port: DesktopPorts;
  private prevState: BotStateName | null = null;
  private alertOn = false;

  constructor(port: DesktopPorts) {
    this.port = port;
  }

  /** Launch, and after an alert: out to his spot, if that is where he lives. */
  async launchFlyIfNeeded(anywhere = false): Promise<void> {
    if (this.phase !== "home" || !this.enabled) return;
    if (this.port.alertActive()) {
      // Wait in the island; the alert's end sends him out.
      this.phase = "atNotchForAlert";
      return;
    }
    this.phase = "flyingOut";
    this.port.setAway(true);
    const landed = await this.port.flyOut(anywhere);
    if (this.phase !== "flyingOut") return; // something else took over mid-flight
    if (landed) {
      this.arrive();
      return;
    }
    // His spot is on a display that is gone: he stays home.
    this.phase = "home";
    this.enabled = false;
    this.port.setAway(false);
  }

  /** Dropped on the desktop after a drag out of the island (Rust placed him). */
  installed() {
    if (this.phase !== "home") return;
    this.enabled = true;
    this.port.setAway(true);
    this.port.emote("happy", 0.6);
    this.port.play("pop");
    this.arrive();
  }

  /** Double click, or dropped on the island: home for good (wins over a pending retract). */
  async flyHome(): Promise<void> {
    if (!OUT.has(this.phase)) return;
    this.phase = "home";
    this.enabled = false;
    await this.port.flyHome(true);
    this.port.play("peek");
    this.port.setAway(false);
  }

  /** The desktop shortcut: out from the island, home from the desktop; otherwise nothing. */
  async flyOutOrHome(): Promise<void> {
    if (this.phase === "onDesktop") return this.flyHome();
    if (this.phase !== "home") return;
    this.enabled = true;
    await this.launchFlyIfNeeded(true);
  }

  /** Call whenever the alert may have changed; only a change does anything. */
  updateAlert(active: boolean) {
    if (active === this.alertOn) return;
    this.alertOn = active;
    if (active) {
      if (this.phase === "onDesktop") this.retract();
    } else if (this.phase === "retracting") {
      this.phase = "alertResolvedDuringRetract";
    } else if (this.phase === "atNotchForAlert") {
      this.phase = "home";
      this.port.later(() => void this.launchFlyIfNeeded(), RETURN_DELAY_MS);
    }
  }

  /** Call with the effective state: a task finishing gets a happy puff out there. */
  updateState(state: BotStateName) {
    const before = this.prevState;
    this.prevState = state;
    if (before !== null && before !== state && state === "finished" && this.phase === "onDesktop") {
      this.port.emote("happy", 1.2);
    }
  }

  private arrive() {
    this.phase = "onDesktop";
    // Nothing reacts to an alert during a flight: look now.
    this.alertOn = this.port.alertActive();
    if (shouldRetractOnLanding(this.alertOn)) this.retract();
  }

  private retract() {
    this.phase = "retracting";
    this.port.emote("surprised");
    this.port.later(() => {
      if (this.phase === "alertResolvedDuringRetract") this.phase = "onDesktop"; // answered before take-off
      else if (this.phase === "retracting") void this.flyToAlert();
    }, RETRACT_DELAY_MS);
  }

  private async flyToAlert() {
    await this.port.flyHome(false);
    this.port.setAway(false);
    if (this.phase === "retracting") {
      this.phase = "atNotchForAlert";
      this.port.revealIsland();
    } else if (this.phase === "alertResolvedDuringRetract") {
      this.phase = "home";
      await this.launchFlyIfNeeded();
    }
  }
}
