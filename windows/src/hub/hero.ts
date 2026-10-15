// The coloured band on top of each page of the Control Centre: a deep black
// with the page's colour breathing in a corner, particles that drift all the
// time and move away from the pointer, and Groovy strolling on its floor. Also
// the glowing cards and Groovy on empty pages. The backdrop is ported from
// Groovy 0.5.0 (MIT); the character is Groovy's own.
//
// One animation loop draws every live canvas; it stops when none is on screen
// or the window is hidden, so a hidden window costs nothing.

import { BotEngine, hexToRGB } from "../mochi/engine";
import type { BotEmoteName, BotStateName } from "../core/layout";

interface Animated {
  el: HTMLCanvasElement;
  draw(dt: number, t: number): void;
  /** Pages are built before they are put in the window: dropped only once gone. */
  shown?: boolean;
  born?: number;
}

const live = new Set<Animated>();
let frame: number | null = null;
let last = performance.now();
const start = performance.now();

function loop(now: number) {
  frame = null;
  const dt = Math.min(0.05, (now - last) / 1000);
  last = now;
  const t = (now - start) / 1000;
  for (const a of live) {
    if (!a.el.isConnected) {
      // Built but not shown yet: wait for it (a few seconds at most).
      if (a.shown || now - (a.born ??= now) > 5000) live.delete(a);
      continue;
    }
    a.shown = true;
    a.draw(dt, t);
  }
  if (live.size > 0 && document.visibilityState === "visible") frame = requestAnimationFrame(loop);
}

function wake() {
  if (frame === null && live.size > 0 && document.visibilityState === "visible") {
    last = performance.now();
    frame = requestAnimationFrame(loop);
  }
}

document.addEventListener("visibilitychange", wake);

function add(a: Animated) {
  live.add(a);
  wake();
}

function fit(canvas: HTMLCanvasElement): { w: number; h: number; ctx: CanvasRenderingContext2D | null } {
  const dpr = Math.min(2, window.devicePixelRatio || 1);
  const w = canvas.clientWidth;
  const h = canvas.clientHeight;
  if (canvas.width !== Math.round(w * dpr) || canvas.height !== Math.round(h * dpr)) {
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
  }
  const ctx = canvas.getContext("2d");
  ctx?.setTransform(dpr, 0, 0, dpr, 0, 0);
  return { w, h, ctx };
}

function rgba(hex: string, alpha: number): string {
  const [r, g, b] = hexToRGB(hex);
  return `rgba(${Math.round(r * 255)},${Math.round(g * 255)},${Math.round(b * 255)},${Math.max(0, Math.min(1, alpha))})`;
}

// ── The backdrop ──────────────────────────────────────────────────────────────

interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  r: number;
  depth: number;
  phase: number;
  white: boolean;
}

/** The deep black, the colour and the particles, on `canvas`; `pointerFrom` is
 *  the element whose pointer pushes the particles. */
export function backdrop(canvas: HTMLCanvasElement, color: string, pointerFrom: HTMLElement, radius = 18) {
  let particles: Particle[] = [];
  let pointer: { x: number; y: number } | null = null;
  let strength = 0;
  pointerFrom.addEventListener("pointermove", (e) => {
    const r = canvas.getBoundingClientRect();
    pointer = { x: e.clientX - r.left, y: e.clientY - r.top };
  });
  pointerFrom.addEventListener("pointerleave", () => (pointer = null));

  const seed = (w: number, h: number) => {
    const count = Math.round(Math.min(90, Math.max(24, (w * h) / 2600)));
    particles = Array.from({ length: count }, () => ({
      x: Math.random() * w,
      y: Math.random() * h,
      vx: (Math.random() - 0.5) * 6,
      vy: -2 - Math.random() * 6,
      r: 0.5 + Math.random() * 1.4,
      depth: 0.35 + Math.random() * 0.65,
      phase: Math.random() * Math.PI * 2,
      white: Math.random() < 0.35,
    }));
  };

  add({
    el: canvas,
    draw(dt, t) {
      const { w, h, ctx } = fit(canvas);
      if (!ctx || w === 0) return;
      if (particles.length === 0) seed(w, h);
      strength += ((pointer ? 1 : 0) - strength) * Math.min(1, dt * 4);
      ctx.clearRect(0, 0, w, h);
      ctx.save();
      ctx.beginPath();
      ctx.roundRect(0, 0, w, h, radius);
      ctx.clip();
      ctx.fillStyle = "#020203";
      ctx.fillRect(0, 0, w, h);
      const lin = ctx.createLinearGradient(w, 0, w * 0.35, h);
      lin.addColorStop(0, rgba(color, 0.1));
      lin.addColorStop(1, "rgba(0,0,0,0)");
      ctx.fillStyle = lin;
      ctx.fillRect(0, 0, w, h);
      const breath = 0.85 + 0.15 * Math.sin(t * 0.9);
      const rad = ctx.createRadialGradient(w * 0.92, -h * 0.15, 0, w * 0.92, -h * 0.15, Math.max(w, h) * 0.55 * breath);
      rad.addColorStop(0, rgba(color, 0.3 * breath));
      rad.addColorStop(0.5, rgba(color, 0.08));
      rad.addColorStop(1, "rgba(0,0,0,0)");
      ctx.fillStyle = rad;
      ctx.fillRect(0, 0, w, h);

      for (const p of particles) {
        // Drift, always; the pointer pushes them away.
        p.x += p.vx * dt * p.depth;
        p.y += p.vy * dt * p.depth;
        let alpha = (0.22 + 0.5 * (0.5 + 0.5 * Math.sin(t * (1.2 + p.depth) + p.phase))) * p.depth;
        let r = p.r;
        if (pointer && strength > 0.01) {
          const dx = p.x - pointer.x;
          const dy = p.y - pointer.y;
          const d = Math.hypot(dx, dy);
          if (d < 140 && d > 0.1) {
            const push = (1 - d / 140) * 90 * strength * dt;
            p.x += (dx / d) * push;
            p.y += (dy / d) * push;
          }
          if (d < 200) {
            const near = (1 - d / 200) * strength;
            alpha += near * 0.6;
            r *= 1 + near * 0.8;
          }
        }
        if (p.y < -4) p.y = h + 4;
        if (p.y > h + 4) p.y = -4;
        if (p.x < -4) p.x = w + 4;
        if (p.x > w + 4) p.x = -4;
        const tint = p.white ? "#FFFFFF" : color;
        if (r > 1.1) {
          ctx.fillStyle = rgba(tint, Math.min(1, alpha) * 0.12);
          ctx.beginPath();
          ctx.arc(p.x, p.y, r * 2.6, 0, Math.PI * 2);
          ctx.fill();
        }
        ctx.fillStyle = rgba(tint, Math.min(1, alpha));
        ctx.beginPath();
        ctx.arc(p.x, p.y, r, 0, Math.PI * 2);
        ctx.fill();
      }
      ctx.restore();
    },
  });
}

