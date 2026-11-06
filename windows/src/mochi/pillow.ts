// Groovy's pillow outline (round 3K item 1): a superellipse whose sides bow in
// a little at the middle while the corners stay full. The engine's body path,
// the growth art and the wardrobe's anchors all use this one geometry.

/** How far the pillow outline's sides bow in at the middle. */
export const PILLOW_BOW = 0.1;

/** The outline's point at angle `a` (body space, y down), before any morph. */
export function pillowXY(a: number, rx: number, ry: number, exp: number, taper = 0): [number, number] {
  const expN = 2.0 / exp;
  const ca = Math.cos(a);
  const sa = Math.sin(a);
  const px0 = rx * (ca >= 0 ? Math.pow(ca, expN) : -Math.pow(-ca, expN));
  const py0 = ry * (sa >= 0 ? Math.pow(sa, expN) : -Math.pow(-sa, expN));
  const u = px0 / rx;
  const v = py0 / ry;
  return [
    px0 * (1 - PILLOW_BOW * (1 - v * v)) * (1 + taper * Math.max(0, v)),
    py0 * (1 - PILLOW_BOW * (1 - u * u)),
  ];
}

const widthCache = new Map<string, number>();

/** Half the outline's width at height `y` (body space): where a rim or collar meets him. */
export function halfWidthAt(y: number, rx: number, ry: number, exp: number): number {
  const key = `${rx.toFixed(2)}|${ry.toFixed(2)}|${exp}|${y.toFixed(2)}`;
  const hit = widthCache.get(key);
  if (hit !== undefined) return hit;
  // The right half, top to bottom: find the segment that crosses y.
  let best = 0;
  let prev = pillowXY(-Math.PI / 2, rx, ry, exp);
  for (let i = 1; i <= 180; i++) {
    const p = pillowXY(-Math.PI / 2 + (i / 180) * Math.PI, rx, ry, exp);
    if ((prev[1] - y) * (p[1] - y) <= 0 && p[1] !== prev[1]) {
      const t = (y - prev[1]) / (p[1] - prev[1]);
      best = Math.max(best, prev[0] + (p[0] - prev[0]) * t);
    }
    prev = p;
  }
  if (widthCache.size > 256) widthCache.clear();
  widthCache.set(key, best);
  return best;
}
