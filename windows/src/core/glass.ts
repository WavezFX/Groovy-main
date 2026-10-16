// How see-through the glass island is (FIXES_ROUND3B item 1b): the wallpaper
// and windows behind it must show through, like frosted glass. "blur" is
// Windows' accent blur behind a light tint; "css" is the page's own glass,
// no blur, so a little more tint. Over a very bright backdrop Rust sends a
// boost (0–1, sampled at most once a minute) and the tint rises only that far.

export type GlassStrength = "clear" | "balanced" | "frosted";
export type GlassLook = "solid" | "blur" | "css";

const TINT: Record<"blur" | "css", Record<GlassStrength, number>> = {
  blur: { clear: 0.26, balanced: 0.34, frosted: 0.42 },
  css: { clear: 0.44, balanced: 0.52, frosted: 0.62 },
};
/** The most a bright backdrop adds to the tint. */
const MAX_BOOST = 0.18;

export function glassStrength(value: unknown): GlassStrength {
  return value === "clear" || value === "frosted" ? value : "balanced";
}

/** The island's background alpha; Solid is opaque. */
export function glassAlpha(look: GlassLook, strength: GlassStrength, boost = 0): number {
  if (look === "solid") return 1;
  const b = Number.isFinite(boost) ? Math.min(1, Math.max(0, boost)) : 0;
  return Math.round((TINT[look][strength] + b * MAX_BOOST) * 100) / 100;
}
