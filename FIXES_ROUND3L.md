# Fixes round 3L: the wardrobe sits on him properly (4 Nov)

**Why:** the owner tested the 3H/3K build (contact sheets from round 3K item 4). Items still look stuck on, like stickers on a box: hats sit flat on top with no curve or shadow; when he looks left or right, items don't follow (glasses slide out past the side of the head, the bandana stays flat); collars and scarves don't bend with the bottom edge; the acorn cap is narrower than the head. **No real 3D** (it would cost graphics memory and clash with the hand-drawn look). This round fakes the depth in 2D, cheaply. Memory budget (owner): under 150 MB idle, never over 300 MB peak, near-0 CPU idle; no new image files, everything drawn in canvas. Do not touch the wisps, growth art, eyes or glow.

One item per commit: "Fixes round 3L, item N: ...", tick it in `WORK_QUEUE.md` in the same commit, push, keep the build green. Files: `windows/src/mochi/outfits.ts` (`placeItem`, `narrowing`, `headAnchors`, `drawItem`, `RIMMED`/`COLLARED`/`ARMED`) and the item art.

## Item 1: Items follow his turn and never leave the head
- `placeItem` already uses `H.yaw`. Make it follow properly: head items (hats) shift sideways by `sin(yaw) * rx * 0.3`, **tilt** a few degrees with the turn (`rotate(sin(yaw) * 0.12)`), and squash horizontally with `cos(yaw)` (keep the existing `narrowing`).
- Face items (glasses, shades, heart glasses): the line `full = span / max(0.05, cos(yaw))` can blow up when he turns. **Clamp** so the item's outer edge (arms included) stays inside the body outline at every yaw from -0.7 to 0.7 rad; arms fade out on the far side as he turns.
- Collars, scarves and bandanas shift and narrow with the turn (they wrap round the neck), so no flat sticker.

## Item 2: Contact shadows
- Where an item touches the body (hat brim, glasses frame, collar edge) draw a soft shadow **on the body only**: a blurred dark ellipse or arc, alpha 0.18 to 0.25, clipped to the body path, offset down and away from the light. Use the existing `withLayer`/clip approach; cache nothing new per frame; skip it at mini or compact sizes.

## Item 3: Curve with the pillow outline
- Hat brims are arcs that follow the crown dip from `headAnchors` (not straight lines). Collars, scarves and bandanas follow the bottom curve of the pillow (`COLLAR_V`), bending a little more when he turns. Glasses' bridge and arms follow the face curve (small arc, not a flat bar).

## Item 4: Sizing from the head
- Size every item from `headAnchors` (rim width, eye line), not fixed numbers. Hats at least 0.92 of the head width at the rim; the acorn cap must not be narrower than the head. Check all items at stages 1 to 6 (his size changes).

## Item 5: Checks
- Extend the CI contact sheet (round 3K item 4): every item on each look, at yaw -0.6, 0, +0.6, plus the eyes-looking-left/right cases. Add a quick test that no face item extends past the body width at any tested yaw.
- Do not claim it looks right: leave the sheet for the owner and write "contact sheet ready for the owner's eyes" in `STATUS.md`. One `[build]` commit at the end.
