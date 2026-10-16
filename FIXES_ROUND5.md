# Fixes round 5: Groovy's own wardrobe, stage 6 and seasonal items

**Do this after round 4.** It changes `windows/src/mochi/` only (plus settings parsing in Rust where noted). Commit each item separately, with "Fixes round 5, item N: …" as the subject, like rounds 3 and 4.

The designs are in `docs/design/wardrobe-v2/`. Open the `.dc.html` files in a browser. Every SVG uses Groovy's current body: `rect x=14 y=30 w=72 h=62 rx=28` in a `0 -14 100 114` viewBox, with almond eyes at (38,62) and (62,62). Map them onto the canvas body the same way `outfits.ts` already maps hats and glasses.

- `Main.dc.html`: growth stages 0–5 for Night → Grove, Ember → Blaze and Frost → Tide, plus the stage meter.
- `Stage6.dc.html`: stage 6 (six months).
- `Wardrobe.dc.html`: Wardrobe v2, 28 items in Head, Face and Neck.
- `Seasonal.dc.html`: 16 seasonal items (Head and Neck).

## Why

The owner wants the wardrobe to be **entirely Groovy's own**. Coucou's `LICENSE-ASSETS.md` reserves the Mochi character art, and today's `outfits.ts` and `wardrobe.ts` are ports of Mochi's outfits (witch hat, santa hat, bunny ears, party hat, crown, beanie, bow…). Replace them all. Keep the body and engine exactly as they are; the owner chose to keep them.

## Item 1: Growth art from the canvas

Round 3 item 6 drew its own stage art. Replace it with the designs in `Main.dc.html`, so each stage matches the board.

- **Grove:** sprout, then two leaves, a sapling, branches, and finally a bonsai.
- **Blaze:** spark, then flicker, flame, wrapping flame, and a full aura.
- **Tide:** droplets, then the water fills ¼, ½ and ¾ (with seaweed), and finally a full sea with fish and a wave crest. The water is clipped to the body shape.

Keep everything that works today: the clock, the `reached`, `shown` and `seen` fields, "Show stage", the intro greeting, and `merge_growth`.

Animate cheaply. Leaves sway, the flame flickers and the water bobs, all with one shared phase driven by the existing engine tick. Animations pause when the island is hidden, as round 3 item 9 requires. Don't add new timers or new windows.

## Item 2: Stage 6 at six months

`STAGE_AFTER` counts *active* seconds. Six months of active time would take years for most people, so stage 6 uses the **calendar** instead:

- Add `hatchedAt` (unix seconds) to `Growth`. Set it when the Seed intro first hatches him.
  - **Migration:** if it's missing, use `now − activeSeconds`.
  - `merge_growth` keeps the **earliest** `hatchedAt`.
- Stage 6 is reached when `reached ≥ 5` **and** `now − hatchedAt ≥ 182 days`.
- Set `MAX_STAGE = 6`. Update `stageOf`, the meter, `progressLine` ("Stage 5 of 6 · next in about 4 months"), the "Show stage" chips and the Rust validation, which currently caps the stage at 5.

The art for each look is in `Stage6.dc.html`:

