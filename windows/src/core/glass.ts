// How see-through the glass island is (FIXES_ROUND3B item 1b): the wallpaper
// and windows behind it must show through, like frosted glass. "blur" is
// Windows' accent blur behind a light tint; "css" is the page's own glass,
// no blur, so a little more tint. Over a very bright backdrop Rust sends a
// boost (0–1, sampled at most once a minute) and the tint rises only that far.

export type GlassStrength = "clear" | "balanced" | "frosted";
export type GlassLook = "solid" | "blur" | "css";

// Round 3C item 1: frosted, not clear. Over a real blur (Windows 11's host
// backdrop, or the accent blur where it stays in the shape) the tint is
// rgba(20,22,28,0.45) at Balanced; with no blur at all it is opaque enough
// (about 0.80) that nothing behind can be read.
const TINT: Record<"blur" | "css", Record<GlassStrength, number>> = {
  blur: { clear: 0.35, balanced: 0.45, frosted: 0.58 },
  css: { clear: 0.78, balanced: 0.8, frosted: 0.85 },
};
/** The most a bright backdrop adds to the tint: enough that white text keeps 4.5:1 over a white page. */
const MAX_BOOST = 0.3;
/** The tint's colour. */
export const TINT_RGB = [20, 22, 28] as const;

export function glassStrength(value: unknown): GlassStrength {
  return value === "clear" || value === "frosted" ? value : "balanced";
}

/** The island's background alpha; Solid is opaque. */
export function glassAlpha(look: GlassLook, strength: GlassStrength, boost = 0): number {
  if (look === "solid") return 1;
  const b = Number.isFinite(boost) ? Math.min(1, Math.max(0, boost)) : 0;
  return Math.min(0.97, Math.round((TINT[look][strength] + b * MAX_BOOST) * 100) / 100);
}