// ── Groovy ────────────────────────────────────────────────────────────────────

const EMOTES: BotEmoteName[] = ["love", "happy", "wink", "proud", "surprised"];

/** Groovy on the band's floor, in the right part of `canvas`: he strolls, hops,
 *  emotes now and then, follows the pointer with his eyes and reacts when poked. */
export function bandGroovy(canvas: HTMLCanvasElement, color: string, state: BotStateName, width: number) {
  const engine = new BotEngine();
  engine.bodyColor = hexToRGB(color);
  engine.setState(state, true);
  const size = 64;
  let x = -1;
  let target = 0;
  let dir = 1;
  let hop = 0;
  let hopV = 0;
  let pause = 1 + Math.random() * 2;
  let nextEvent = 2 + Math.random() * 4;
  let pointer: { x: number; y: number } | null = null;
  canvas.addEventListener("pointermove", (e) => {
    const r = canvas.getBoundingClientRect();
    pointer = { x: e.clientX - r.left, y: e.clientY - r.top };
  });
  canvas.addEventListener("pointerleave", () => (pointer = null));
  canvas.addEventListener("click", (e) => {
    const r = canvas.getBoundingClientRect();
    if (Math.abs(e.clientX - r.left - x) < size * 0.4) {
      engine.slap();
      hopV = 260;
    }
  });
  const roams = state === "idle";

  add({
    el: canvas,
    draw(dt, t) {
      const { w: cw, h: ch, ctx } = fit(canvas);
      if (!ctx || cw === 0) return;
      const lo = Math.max(0, cw - width - 18) + size / 2;
      const hi = cw - 18 - size / 2;
      if (x < 0) x = target = lo + (hi - lo) * 0.6;
      ctx.clearRect(0, 0, cw, ch);
      if (roams) {
        if (Math.abs(target - x) < 1) {
          pause -= dt;
          if (pause <= 0) {
            target = lo + Math.random() * Math.max(1, hi - lo);
            pause = 1.5 + Math.random() * 3.5;
          }
        } else {
          const step = Math.sign(target - x) * Math.min(Math.abs(target - x), 28 * dt);
          x += step;
          dir = Math.sign(step) || dir;
          if (hop === 0 && hopV === 0) hopV = 70;
        }
      }
      nextEvent -= dt;
      if (nextEvent <= 0) {
        nextEvent = 4 + Math.random() * 6;
        const roll = Math.random();
        if (roll < 0.35) hopV = 200;
        else if (roll < 0.6) engine.triggerEmote(EMOTES[Math.floor(Math.random() * EMOTES.length)]);
        else if (roll < 0.75) engine.greet();
      }
      if (hopV !== 0 || hop > 0) {
        hopV -= 900 * dt;
        hop = Math.max(0, hop + hopV * dt);
        if (hop === 0 && hopV < 0) {
          hopV = 0;
          engine.squash();
        }
      }
      if (pointer) {
        engine.lookX = Math.max(-1, Math.min(1, (pointer.x - x) / 160));
        engine.lookY = Math.max(-1, Math.min(1, (pointer.y - (ch - size * 0.6)) / 120));
      } else {
        engine.lookX = dir * 0.5;
        engine.lookY = 0.1 + 0.1 * Math.sin(t);
      }
      engine.update(dt);
      const floor = ch - 8;
      ctx.save();
      ctx.translate(x - size / 2, floor - size * 0.78 - hop);
      ctx.fillStyle = `rgba(0,0,0,${0.35 * (1 - Math.min(1, hop / 60))})`;
      ctx.beginPath();
      ctx.ellipse(size / 2, size * 0.78 + hop - 1, size * 0.3, size * 0.05, 0, 0, Math.PI * 2);
      ctx.fill();
      engine.draw(ctx, size, size);
      ctx.restore();
    },
  });
}

