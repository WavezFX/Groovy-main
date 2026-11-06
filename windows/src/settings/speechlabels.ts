// The Parakeet warnings, shared by the Voice page and the hub's speech pages.
// Kept out of voice.ts so importing them doesn't load that page's code at
// start (round 3D item 9: a page's code is fetched on its first visit).
import { N_ } from "../i18n/i18n";

export const PARAKEET_LABEL = N_("Parakeet: high accuracy (uses more memory)");
export const PARAKEET_OVER_LIMIT = N_("Parakeet uses about 670 MB while it listens: over Groovy's 400 MB limit. Whisper (light) keeps Groovy under it.");
