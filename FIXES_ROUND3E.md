# Fixes round 3E: clean-up after the owner tested the 3D build (24 Oct)

**Do this next**, right after the item in progress, then round 5. Commit each item separately with "Fixes round 3E, item N: …" as the subject, tick it in `WORK_QUEUE.md` in the same commit, push, and keep the build green. Screenshots of the problems were shared in chat; they're described below.

## Item 1: The island tiles look right at their normal size

On the 4 × 2 tile grid, the tile text is too big and gets cut off ("Remin…", "ArtCraft Launc…", "Visual Studio…", "Proton VPN" wrapping).

- **Tile layout:** icon 22 px at the top left. Title **12 px semibold**, one line, ending in "…" only if it truly can't fit. Subtitle **10.5 px**, one line, dimmer.
  - Shorten the default names: "Reminders" can show "Reminders" if the tile is wide enough; otherwise use a short label the user can edit.
  - App names fit on one line ("ArtCraft", "VS Code", "Proton VPN").
- **Spotify in its 1-tile size:** the 40/52 px controls belong only in the **expanded 2 × 2 player** (round 3D item 8). In the 1-tile size, show the album art (or the Spotify icon), the song title, and **one** play/pause button (about 26 px). Previous and next appear only when expanded.
- **Rebalance the island:** make the Groovy area on the left about 15% narrower (his size stays the same), and give that space to the tile grid, so tiles are wider and labels fit.
- **Page switcher:** the two dots under the tiles are far too small to click.
  - Replace them with a proper switcher: dots 8 px with a **32 × 32 px hit area each**, or a pill "1 / 2" with **‹ ›** buttons of at least 28 px.
  - The mouse wheel and a horizontal swipe over the tiles also change pages.
  - The page change slides smoothly.
- **Test:** screenshot the island in CI with long app names and both tile pages, and fail if any tile text overflows its tile, or if any control's hit area is under 28 px.

## Item 2: Real app icons, never letters

App tiles show a **letter** (ArtCraft "A", Proton VPN "P", VS Code "V") instead of the app's own icon.

- **For apps:** extract the real icon from the app's shortcut or exe. Use `IShellItemImageFactory` / `SHGetImageList(SHIL_JUMBO)`, or `ExtractIconEx` from the target. Store-app icons come from the package's logo assets (resolve the AUMID through `IShellItem`).
  - Render at 2× the shown size for sharpness.
  - Cache as a small PNG per app in Groovy's data folder, refreshed when the exe changes.
- **For websites:** the site's favicon (fetched once and cached).
- **For folders:** the folder icon.
- **Services** (GitHub, Vercel, Neon, Resend, Notion, Cal.com, Stripe, n8n, Spotify, Mail, Claude Code): use each service's **official logo mark**, as a consistent monochrome or brand-colour glyph set, sized the same.
- Letters are only a last-resort fallback when no icon can be found, and are logged.
- **Memory:** icons load lazily with the tiles on screen; the cache in memory holds at most what's visible.

## Item 3: One clean tiles list in Settings (merge "Tiles" and "My island")

Settings → Island → Layout & tiles currently has **two separate sections**:
- a "Tiles · tap to show or hide" grid of services (dashed boxes, with mini coloured Groovy icons on only some of them, such as Spotify green and Neon orange);
- a separate "My island" list (presets, then rows with Compact, ↑ ↓, Remove).

The owner wants **one** place.

- **One "Tiles" section:**
  - A grid of **every** tile, services and the user's own apps, websites and folders together, each with its **real icon** (item 2).
  - Tap a tile to **show or hide** it on the island. Shown tiles are solid, hidden ones dimmed.
  - **Drag** to reorder, with arrow keys for keyboard users.
  - Each tile has a small "…" menu with Compact/Wide, Rename, and Remove (Remove only for user-added items).
  - The order here **is** the island's order, and a live mini preview of the island updates as you change it.
- **"+ Add tile"** opens a picker: an installed app (searchable list with icons), a website, a folder, or a service. One click adds it.
- **Presets** (Developer, Everyday, Custom) become a small segmented control above the grid. Choosing one sets the tiles; any change switches to Custom.
- **Remove the mini Groovy agent icons** from service tiles. Every tile uses its real logo or app icon, consistently.
- Glass, Opens and Closes stay in "Look and feel" below. Accounts stay in Accounts.

## Item 4: Growth must start where he really is (bug)

Groovy shows **"Stage 5 of 5 · fully grown"** although the owner has had him for under a month.

- **The cause:** `migrateGrownLook` (`windows/src/mochi/growth.ts:105`) sets `reached: MAX_STAGE` for anyone who had the old Grove, Blaze or Tide look. Because growth "never goes back" (`mergeGrowth`), he's stuck at stage 5.
- **The fix:**
  - Make it **one-time**: recompute `reached` from the real record: `stageFor(activeSeconds)`, plus the six-month rule with `hatchedAt` for stage 6 (round 5 item 2).
  - This one correction is allowed to lower `reached`, and it's recorded as `growthCorrected: true`, so it never runs twice.
  - Remove the "fully grown" grant from `migrateGrownLook`.
  - Rust's `merge_growth` must accept this corrected record (compare a correction version number, not just the stage).
