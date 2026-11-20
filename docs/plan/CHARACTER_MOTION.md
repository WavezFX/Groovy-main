# Character motion: what is ported, and Groovy's own design (round 4E item 1, 11 Nov 2026)

`docs/GROOVY_SPEC.md` section 2: the animation engine must be original motion; anything ported is rewritten. The growth art, the three looks and the pillow outline don't change.

**State of this item (20 Nov 2026):** the design in section 2 was **approved by the owner on 20 Nov 2026**. The engine is rewritten from it (section 4). Also rewritten: `minibots.ts`. Still to do: `core/anim.ts`'s easing table and the flight in `desktop-logic.ts` (section 3, step 3), and the CI numbers (P1, P10).

## 1. What is ported today
`windows/src/mochi/engine.ts` (1597 lines) was compared, method by method, with the engine of the project Groovy's code started from (whitespace and comments ignored). Of that engine's 1376 lines, about 1230 are still in Groovy's file unchanged. By method (lines of code, share identical):

| Method | Lines | Same as the original | What it is | Verdict |
|---|---|---|---|---|
| `update` | 124 | 96 % | every frame: blink timer, breathing, eye tracking, state tweens | **ported** |
| `draw` | 76 | 69 % | draw order, transforms (Groovy added the head model, growth lift, wisps) | **ported**, partly Groovy's |
| `drawBadge` | 64 | 85 % | the state badge beside him | **ported** |
| `drawParticles` | 49 | 100 % | every particle's shape and path | **ported** |
| `triggerEmote` | 47 | 98 % | the seven emotes' keyframes | **ported** |
| `drawEyes` | 44 | 71 % | eye shapes per state (Groovy added almond eyes and pitch) | **ported**, partly Groovy's |
| `setState` | 36 | 97 % | transitions between the states | **ported** |
| `drawMouth` | 35 | 100 % | mouths per mood | **ported** |
| `doMiniBehaviorLoop` | 33 | 100 % | small random idle behaviours | **ported** |
| `drawBody` | 30 | 76 % | body fill, shine (Groovy added the pillow and glass) | **ported**, partly Groovy's |
| `greet` | 29 | 100 % | the hello sequence | **ported** |
| `setOutfit`, `slap`, `gulp`, `squash`, `blink`, `doRoll`, `applyDance`, `setBadge`, `setPermanentEmote`, `interruptGreet`, `animateMorph`, `resetMorph` | 123 | 100 % | squash and stretch, the file-drop gulp, the roll, the dance | **ported** |
| `emit`, `anim` | 24 | 92–97 % | particle spawner, tween runner | **ported** |
| `bodyPath` | 21 | 71 % | outline (now the pillow) | Groovy's shape, ported scaffolding |
| `handTargets`, `drawCachedBody`, `fillBody`, `drawSkinMouth`, `eyeCentres`, `eyePitch`, `moving`, `style`, `setStage`, `setSkin` | 143 | not in the original | wisps, growth stages, looks, the cached body | **Groovy's own** |

Weighted by lines, about **three quarters of the engine's methods are the original's**. Also shared with the original, and to be rewritten with it: `core/anim.ts` (the easing table, 101 of its distinct lines), the state and emote names and timings in `core/layout.ts`, `mochi/minibots.ts`, and the flight and drag maths in `mochi/desktop-logic.ts` (232 lines).

Already Groovy's own (no line in the original): `mochi/skins.ts`, `pillow.ts`, `growth.ts`, `stage6.ts`, `appearance.ts`, `sprites.ts`, `seed.ts`, `art/*`, and almost all of `outfits.ts` and `wardrobe.ts`.

## 2. Groovy's own motion: the design to approve
One idea runs through it: **he is a soft pillow with an element living in him.** The body moves like something stuffed (it bows and puffs at the corners, it never squashes like a ball), and the element (leaf, flame, droplet) does the expressive work. Three motion tools only, all Groovy's: a spring (frequency and damping, as the wardrobe's `swing` already uses), a sine breath, and a cubic ease-out. No keyframe tables.

