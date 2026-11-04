# Fixes round 3K: Groovy's own body (4 Nov)

> **STATUS (4 Nov, evening): DO NOT START THIS ROUND YET. The body outline is not decided.**
> The owner rejected, after seeing them: the teardrop ("drop"), the glass flask, orb and lantern (plant/lotus inside), and a pudding, dome and gem outline drawn on the real renderer. The owner wants the growth art kept exactly as it is: Night's bonsai and sakura, Ember's fire, Frost's water inside with marine life and the golden submarine, and the diamond eyes (stage 6). **Decided: hands become element wisps** (a vine, a flame lick, a water tendril that appear only when he gestures). **Open:** the outline. Next step is the owner comparing today's Groovy with Coucou's Mochi; if they already look clearly different, the outline may stay and only the hands, face construction and details change. The "design" section below is the OLD default and is withdrawn; it will be rewritten once the owner decides.

**Why:** Coucou's asset terms (`LICENSE-ASSETS.md` in its repo) reserve the Mochi character's design, look, expressions and animations. Groovy's body is still Mochi-derived: `engine.ts` `bodyPath` draws the same squircle (a rounded superellipse, `R*1.14` by `R*0.88`) with the same face-and-hands construction, ported from Coucou's Swift view, and the mini bots keep "the original" shape. The elemental looks, growth and wardrobe are Groovy's own, but they sit on that base. This round gives Groovy **its own base**, so the name, character and art can be reserved as the owner's (`LICENSE-ASSETS.md`, `docs/LICENSING.md` section 3).

**Do this BEFORE 3H item 1** (the wardrobe fit anchors to the head, so it must be built on the new body). One item per commit: "Fixes round 3K, item N: ...", tick it in `WORK_QUEUE.md` in the same commit, push, keep the build green. No feature is removed: every state, animation, sound, wardrobe item, growth stage and setting keeps working. Only how he looks changes. Memory rule as always: under 300 MB idle, never over 400 MB at peak, near-0 CPU idle.

## Rules for this round
- **Do not copy from Coucou or Mochi.** Do not open or trace Mochi's artwork; do not reuse the squircle exponent, the proportions or the face layout. Build from the new design below.
- **Keep a design record** in `docs/design/body-v2/` (dated notes, early studies, the reasons for each choice). It is the evidence that the design is independent.
- Keep the code that is Coucou's MIT code (state machine, physics) with its notice; it is the *look* that changes.

## The design (default direction; the owner may change it before this is pushed)
**"The drop":** one teardrop silhouette that all three looks share. A seed, a flame and a water drop are all teardrops, and Groovy hatches from a seed, so the shape tells the story.
- **Body:** a teardrop: a soft point at the top, a full rounded base, slightly taller than wide (not a blob). The point leans with movement (secondary motion) and sprouts the look's growth: Night a leaf-bud, Ember a flame tip, Frost a droplet crest. Squash and stretch follow the point.
- **Face:** the existing sharp almond eyes stay (they are Groovy's own) but sit lower and wider-set, with a small mouth built from a single curve. **No ears. No blush cheeks.** Feelings come from the eyes, the glow and the look's own particles (leaves, sparks, bubbles), not hearts and stars.
- **Hands:** none by default. When he points, waves or holds something, short element-made "wisps" (a vine, a flame lick, a water tendril) extend from the base and retract. This replaces the stub hands.
- **Material:** each look keeps its colour and glow (Night green glow, Ember warm, Frost cool). Surface shading is drawn from the teardrop's own light direction, not copied from the old gradient.

## Item 1: Design study and record (no app change)
- Make 3 to 5 studies of the teardrop as small canvas/SVG renders (front, three-quarter, side, squash, stretch) for each look, in `docs/design/body-v2/` with a short `README.md` (date, intent, what was rejected and why).
- Output a contact-sheet image of the studies from CI for the owner. No app code changes in this item.

## Item 2: The new base body (then STOP for the owner's approval)
- Replace the squircle: `bodyPath` becomes a parametric teardrop (control points per look and growth stage). Remove the superellipse exponent, `taper` and `rrPoint` morph code that only served the old shape, or rebuild what the outfit "rigid roll" needs on the new shape.
- Extend `Head` (`outfits.ts` `makeHead`) with honest anchors from the live body: `crown` (the point), `eyeLine`, `neck`, `base`. Keep the existing fields working so nothing breaks before 3H item 1 re-fits the wardrobe.
- Mini bots use the same teardrop (smaller, flat colour).
- Commit with `[build]`. CI renders a contact sheet: each look at stages 1 to 6, front and sideways, and a few expressions.
- **STOP after this item.** Write in `STATUS.md`: "Waiting for the owner to approve the new body." Do not start item 3 until the owner says it is approved.

## Item 3: The face and expressions
- Rebuild the mouth and the expression set on the new face: each existing state (idle, listening, thinking, speaking, happy, love, surprised, proud, wink, tired, searching and the rest in `engine.ts`) keeps its meaning and timing, drawn with the new eyes, glow and element particles instead of hearts, stars and blush. Same triggers, same sounds.
- Update any test that asserts the old shapes or particles.

## Item 4: Wisps instead of hands
- Replace `drawHandsBehind` and the front hands with element wisps (vine, flame lick, water tendril). Same gestures (wave, point, hold, hug), same timing, same tool-use poses.

## Item 5: Growth stages 1 to 6 and wardrobe anchors on the new body
- Stages 1 to 5 grow out of the drop (bud to bonsai crown, flicker to flame, droplet to tide). Stage 6 (sakura bonsai, golden flame, Diamond sea with shark and submarine) keeps its art, re-seated on the new shape.
- Keep `growth.ts` and per-look growth unchanged (running time, own record per look).
- Update the contact-sheet test so every wardrobe item still sits on the new head (3H item 1 then improves the fit).

## Item 6: Remove the old art and prove it
- Delete everything left of the old shape and face (unused paths, constants, hands, blush, the old "minis keep the original" branch, old screenshots and fixtures that show the old body). `knip` and `cargo machete` clean.
- Check `git grep` for the old exponent/proportions and for any Coucou or Mochi artwork or name in user-visible text, screenshots or installer images.
- Update `README`, the store/branding images and the app icon if they show the old body.

## Item 7: Checks
- Every state renders; frame-time test passes; idle and peak memory within budget; CPU near 0 at idle; all existing tests pass; contact sheets for looks, stages, wardrobe and expressions attached in CI.
- `docs/design/body-v2/` is complete (the design record).
- Tick `LICENSING.md` section 3 as done and tell the owner it can now be reserved in `LICENSE-ASSETS.md` (the line about Mochi-derived parts is then removed).
