// Draws round 5's art (generated from docs/design/wardrobe-v2 by
// scripts/gen-art.py) on Groovy's canvas: SVG path data through Path2D, in
// the boards' own coordinates, mapped onto the engine's body. The boards draw
// Groovy as a 72×62 rounded rect centred on (50, 61); the engine's body is
// centred on the origin with half-sizes rx and ry, so one maps onto the other
// with a scale. Paths are built once and kept.

export interface Shape {
  /** SVG path data, in board units. */
  d: string;
  f?: string;
  s?: string;
  w?: number;
  cap?: 1;
  dash?: number[];
  o?: number;
  /** SVG transform: rotate(a cx cy), translate(x y). */
  t?: string;
  /** Clipped to the body on the board. */
  clip?: 1;
}

export interface Art {
  behind?: Shape[];
  body?: Shape[];
  front?: Shape[];
  /** The eyes' colour and bright centre on this card. */
  iris?: string;
  core?: string;
}

/** The board's body: centre and half-sizes. */
export const BOARD = { cx: 50, cy: 61, hw: 36, hh: 31 } as const;

const PATHS = new WeakMap<Shape, Path2D>();

function pathOf(s: Shape): Path2D {
  let p = PATHS.get(s);
  if (!p) {
    p = new Path2D(s.d);
    PATHS.set(s, p);
  }
  return p;
}

/** Applies an SVG transform list (rotate, translate, scale) to the context. */
export function applyTransform(x: CanvasRenderingContext2D, t: string) {
  for (const m of t.matchAll(/(rotate|translate|scale)\(([^)]*)\)/g)) {
    const n = m[2].split(/[\s,]+/).filter(Boolean).map(Number);
    if (m[1] === "translate") x.translate(n[0] ?? 0, n[1] ?? 0);
    else if (m[1] === "scale") x.scale(n[0] ?? 1, n[1] ?? n[0] ?? 1);
    else {
      const a = ((n[0] ?? 0) * Math.PI) / 180;
      if (n.length >= 3) {
        x.translate(n[1], n[2]);
        x.rotate(a);
        x.translate(-n[1], -n[2]);
      } else x.rotate(a);
    }
  }
}

/** Board space on the body: (50, 61) at the body's centre, the board's body over his. */
export function toBoard(x: CanvasRenderingContext2D, rx: number, ry: number, shiftX = 0) {
  x.translate(shiftX, 0);
  x.scale(rx / BOARD.hw, ry / BOARD.hh);
  x.translate(-BOARD.cx, -BOARD.cy);
}

/** Draws shapes in board space (the caller has called toBoard). */
export function drawShapes(x: CanvasRenderingContext2D, shapes: readonly Shape[] | undefined, only?: "clipped" | "free") {
  if (!shapes) return;
  for (const s of shapes) {
    if (only === "clipped" && !s.clip) continue;
    if (only === "free" && s.clip) continue;
    x.save();
    if (s.t) applyTransform(x, s.t);
    if (s.o !== undefined) x.globalAlpha *= s.o;
    const p = pathOf(s);
    if (s.f) {
      x.fillStyle = s.f;
      x.fill(p);
    }
    if (s.s) {
      x.strokeStyle = s.s;
      x.lineWidth = s.w ?? 1;
      x.lineCap = s.cap ? "round" : "butt";
      x.lineJoin = "round";
      x.setLineDash(s.dash ?? []);
      x.stroke(p);
    }
    x.restore();
  }
}
