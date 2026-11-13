# Fixes round 3K: Groovy's own body, "Pillow + wisps" (4 Nov)

> **DECIDED by the owner (4 Nov): pillow outline + element wisps for hands, wisps 15% smaller. Do this TOGETHER with 3H item 1 (the wardrobe fit), in the same pass: 3K items 1 to 2 first, then 3H item 1 on the new body.** Reference renders: `docs/design/body-v2/`. The old teardrop design is withdrawn.

**Why:** Coucou's asset terms reserve the Mochi character's look. Groovy's body was a squircle with oval hands. The glowing almond eyes, aura, colours and growth art are already Groovy's own. This round gives the **silhouette and hands** their own design so the character can be reserved as the owner's (`LICENSE-ASSETS.md`, `docs/LICENSING.md` section 3).

**Keep exactly as is (owner's rule):** Night's bonsai and sakura, Ember's fire, Frost's water inside with marine life and the golden submarine, the stage 6 diamond eyes, all halos, colours, expressions, sounds, wardrobe and settings. No feature is removed. Memory rule: under 300 MB idle, never over 400 MB peak, near-0 CPU idle; no new images or files, everything is drawn in canvas.

## Rules
- **Do not copy from Coucou or Mochi.** Do not open or trace Mochi's artwork. Build from the formula below.
- The design record is `docs/design/body-v2/` (already added). Add dated notes there if anything changes.
- One item per commit: "Fixes round 3K, item N: ...", ticked in `WORK_QUEUE.md` in the same commit, push, keep the build green. Keep Coucou's MIT code (state machine, physics) with its notice.

## Item 1: Pillow outline
- `skins.ts` `SHAPE` -> `{w:1.0, h:1.1, exp:3.0, taper:0}` and make the mini bots use the same shape (smaller, flat colour).
- `engine.ts` `bodyPath`: build the existing superellipse point (`u=px0/rx`, `v=py0/ry`, both -1..1), then apply the pillow bow:
  `px = px0 * (1 - 0.10 * (1 - v*v))`, `py = py0 * (1 - 0.10 * (1 - u*u))`.
  Sides bow in a little at the middle, corners stay full. Keep the `morph` blend to the outfit rounded-rect. Put 0.10 in a named constant `PILLOW_BOW`.
- Keep growth art anchors working (sprout/bonsai top, flame, water fill clip, stage 6 eyes): they must follow the new path.

## Item 2: Element wisps instead of hands
- In `drawHandsBehind` (and any front-hand drawing) replace the oval hand with an element wisp drawn at the same position/rotation, so wave, point, hold, hug and tool poses keep their timing:
  - **Night:** a leaf (pointed both ends, centre vein), mint to green gradient, green glow.
  - **Ember:** a flame lick (pointed top, wavy), pale yellow to orange, orange glow.
  - **Frost:** a droplet (pointed top, round base), white to ice blue, blue glow.
  - Each has a small soft highlight. Size `max(hew, heh) * 1.25 * WISP_SCALE` with `WISP_SCALE = 0.85` (named constant, one number to tune). Gentle sway: rotate by `sd*0.35 + sin(t*6+sd)*0.08`. Glow via the existing `shadowBlur` approach (`R*0.3`).
- Drawn only where hands are drawn today (not at mini/compact sizes), so there is no extra cost at idle.

## Item 3: Wardrobe fit on the new body (this IS 3H item 1)
- Do 3H item 1 now, anchoring to the new pillow head: extend `Head` (`outfits.ts` `makeHead`) with honest anchors from the live body (`crown`, `eyeLine`, `neck`, `base`), keep old fields working, and re-seat every item.

## Item 4: Checks
- CI contact sheet (`[build]` commit): each look at stages 1 to 6, expressions idle/happy/thinking/surprised/sleepy, collapsed and open island, and one with a hat and one with a held item. Confirm: growth art unchanged, wisps read at the open island size, wearables sit correctly, no memory or CPU regression.
- Update any test asserting the old hand shape or old outline.
- Tell the owner in `STATUS.md` when the contact sheet is ready to look at.