/** Groovy alone (the sidebar's logo, an empty page). */
export function liveGroovy(canvas: HTMLCanvasElement, color = "#F5F6F8", state: BotStateName = "idle") {
  const engine = new BotEngine();
  engine.bodyColor = hexToRGB(color);
  engine.setState(state, true);
  canvas.addEventListener("click", () => engine.slap());
  let mouse: { x: number; y: number } | null = null;
  window.addEventListener("pointermove", (e) => {
    const r = canvas.getBoundingClientRect();
    mouse = { x: e.clientX - (r.left + r.width / 2), y: e.clientY - (r.top + r.height / 2) };
  });
  add({
    el: canvas,
    draw(dt) {
      const { w, h, ctx } = fit(canvas);
      if (!ctx || w === 0) return;
      if (mouse) {
        engine.lookX = Math.max(-1, Math.min(1, mouse.x / 300));
        engine.lookY = Math.max(-1, Math.min(1, mouse.y / 300));
      }
      engine.update(dt);
      ctx.clearRect(0, 0, w, h);
      engine.draw(ctx, w, h);
    },
  });
  return engine;
}

// ── Page themes ───────────────────────────────────────────────────────────────

export interface Theme {
  color: string;
  /** What Groovy is doing on the page's band. */
  state?: BotStateName;
}

export const THEMES: Record<string, Theme> = {
  dictation: { color: "#FB923C" },
  notes: { color: "#3B9EFF", state: "working" },
  insights: { color: "#34D399" },
  dictionary: { color: "#A78BFA" },
  snippets: { color: "#F7B32B" },
  styles: { color: "#F472B6" },
  flowsettings: { color: "#8E939C" },
  voice: { color: "#F97316" },
  skills: { color: "#22D3EE", state: "working" },
  assistant: { color: "#7C5CFF" },
  island: { color: "#3B9EFF" },
  wardrobe: { color: "#F472B6" },
  models: { color: "#22D3EE", state: "thinking" },
  memory: { color: "#A78BFA" },
  reminders: { color: "#F7B32B" },
  music: { color: "#1DB954" },
  calls: { color: "#F97316" },
  chat: { color: "#E07950", state: "thinking" },
  agents: { color: "#3B9EFF", state: "working" },
  pills: { color: "#F5A524" },
  phone: { color: "#3DDC84" },
  breaks: { color: "#34D399", state: "sleeping" },
  general: { color: "#8E939C" },
  permissions: { color: "#EF4444" },
  system: { color: "#8E939C" },
};

/** The band itself: title, line, actions, and Groovy. */
export function hero(page: string, title: string, subtitle: string | null, actions: HTMLElement[] = [], compact = false): HTMLElement {
  const theme = THEMES[page] ?? THEMES.general;
  const el = document.createElement("div");
  el.className = compact ? "hub-hero compact" : "hub-hero";
  el.style.setProperty("--hero-line", rgba(theme.color, 0.16));
  const back = document.createElement("canvas");
  back.className = "backdrop";
  const bot = document.createElement("canvas");
  bot.className = "troupe";
  const copy = document.createElement("div");
  copy.className = "copy";
  const h1 = document.createElement("h1");
  h1.textContent = title;
  copy.append(h1);
  if (subtitle) {
    const p = document.createElement("p");
    p.textContent = subtitle;
    copy.append(p);
  }
  if (actions.length) {
    const row = document.createElement("div");
    row.className = "actions";
    row.append(...actions);
    copy.append(row);
  }
  el.append(back, bot, copy);
  backdrop(back, theme.color, el);
  bandGroovy(bot, "#F5F6F8", theme.state ?? "idle", compact ? 280 : 340);
  return el;
}

/** A card in a colour: the band's black and particles behind `content`. */
export function glowCard(color: string, ...content: (HTMLElement | string)[]): HTMLElement {
  const el = document.createElement("div");
  el.className = "hub-glow";
  el.style.setProperty("--glow-line", rgba(color, 0.18));
  const canvas = document.createElement("canvas");
  const inner = document.createElement("div");
  inner.className = "inner";
  inner.append(...content);
  el.append(canvas, inner);
  backdrop(canvas, color, el, 14);
  return el;
}

/** An empty page: Groovy asleep, and a line. */
export function emptyState(text: string, state: BotStateName = "sleeping", color = "#F5F6F8"): HTMLElement {
  const el = document.createElement("div");
  el.className = "hub-empty";
  const canvas = document.createElement("canvas");
  const p = document.createElement("div");
  p.textContent = text;
  el.append(canvas, p);
  liveGroovy(canvas, color, state);
  return el;
}