- **The stage timeline UI:** today it shows six stacked full-width green bars. Use the approved design (`docs/design/settings-v2/Groovy.dc.html`): **one horizontal row of 7 steps** (Seed, 1 d, 3 d, 1 wk, 2 wk, 1 mo, 6 mo), filled up to the real stage, with "Stage N of 6 · next in about …".
- **Tests:** a profile migrated from Grove with 2 days active shows stage 1, not 5; the correction runs once; a real 35-day profile shows stage 5.

## Item 5: Stage 6 and the final forms (round 5 item 2, with the owner's latest wishes)

Round 5 item 2 hasn't been done yet. Do it now, with these specifics. The designs are in `docs/design/wardrobe-v2/Stage6.dc.html`; where these notes differ, the notes win.

- **Night → Grove, stage 6:** a **pink Sakura bonsai** on his head, with drifting petals. His **eyes become green emeralds**: the same almond shape and tilt, faceted, deep green (#0FA958 to #7CF2B0), with small sparkles.
- **Ember → Blaze, stage 6:** a full **golden flame** (#E0A100 / #FFD84A, with a white-hot core) and gold eyes.
- **Frost → Tide, stage 6:**
  - the **Diamond sea**: **diamond eyes** and diamond flakes;
  - **golden** sea life swims through him, one at a time: a small fish most of the time, sometimes a **golden shark**, sometimes a **golden submarine**.

## Item 6: Ember's fire and Frost's water must look as clean as Night

The owner finds Ember's flame and Frost's water **too cartoonish** compared with Night's clean look.

- **Bring all three to the same finish:** soft layered gradients, subtle inner glow, smooth edges and restrained colour. No thick outlines, flat cartoon blobs or hard-edged shapes.
- **Ember (flame):**
  - several translucent flame layers (deep orange to amber to a pale core) with soft edges;
  - gentle, natural flicker: a slow noise-driven sway, not a bouncing loop;
  - a faint heat glow on the body;
  - a few tiny embers rising.
- **Frost (water):**
  - water inside the body with a **soft refraction and caustic shimmer**;
  - a gentle surface wave;
  - small bubbles;
  - sea life that **swims smoothly** across (eased paths, a slight tail motion) rather than sliding.
  - The water level follows the stage.
- **Performance:** draw on the existing canvas at the shared frame tick, cache static layers, and use cheap noise (precomputed). It still passes the frame-time test and the 300/400 MB budgets, and animates only while visible.
- Use the canvas boards (`docs/design/wardrobe-v2/Main.dc.html`, `Stage6.dc.html`) for shapes and stages. These notes set the quality bar.

## Item 7: The full wardrobe, with stage unlocks and seasonal items (round 5 items 3 and 4)

The wardrobe still shows the **old Coucou/Mochi items** (Party hat, Beanie, Crown, Witch hat, Santa hat, Bunny ears, Bow), with **no stage unlocks** and **no Seasonal tab**. Round 5 items 3 and 4 haven't been done yet. Do them now, exactly as `FIXES_ROUND5.md` describes:

- **Head, Face and Neck tabs** with the **28 Wardrobe v2 items**, plus the **Diamond crown** at stage 6. Use the `docs/design/wardrobe-v2/Wardrobe.dc.html` designs, drawn in code.
- **Stage-locked items:**
  - Seed pendant: stage 1.
  - Flower crown: stage 2.
  - Mini top hat: stage 3.
  - Cloud buddy: stage 4.
  - Gold chain: stage 5.
  - Diamond crown: stage 6.
  - Locked items show **dashed with "Stage N"** and unlock automatically.
- **A fourth tab, "Seasonal":** the 16 seasonal items (Halloween, Winter & Christmas, Easter, Summer). They're earned during their season and kept forever. Items not yet earned show "Comes back in …". It's October, so the Halloween items should already be earned.
- **Delete every Mochi/Coucou outfit** and its strings, and migrate saved outfits.
- Keep search and Favourites (in the screenshot), and keep the speed fixes from round 3B item 4.

## Item 8: Checks

- Every Settings page and the island are screenshotted in CI (dark and light).
- The tile text-overflow and hit-area tests pass.
- Growth is correct for fresh, migrated and long-running profiles.
- Stage 6 renders for all three looks.
- No Mochi item is left (grep).
- The memory budgets hold (300 MB Standby, 400 MB peak), and the frame-time test passes.

## Item 9: Frost's water leaves blank spots when he faces sideways (owner's screenshot, 25 Oct)

When Frost turns his head to the side, part of his body shows no water (a pale blank strip on one side), so he looks half empty.

- **Cause** (`windows/src/mochi/finish.ts`, `surface()` and `drawWater()`): the water is drawn only between board x = 8 and 92 (his body's width). `skins.ts` `drawLayer` shifts the whole board sideways by `sin(yaw) * rx * 0.8` when he turns, so on the side he turns away from, the body's edge lies outside 8–92 and nothing is painted there.
- **Fix:** draw the water far wider than his body, since the clip to his body already decides what shows. In `surface()`, run the wave from `SEA_L = -70` to `SEA_R = 170` and close the path down to y 140 (not 98). In `drawWater()`, run the caustic bands from x -50 to 150, not 10 to 90. Bubbles may stay where they are.
- **Check the same thing everywhere that is shifted with the board:** Ember's heat glow (`drawHeat`), the golden swimmer (`drawStage6Body`, `skins.ts` line 243, shifted by 0.8) and any full-width layer in `stage6.ts`. Each must cover the body at the largest yaw, on both sides.
- **Test:** render Frost at yaw -1, 0 and +1 (all stages 1-6) in CI and fail if any pixel inside the body outline is left unpainted by the water layer. Screenshot the three in the CI artifacts.

## Item 10: Wardrobe items must fit and track his head (owner, 25 Oct)

The head, face (eyes) and neck items don't fit nicely. When he faces sideways they show blank spots and they don't sit the way they do in Coucou.

- **Likely causes** (`windows/src/mochi/outfits.ts`):
  - `drawItem` slides every item by `sin(yaw) * rx * 0.35`, but his eyes are shifted by `sin(yaw) * rx * 0.8` (`skins.ts` and `drawEyes`). So **face items drift off the eyes** when he turns. Face items must use the same 0.8 shift as the eyes; hats and neck items get their own shift that keeps them on the part of the head they belong to.
  - The art is flat and is not narrowed as the head turns, so a hat or glasses stays full width on a body that looks narrower. Narrow each item horizontally by the head's turn (about `cos(yaw)` on the way round, never below about 0.6 so nothing vanishes) around the item's own anchor, the way Coucou's items follow its head.
  - Items drawn "clipped" to the body (`drawShapes(…, "clipped")`) lose the part that falls outside the body when it slides. Only parts meant to sit inside the body (the lens fill, say) are clipped; frames, hats and neck items are drawn free.
  - Pitch, roll and the head's physics wobble (`physDx`, `physDy`) must move items with the head too.
- **Reference:** upstream Coucou's items are the reference for **where and how an item sits** on the head (anchor points, how it follows the head, how it layers behind and in front). Don't copy any Coucou art; only the placement behaviour.
- **Fix every one of the 28 Wardrobe v2 items, the Diamond crown and the 16 seasonal items:** each gets an anchor (head top, eye line, neck) and a behind/body/front split, so nothing is cut off or floats at any angle.
- **Test:** render every item on all three looks and all six stages at yaw -1, -0.5, 0, 0.5 and 1, and at pitch ±0.3. Fail if any item's art sits outside its anchor zone by more than 6% of his width, or leaves a gap against the body. Put the contact sheet in the CI artifacts so the owner can see it.

## Item 11: Preview the growth looks, with stages that unlock at their minimum (owner, 25 Oct)

He wants to **preview every stage's look**, with each stage unlocking once he reaches its minimum requirement. **This is not a stage picker:** the real stage is still earned by time, and previewing never changes it (round 3B item 5 stays).

- **Where:** Settings → Groovy, under the 7-step timeline: a strip of the six stage looks (1 to 6) for his current look (Night, Ember or Frost), each drawn live at the shared frame tick with the same renderer as the island.
- **Unlocked stages** (reached): shown in full colour with a tick; tapping plays that stage's look in the big preview.
- **Locked stages:** shown dimmed with a dashed border and "Stage N · unlocks in about …" (from `STAGE_AFTER`, 1 d, 3 d, 1 wk, 2 wk, 1 mo, 6 mo). Tapping one **previews it for 5 seconds** in the big preview with a "Preview only" label, then goes back to his real look. It can't be kept, saved or shown on the island.
- **Same for wardrobe items that wait for a stage** (Seed pendant, Flower crown, Mini top hat, Cloud buddy, Gold chain, Diamond crown): tapping a locked one tries it on for 5 seconds with "Preview only", then it comes off. When the stage is reached it unlocks for good, as now.
- **The three looks:** a small Night / Ember / Frost switch above the strip previews the other looks at any stage too (preview only, 5 seconds); the chosen look is changed only where it is chosen now.
- **Memory:** the strip draws only while the page is visible, one small canvas at a time (lazy), and stops when Settings closes. It must not raise idle memory (round 3D items 7 and 9).
- **Test:** a profile at stage 2 shows stages 1 and 2 unlocked and 3 to 6 locked; previewing stage 6 doesn't change `reached`; the preview reverts after 5 seconds; nothing is saved.
