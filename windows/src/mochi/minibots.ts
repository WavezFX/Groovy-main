// Small Groovys for task pills and the compact grid (round 4E: rewritten for
// Groovy). Each one is a canvas with its own BotEngine in the flat "mini" style;
// the island's frame loop calls tickMiniBots() while any of them is shown.
import { BotEngine, hexToRGB } from "./engine";
import type { AgentTask } from "../core/state";

interface Mini {
  engine: BotEngine;
  /** The canvas's CSS size: the body is drawn at 60 % of it. */
  size: number;
  taskId: string;
  /** Read each frame: whether this one sways to the music. */
  dancing?: () => boolean;
}

const minis = new Map<HTMLCanvasElement, Mini>();

/** Device pixels per CSS pixel, capped at 2 so a mini never costs much to draw. */
const ratio = () => Math.min(2, window.devicePixelRatio || 1);

/**
 * A slot `bodySize` CSS pixels square holding a mini Groovy whose body fills
 * it. The engine draws the body at 60 % of its canvas, so the canvas is
 * `bodySize / 0.6` across, centred on the slot and overflowing it.
 */
export function createMiniBot(task: AgentTask, bodySize: number, dancing?: () => boolean): HTMLElement {
  const slot = document.createElement("span");
  slot.className = "mini";
  slot.style.width = slot.style.height = `${bodySize}px`;

  const size = bodySize / 0.6;
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = Math.round(size * ratio());
  canvas.style.width = canvas.style.height = `${size}px`;
  slot.append(canvas);

  const engine = new BotEngine();
  engine.isMini = true;
  engine.bodyColor = hexToRGB(task.color);
  engine.setState(task.state, true);
  if (task.emote) engine.setPermanentEmote(task.emote);
  if (task.miniEye) {
    // A fixed eye for this task, kept through every state change.
    engine.permanentEye = engine.eyeOverride = task.miniEye;
    engine.eyeOverrideUntil = Number.POSITIVE_INFINITY;
  }
  minis.set(canvas, { engine, size, taskId: task.id, dancing });
  return slot;
}

/** Forgets every mini whose canvas has left the page (views are rebuilt whole). */
export function pruneMiniBots() {
  for (const canvas of [...minis.keys()]) if (!canvas.isConnected) minis.delete(canvas);
}

/** Follows each task's state and colour. */
export function syncMiniBotStates(tasks: AgentTask[]) {
  const byId = new Map(tasks.map((t) => [t.id, t]));
  for (const m of minis.values()) {
    const task = byId.get(m.taskId);
    if (!task) continue;
    m.engine.setState(task.state);
    m.engine.bodyColor = hexToRGB(task.color);
  }
}

/** Steps and redraws every mini on the page. */
export function tickMiniBots(dt: number) {
  const r = ratio();
  for (const [canvas, m] of minis) {
    if (!canvas.isConnected) continue; // not on the page (yet): pruneMiniBots() forgets it
    const x = canvas.getContext("2d");
    if (!x) continue;
    m.engine.setDancing(m.dancing?.() ?? false);
    m.engine.update(dt);
    x.setTransform(r, 0, 0, r, 0, 0);
    x.clearRect(0, 0, m.size, m.size);
    x.save();
    m.engine.applyDance(x, m.size, m.size);
    m.engine.draw(x, m.size, m.size);
    x.restore();
  }
}
