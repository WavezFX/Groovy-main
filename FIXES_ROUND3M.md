# Fixes round 3M: the wardrobe is worn, not stuck on (4 Nov, Opus)

**Why:** the owner tested build 217 (0.6.0) on his PC. Even after 3H and 3L, the clothes still look stuck on and don't move like they would on a real head. What he saw in the app (Night look, open island, wardrobe page):
- **Acorn cap / pumpkin / mushroom:** they sit on top like lids. The mushroom's pale underside reads as a see-through band between the cap and his head. Hats don't sink onto the crown, and there's no back of the hat.
- **Eye patch:** the strap is a short diagonal line that stops mid-face. It doesn't go round his head.
- **Headphones:** the band floats above his head with a gap. The cups don't turn with him.
- **Tie:** it's drawn up over his face, between his eyes. A neck item must never cover his eyes.
- **Scarf / collar / bow tie:** they're flat strips across the bottom edge.
- **Motion:** nothing moves like real clothing. No settle after a hop, no swing on dangling parts, no squash with his body.

**The owner's rule:** every item must look and move as if a real head were wearing it. If something goes round his head, it goes all the way round: the front is drawn on him, the back is hidden behind him, and there are **no transparent gaps or blank bands**. Keep the hand-drawn 2D look (no 3D engine, no new image files). Memory budget: under 150 MB idle, never over 300 MB peak, near-0 CPU at idle. Don't touch the wisps, growth art, eyes, glow or the pillow body.

One item per commit: "Fixes round 3M, item N: ...", ticked in `WORK_QUEUE.md` in the same commit, push, keep the build green. Main file: `windows/src/mochi/outfits.ts` (it already has `Head`, `headAnchors`, behind/body/front layers, `RIMMED`, `COLLARED`, `ARMED`, a spring lag). Build on it. Don't start over.

## Item 1: One head model for everything (fake 3D, drawn in 2D)
- Give `Head` a simple 3D head: an ellipsoid matching the pillow (`rx`, `ry`, depth about 0.8 × rx), centred on the body, rotated by the **same** yaw/pitch/roll the eyes use.
- Define every anchor as a point on that ellipsoid (latitude/longitude), not as 2D offsets: crown, brow line, eye line, ears (left/right sides at eye height), neck line (just above the base, **below the eyes with a gap of at least 0.12 × ry**).
- Project to 2D each frame. A point is visible when its surface normal faces the viewer. Use this to split every wrapping part into a front piece (drawn after the body) and a back piece (drawn before the body, hidden by it). Remove the old per-item yaw hacks this replaces.

## Item 2: Hats are worn
- A hat's rim is a ring on the ellipsoid at the hat's seat latitude. The front arc is drawn over his head and the back arc goes behind it. The crown of the hat covers the top of the head with **no gap**: fill down to the seat ring, clipped to the outline so nothing shows through.
- No see-through undersides. The mushroom shows a solid, shaded underside only where the rim tips up towards the viewer. Acorn, pumpkin, top hat, bucket hat, caps and crowns all sit **down** on the crown (slightly squashed by it), not on top like lids.
- Brims tilt with pitch and turn with yaw. A cap's peak points where his face points.

## Item 3: Things that go round him go all the way round
- **Eye patch:** the strap runs from the patch round the side of the head (along the ellipsoid) and disappears behind him. Same for goggles, visors and headbands.
- **Glasses / shades:** the lenses sit on the eyes (as 3L made them). The arms run back to the ears and disappear behind him. The far arm hides when he turns.
- **Headphones:** the band touches the top of the head (seated on the ellipsoid, no gap). The cups sit on the ears and turn with him. The near cup is bigger and in front, the far cup is partly or fully hidden.
- **Neck items:** a ring on the neck line, front over him, back behind. **Ties hang down from the knot**, falling past the bottom edge, never up over his face. Bow ties and pendants sit on the neck line below the eyes. Scarves wrap with thickness: a front fold, plus ends that hang.

## Item 4: They move like real clothing
- **Secondary motion:** use a damped spring (per item, cheap, only while visible) so hats lag a little and settle after hops, nods and turns. Dangling parts (tie, scarf ends, pendant, acorn stalk, bunny ears, pom-poms) swing like pendulums with damping.
- **Squash and stretch:** worn items squash and stretch with his body (same scale as the body) so they never float off during a bounce.
- **Rest:** at rest, everything settles to exactly still. No idle wobble, no CPU when nothing is moving. Skip the springs at the collapsed island size.

## Item 5: Checks the owner can see
- Extend the CI contact sheet: every item on each look at yaw -0.6 / 0 / +0.6 and pitch -0.2 / +0.2, at the open island size. Add a short **motion strip** (or animated GIF/WebP) per item category, showing a hop, a nod and a turn, so the owner can judge the movement.
- Tests:
  - no item covers either eye, except lenses and the patch;
  - no gap between a hat and the crown;
  - no neck item above the neck line;
  - every wrapping part has its back piece hidden by the body.
- Update the item-fit check from 3L to the new anchors. Keep the "worst 10" summary.
- Don't claim it looks right. Write "contact sheet and motion strips ready for the owner" in `STATUS.md`. One `[build]` commit at the end.

## Item 6: Polish after the first 3M build (added 4 Nov 21:10, from build 222's `items-sheet-night.png` and `items-motion-strips.png`)
3M is a clear step up: ties now hang below him, straps wrap, and the headphone cups sit on his sides. What's still wrong on the sheet:
1. **Pale ring round his head** when seen from above (pitch -0.2): the back of a hat or crown's rim is drawn as a light grey ellipse, so it reads as a see-through halo, the exact thing the owner rejected. Draw the inside/back of the rim in the **item's own colour, darker** (shaded), or hide it where the head covers it. No grey or transparent fill anywhere.
2. **Grey seam across his forehead** under hats, crowns and glasses at pitch +0.2: a thin grey line shows where the back piece meets the outline. Remove it: the back piece must be fully hidden by the body (clip it to outside the outline), with no stroke left on the face.
3. **Glasses arms curve along his lower face** (shades, heart glasses, round specs, visor) when seen from above: the far arm draws as an arc across the cheek and looks like a mouth. Arms go straight back to the ears at **eye height** and hide behind him. They never cross the lower face.
4. **The beret floats** above his head at pitch +0.2 with a gap. Seat it on the crown like the other hats.
5. **Headphones sink 9 px** at yaw ±1 (`items-problems.txt` warnings). Keep the band on the head at full turn.
6. Re-run the sheets and motion strips. Then write in `STATUS.md` which of 1 to 5 are fixed, and one `[build]` at the end.