- **Grove → Sakura bonsai.** The bonsai blossoms pink (#E26A9B, #FF9FC6, #FFC1DA). His eyes turn #FF6FA8 and a few petals drift around him (at most 5, slow).
- **Blaze → Golden flame.** The flame turns gold (#E0A100 outer, #FFD84A inner) with a white-hot tip, gold eyes and 3 floating sparkles.
- **Tide → Diamond sea.** His eyes become **diamonds**: the same almond shape and tilt, faceted, #CFF3FF with a #7FD4FF edge. Diamond flakes glint around him.
  - All sea life inside is **gold** (#E0B84A).
  - Usually a small golden fish swims through. Sometimes it's a golden **shark** and sometimes a golden **submarine**. Pick one at random on each pass: fish 70%, shark 20%, submarine 10%.
  - Only **one creature at a time**, one pass every 2–4 minutes, so it stays a treat and costs nothing while idle.

Reaching stage 6 plays a special intro moment: the Seed intro's sparkle, then a one-line toast: "Six months together. Thank you."

## Item 3: Wardrobe v2

Replace `wardrobe.ts` and `outfits.ts` with the 28 items in `Wardrobe.dc.html`. Draw them all in code (Canvas2D paths), following the board's shapes and colours. Don't use image files.

- **Head (12):** Leaf crown, Acorn cap, Mushroom cap, Flower crown (Stage 2), Bucket hat, Backwards cap, Headphones, Pilot goggles, Beret & pin, Mini top hat (Stage 3), Cloud buddy (Stage 4), Antenna.
- **Face (8):** Round specs, Shades, Heart glasses, Glow visor, Monocle, Freckles & blush, Star stickers, Pirate patch.
- **Neck (8):** Bandana, Bow tie, Leaf scarf, Seed pendant (Stage 1), Neck phones, Gold chain (Stage 5), Flower lei, Necktie.

How it works:

- The three slots (Head, Face, Neck) are independent, and each has "None". There's a "Remove all" button.
- A locked item shows dashed with "Stage N". It unlocks when `reached ≥ N`. It never locks again, even when the user chooses to show an earlier stage.
- Add a "Stage 6" item: a **Diamond crown** for the head, gold with a single #CFF3FF gem, drawn in the same style. It's the reward for six months.
- **Migration:** saved outfits that no longer exist become "None". Map any saved Mochi outfit ids to the nearest new one where obvious (beanie → Bucket hat, bow → Bow tie), and to "None" otherwise.
- **Delete every Mochi outfit:** witch hat, santa hat, bunny ears, party hat, crown, beanie, bow and so on, along with their `N_()` strings. Grep `outfits.ts`, `wardrobe.ts`, `i18n` and the settings UI so no reserved design is left.

## Item 4: Seasonal collection

Add the 16 items in `Seasonal.dc.html` as a fourth wardrobe tab, "Seasonal". Each season has 2 Head items and 2 Neck items:

| Season | Window (local date) | Head | Neck |
|---|---|---|---|
| Halloween | 1 Oct – 7 Nov | Pumpkin cap, Bat-wing band | Candy-corn scarf, Little ghost pal |
| Winter & Christmas | 1 Dec – 6 Jan | Bobble beanie, Twig antlers | Knit stripe scarf, Jingle-bell collar |
| Easter | Easter Sunday ± 7 days (compute with the standard Gregorian Easter formula) | Bunny-ear band, Eggshell hat | Pastel ribbon bow, Daisy chain |
| Summer | 21 Jun – 31 Aug | Straw sun hat, Watermelon cap | Shell necklace, Pool ring |

How the seasons work:

- **Earning.** When Groovy runs during a season, that season's 4 items are earned and saved (`seasonalEarned: string[]` in settings, merged as a union in Rust). They then **stay forever** and can be worn any time of year.
- **Not earned yet.** An item that hasn't been earned shows locked, with "Comes back in October" (or the season's month).
- **Auto dress.** If the wardrobe's Auto option is on, he wears one of the season's head items on the first day of the season, until the user changes it. This replaces today's October-only witch-hat behaviour, which also had a bug that disabled the other hat buttons; make sure every button stays clickable.
- **Pool ring** sits around his middle (draw it over the lower body). Every other item uses its normal slot.
- **Today is in the Halloween window,** so the Halloween items should earn on the first launch after this ships.

## Item 5: Tests and checks

- **Unit tests:**
  - `stageFor` and stage 6 at the 182-day boundary;
  - `hatchedAt` migration and merge;
  - season windows, including Easter for 2026, 2027 and 2028 (5 Apr, 28 Mar, 16 Apr);
  - seasonal earning and the union merge;
  - outfit-id migration.
- **Rust tests** for the new settings fields (they must stay green on Windows CI).
- **Memory:** the smoke test from round 3 item 9 must still be under 300 MB idle. The new art is drawn on the existing canvas, with no new windows or images.
- **Final check:** grep for `witch`, `santa`, `partyHat`, `bunnyEars`, `Mochi outfit` and confirm nothing is left.