| What | Today (ported) | Groovy's own |
|---|---|---|
| Breathing | a symmetric scale pulse | **Pillow breath:** 4.2 s, uneven: in 1.6 s, hold 0.3 s, out 2.3 s. The corners fill (`PILLOW_BOW` 0.10 → 0.07) and the sides ease out; the height changes under 1.5 %. The halo follows the in-breath. Asleep: 6.5 s. |
| Blink | lids drop and rise evenly | **Per look.** Night: a shutter, the almond narrows from its outer corners to a line in 70 ms and opens in 110 ms with a small overshoot; a double blink one time in eight. Ember: the glow dips first (90 ms), then the lid. Frost: slow (160 ms down, 220 ms up) with one ripple across the water line. Every 3.5–9 s, never on a beat. |
| Resting | small random behaviours from a list | **The element idles, the body doesn't.** Leaf sways ±6° over 5.5 s; flame licks on a 0.9 s flicker; droplet bobs over 2.4 s. Every 20–40 s one small gesture of the look (leaf turns to the light, flame gutters and recovers, droplet rounds and falls back). |
| Looking | eyes follow the pointer | Eyes lead and the head follows a beat later on a spring (the head model's yaw and pitch already exist); eyes glance at the pointer only when it comes near or stops, not all the time. |
| Working | a looping wobble | Wisps "knit": two small alternating circles, quickening with activity; body still. |
| Thinking | dots and a sway | Eyes drift up along a slow figure of eight (2.8 s); the halo breaks into three dots that orbit once per breath. |
| Searching | a left-right sweep | The element leans and scans in an arc (leaf like a compass needle, flame leaning, droplet stretching); eyes follow it. |
| Approval, question | bounce, tilt | Approval: he leans toward the user (pitch +0.08) and the halo beats twice every 2 s. Question: head tilts 7° on a soft spring and the wisp curls into a hook. |
| Happy | a hop | **Puff:** corners fill fast (90 ms), he lifts 6 % and settles on the spring; the element pops (two leaves, a spark burst, three bubbles). |
| Love | hearts | Look-own particles spiral up and fade: leaves, embers, bubbles. No hearts. |
| Surprised | eyes widen, jump | Eyes open to 1.25× in 60 ms, the outline goes taut (bow 0.10 → 0.02), wisps flare outward, then all three settle at different speeds. |
| Proud | chest out | He rises on the in-breath and holds it 0.8 s; one halo ring opens outward and fades. |
| Wink | one eye closes | One eye shutters; the wisp on the other side flicks. |
| Yawn | mouth opens | The whole pillow sags for 1.2 s (bow 0.10 → 0.16), eyes squeeze to crescents, the element droops, then a slow refill. |
| Annoyed | shake | Lids flatten to a straight top edge, wisps drop, three short sideways twitches 40 ms apart. |
| Squash, gulp, roll, dance, hello | squash-and-stretch set pieces | **Pillow set pieces:** the file-drop gulp is the sides bowing in and popping back; the roll is a tumble with the wisps trailing a quarter-turn behind; the dance is a sway from the base with the element keeping the beat; hello is a two-beat wisp wave with a puff. |
| Particles | one shared set | **Per look**, drawn from the look's own art: leaf flecks, sparks, bubbles; the same count limits as today. |
| Mouth | a set of mouth shapes per mood | Keep `drawSkinMouth` (Groovy's) only; the older `drawMouth` set goes. |
| Badge | a badge beside him | Redrawn as a small tag hanging from his outline that swings on the wardrobe's pendulum. |

Rules that stay: every loop stops at rest (idle CPU at most 0.3 %, P1), springs snap to exactly 0, the sprite size animates nothing, frame time p95 at most 20 ms (P10). State and emote **names** stay (`idle`, `working`, `thinking`…, `happy`, `wink`…) so the island's code doesn't change.

## 3. The rewrite (after approval)
1. A contact sheet of the design (`scripts/body-sheet.mjs`: each state and emote, eight frames, three looks) for the owner.
2. New `engine.ts` written from section 2, not from the old file: the frame update, state changes, emotes, particles and set pieces. Keep the public methods' names and the parts marked Groovy's own in section 1.
3. Replace `core/anim.ts`'s easing table with the three tools above; then `minibots.ts` and the flight in `desktop-logic.ts` on the same spring.
4. Re-run the comparison in section 1: **no method may be more than 20 % the same**, and write the new table here.
5. `body-sheet.mjs`, `items-check.mjs`, wardrobe and growth tests, frame test.

## 4. The rewrite (done 20 Nov 2026, round 4E item 1)
`windows/src/mochi/engine.ts` was rewritten from section 2, not from the old file. The motion tools are in a new file, `windows/src/mochi/motion.ts`: `stepSpring` (frequency and damping, snaps to exactly its target), `breath` (4.2 s, in 1.6 / hold 0.3 / out 2.3), `envelope` and `easeOut` (cubic). A move is an **act**: a short function of its age that adds offsets; the springs chase base + offsets, which gives the anticipation, overshoot and follow-through. There are no keyframe tables and no `setTimeout` chains (timed steps are acts too, so they stop with the frame loop).

| Method or part | Verdict now | What it does now |
|---|---|---|
| `update` | **Groovy's own** | glides, acts, eyes-lead/head-follows springs, per-look blink timing (3.5–9 s, never at the top of a breath, double 1 in 8), idle element gesture every 20–40 s, glance on pointer rest, soft-part and pendulum springs |
| `draw` | **Groovy's own** | adds the breath (height under 1.5 %, held while proud), halo pulse and approval double beat, thinking dots orbiting once per breath; same draw order for the growth art and wardrobe |
| `setState` | **Groovy's own** | finished: puff + element pop; error: three 40 ms twitches; approval: dip then lean (+0.08 pitch); question: 7° tilt on a soft spring, wisp hook; sleeping: yawn sag |
| `triggerEmote` | **Groovy's own** | happy puff, love spiral of look particles (no hearts), surprised taut outline + flare, proud held breath + halo ring, wink + opposite wisp flick, yawn sag, annoyed twitches |
| `blink` | **Groovy's own** | Night shutter 70/110 ms with overshoot; Ember glow dip then lid; Frost 160/220 ms with a ripple |
| `squash`, `gulp`, `slap`, `doRoll`, `greet`, `interruptGreet` | **Groovy's own** | pillow set pieces: sides bow in and pop; tumble with wisps a quarter-turn behind; hello = puff + two-beat wisp wave |
| `stepDanceLevel`, `danceTransform`, `applyDance` | **Groovy's own** | a sway from the base at 104 BPM, a puff after each beat, wisps keep the beat |
| `emit`, particles | **Groovy's own** | per look: leaf flecks, embers, bubbles; plus drops, z, halo ring; at most 24 alive (12 lite) |
| `drawEyes`, eye glyphs | **Groovy's own** | almond eyes (skins.ts) with the eye lead; mood glyphs redrawn (crescents, flat lids, glints) |
| `drawBadge` → `drawTag` | **Groovy's own** | a tag hanging from his outline on a pendulum |
| `drawMouth` → `drawSlot` | **Groovy's own** | only the drop slot; moods use `drawSkinMouth` |
| `doMiniBehaviorLoop` → `miniCue` | **Groovy's own** | minis puff, twitch, wink or spiral flecks |
| `glide` (was `anim`) | **Groovy's own** | one eased value, cubic ease-out |
| `bodyPath` | **Groovy's own** | the pillow outline with a live bow (taut 0.02 … sagging 0.16); exactly `pillow.ts` at rest |
| `wispPath`, `drawWisp`, `fillBody`, `drawCachedBody`, `eyeCentres`, `eyePitch`, `drawSkinMouth`, `style`, `setStage`, `setSkin` | **Groovy's own** (kept) | unchanged; the body cache key now includes the bow |
| `rrPoint` | geometry helper, kept | ray to rounded-rect, for the drop-box morph |

No row is **ported**. Public names are unchanged (states, emotes, `setState`, `triggerEmote`, `greet`, `gulp`, `slap`, `setOutfit`, `setSkin`, `setStage`, `animateMorph`, `busy`/`ambient`/`moving`, `locks`), so the island, wardrobe, growth and minis need no change. `ambient` is true only for searching, approval, rate limit, sleeping and minis, as before, so idle CPU rules hold: at rest every spring is exactly still and the island stops its frames.

**Contact sheet:** `docs/plan/motion-sheet/motion-{night,ember,frost}.jpg` (`windows/scripts/motion-sheet.mjs`: 18 rows of states and emotes, eight frames over 1.6 s, fixed clock). **Not re-run here:** the line comparison with the original engine (its source isn't in this worktree; re-run it on the reference checkout, target under 20 % per method) and the Windows numbers (P1, P10), which only CI measures.
