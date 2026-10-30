# Fixes round 3H: the wardrobe must really fit, and one clear growth screen (30 Oct)

**Do this first, before 3D item 11 and the checks.** Commit each item separately with "Fixes round 3H, item N: …", tick it in `WORK_QUEUE.md` in the same commit, push, keep the build green. Keep the speed fixes (cached previews, one live engine) and the memory budgets.

## Item 1: Items sit on him the way Coucou's do (3E item 10 wasn't enough)

The owner's test of the 3E build: the **acorn cap** on the island and in Settings doesn't wrap his head, the **glasses** don't sit on his eyes, and it all still looks like **a flat picture pasted on top**. In Coucou the same items look 3D: the cap goes round the head (you see the back of the brim and the front), the glasses sit on the eyes, the neck items go round the neck. 3E item 10 only fixed the sideways shift; the items are still single flat pictures. Make them genuinely fit:

- **Anchors from the live head, not board coordinates.** The engine publishes every frame: the head's rim (the top of his body outline), the two eye centres and their spacing (exactly what `drawEyes` uses, including the yaw shift, pitch and `physDx`/`physDy`), and the neck line (the bottom of the body, with the squash). Each item is authored against named anchors (`rimLeft`, `rimRight`, `rimTop`, `eyeL`, `eyeR`, `neck`) and placed by mapping those points onto the live ones.
- **Items are a few parts at different depths** (back, body, front), each projected with its own depth as the head turns and nods, so there is real wrap and parallax:
  - **Caps and hats** (acorn cap, beanie, witch hat, crowns…): a **back part** (the far half of the brim and the inside) drawn **before** the body, and a **front part** (the crown and the near half of the brim) drawn **after** it. The rim is an **ellipse on his head** whose width follows his turn and whose height follows his pitch, so the cap visibly goes round him and can't float above or sink into him.
  - **Glasses and face items:** lens centres are **bound to the eye centres**, lens size to the eye spacing, so they sit on the eyes at every angle. Lenses are see-through (the eyes show through). The **arms** run back along the sides to the head and are hidden behind the body on the near side and visible on the far side. When he turns, the far lens narrows with his turn and the near one widens, and the bridge moves with the nose line.
  - **Neck items** (scarf, chain, bow tie…): an ellipse round the base of the body; the **back half drawn before the body**, the front half after, following his squash and tilt.
- **Parts that sit inside his body** (lens tint, a collar's inner side) are clipped to the body; everything else is drawn free, so nothing is cut off.
- **Pitch and roll** (nodding, tilting, hopping and the landing squash) move every part with the head, with no lag against it.
- **All items:** the 28 Wardrobe v2 items, the Diamond crown, the 16 seasonal items. Do the **acorn cap and the glasses first**, as the pattern, then the rest.
- **Reference:** upstream Coucou's items are the reference for how an item sits and wraps (anchors, layering, what's behind and in front). Don't copy its art.
- **Tests:**
  - A contact sheet in the CI artifacts: every item × each look × yaw -1, -0.5, 0, 0.5, 1 × pitch -0.3, 0, 0.3, on the island and in the Settings preview.
  - Measured checks that fail the build: a hat's rim stays within 2 % of the head outline; a lens centre is within 3 % of its eye centre; a neck item's ellipse stays within 3 % of the body's base; no part of any item is cut off by a clip.
- **Owner's screenshots** of the acorn cap and the glasses (island and Settings) go in `docs/design/wardrobe-v2/fit-issues/` if they're attached; use them as the before/after.

## Item 2: One look switch, a clean growth and preview screen

The Settings → Groovy page has **two** Night / Ember / Frost switches: one at the top that changes his look, and a second in the preview that only previews. Together with the progression stages this is confusing, and the preview is laid out oddly and looks glitchy.

- **One switch only.** The look tabs at the top (Night, Ember, Frost) choose **which look the whole page is about**: its timeline, its stage previews, its wardrobe. Each tab shows its own state ("Stage 3", "Not started").
- **"Wearing" is separate from "viewing".** The look he's wearing carries a "Wearing" badge. Viewing another look shows that look's page with a **"Wear this look"** button; nothing changes on the island until the owner presses it. Only the look he's wearing grows (3E item 12).
- **Delete the second switch** in the preview strip. The strip always shows the viewed look's six stages: reached ones in colour with a tick, the rest dimmed with "unlocks in about …"; tapping a locked stage previews it for 5 seconds ("Preview only"), as in 3E item 11.
- **Layout:** the big preview on the left; to its right the timeline; below, the stage strip in one tidy row (six equal cells, no wrapping or overlap at the smallest Settings width); wardrobe items below that. No jumpy resizing when a preview starts or ends (reserve the space), no flicker on tab changes (reuse the cached canvases), and the whole page animates with the shared motion module.
- **Test:** screenshots of the page for each look at the minimum and the normal window width, dark and light; a frame-time test while previewing; no layout shift when a preview starts.

## Item 3: The growth clock must be clear and trustworthy

The owner has had Groovy for a while and the progress hasn't moved: it still said about 10 hours left to Stage 1. **Cause (by design today):** growth counts **running time** (`startGrowthClock` adds the minutes Groovy's island is actually running and the PC is awake), and Stage 1 needs **24 hours of that**, not 24 calendar hours. A few evenings of use don't add up to a day.

- **Show it plainly:** on the Groovy page, "Time with Night: 14 h of 24 h for Stage 1 · counts only while Groovy is running", with a small "today: 2 h 10 m" line. The owner must be able to see that it's counting.
- **Make sure it really counts:** while Groovy is running and the PC is awake, the clock must keep ticking in Standby, with the island hidden, and when full-screen apps are in front (it must not depend on the window being visible). Add a test for this, and write each tick's time to the log in the CI build.
- **Owner's decision (ask in the UI and in `STATUS.md`, don't guess):** keep it as running time, or count **calendar days the look was worn** instead (a day counts if that look was worn for at least 30 minutes), which would be gentler. Default: keep running time until the owner chooses. Build the clock so the rule is a single constant that can be changed.
- **Test:** a look worn 2 h on three evenings shows 6 h counted; the other two looks stay at 0 and "Not started".

## Item 4: Checks

- Contact sheet of every item at every angle (item 1) reviewed and attached.
- Growth page screenshots (item 2) for all three looks, dark and light.
- Growth clock tests (item 3) pass.
- Memory budgets hold (300 MB idle, 400 MB peak) and the frame-time test passes with the new item rendering.
