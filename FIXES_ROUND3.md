# Groovy round 3: Groovy's own island, one island only, wardrobe, growing looks and the Seed intro

Owner's decisions from 16 Oct 2026, after reviewing design mockups. The mockups are in `docs/design/island-round3/`:
- `MixF.dc.html`: the normal state;
- `MixE.dc.html`: while music plays;
- `Voice.dc.html`: dictation and call states.

These are HTML mockups. Read the markup for exact sizes, colours, spacing and copy. They're drawn at 1.5× the island's real size.

Same rules as rounds 1 and 2:
- work top to bottom;
- build, test and commit after each item;
- update STATUS.md;
- test on a low-power 8 GB Windows PC;
- keep `windows-build.yml` green.

## 1. One island only: remove the bottom pill
- The see-through capsule at the bottom of the screen goes completely: Groovy's face, the mic, Record a call and Open Groovy (from Coucou's Flow pill). The owner reports its buttons don't work anyway.
- **Remove:**
  - the `pill` window: `windows/pill.html`, `windows/src/pill/`, `src-tauri/src/flow/pill.rs`;
  - its entry in `vite.config.ts`;
  - `"pill"` in `capabilities/default.json`;
  - the `pill_*` commands;
  - the "pill position" settings.
- **Move everything it did into the top island:**
  - `flow::pill::announce(...)` messages (e.g. `lib.rs:913`, `1302`, `1314`) become island notifications, with the same icon, colour, text, optional action button and timeout;
  - dictation and recording states use section 3 below;
  - "paste last dictation again" becomes an island action and keeps its shortcut.
- Check that nothing else opens a second floating window.
- Dictation (hold right Ctrl), the Notetaker and calls must keep working with no pill window.

## 2. The new island layout (glass)
The island's **frame stays the same**: hanging from the top, the Home / Chat / + tabs on the left, gear and sound on the right. Only the inside changes, and the whole island becomes **glass**.

### 2a. Normal state (`MixF.dc.html`)
- **Left card, "Up next":**
  - Groovy (his current look and stage, section 6), with the single most important thing right now next to him: a small label ("UP NEXT", or "UP NEXT · IN 15 MIN"), a title, and a detail line.
  - Below that, three buttons: **Dictate**, **Call** and **Record**.
  - Priority order for "Up next":
    1. a meeting within 30 min;
    2. new mail (sender and subject, from round 1's Windows-notification mail);
    3. an agent that finished or needs approval;
    4. a reminder due;
    5. otherwise a greeting.
  - Clicking it opens the thing (the mail app, the meeting link, the agent session).
  - Claude Code's "hooks installed" status is **no longer** shown here. It moves to its tile and to Control Centre → Agents.
- **Right card, tile grid:**
  - **4 × 2 tiles** with live info, e.g. Spotify "Paused", Mail "2 new", Claude Code "Tests passed", GitHub "2 PRs", Vercel "Ready", Resend "14 today", n8n "3 runs".
  - The last cell is a dashed **+ Add tile**.
  - With more than 7 tiles, use pages (dots) or scrolling. Clicking a tile opens its detail or app.
- **The Spotify tile, when nothing is playing,** is a small square with Spotify's mark and three working buttons: **⏮ ▶ ⏭**.

### 2b. While music plays (`MixE.dc.html`)
- As soon as music is playing (from Spotify, the tile's ▶, or voice), the right card switches to:
  - a **wide player bar** on top: artwork, title, artist, progress bar, ⏮ ⏯ ⏭;
  - **4 tiles** under it, with page dots for the rest.
- The Spotify tile isn't repeated while the player is shown.
- On **pause or stop**, wait about 5 s and then shrink back to 2a, so a quick pause or skip doesn't make it flicker.
- Animate both ways in about 250 ms, using transform and opacity only.
- Clicking the player opens Spotify.
- This uses the existing Windows media-session code (`spotify.rs`, the GlobalSystemMediaTransportControls part). It should also work for any app playing media, labelled with that app's name.

### 2c. Glass, cheaply
- Use **Windows' own Acrylic effect** on the island window (Tauri 2 `windowEffects` / `set_effects` with `Effect::Acrylic` on Windows 11, `Effect::Blur` as the Windows 10 fallback), with a transparent webview background.
- **Don't** use CSS `backdrop-filter` across the whole window. Windows draws Acrylic itself, so it costs Groovy almost no CPU or RAM.
- Make the glass follow the island's rounded shape:
  - size the window to the island exactly;
  - use DWM rounded corners where Windows allows;
  - otherwise give the window a matching region, so there's no rectangular frosted box.
- Inner cards are translucent white (`rgba(255,255,255,0.10)`, border `rgba(255,255,255,0.20)`) with white text. Check the text contrast over bright wallpapers too: add a slight dark tint to the glass (about `rgba(18,20,24,0.42)`) so text stays readable.
- **Setting:** Control Centre → Appearance → Island style: **Glass** (default) / **Solid**. Solid is the black version.
- Also respect Windows' "Transparency effects" switch and battery saver: when either is off, use Solid automatically.
- **Measure and record in STATUS.md:** idle CPU and RAM of the island with glass vs solid, and while expanding and collapsing. Target: no measurable idle CPU, and under 5 MB extra RAM for glass.

## 3. Voice and recording states (`Voice.dc.html`)
- **Default:** while dictating, on a call, or recording, the island stays as it is and shows a **small badge**: a green dot, "Dictating 0:04" (red "Recording 12:40" for calls).
- The badge has a **button** that shrinks the island into a **live strip**: Groovy, nine bars that move with your voice, the timer, a "back to island" button, and Stop. Recording shows "Stop & write notes".
- **Setting:** Settings → Voice → "While listening, show": **Badge** (default) / **Live strip**.
- The Dictate, Call and Record buttons in the left card must actually start these. Test each on Windows.

## 4. Integrations: let the user turn on any of them
- **Bug:** `MAX_DECLARED = 4` (`src/core/pills.ts:143`). With GitHub, Vercel, n8n and Resend on, the switch handler in `src/settings/main.ts:847` just `return`s, so Spotify, Cal.com, Notion etc. can't be turned on and there's no message.
- **Fix:**
  - remove the 4-slot limit: the tile grid pages handle any number;
  - every integration can be switched on or off freely;
  - drag to reorder.
  - If an integration needs a key (Notion, Cal.com, Stripe…), switching it on opens its key field with a link to where to get the key. Don't refuse silently.
- The **+ Add tile** in the island opens this same list.
- Merge round 1's "My island" tiles and the integration pills into **one** tile system, so there's only one list to manage.

## 5. Wardrobe: no Outfit section, independent Head / Face / Neck
- **Remove the Outfit section entirely,** including "Auto" (the seasonal witch hat, Santa hat etc.) and the pumpkin costume.
- **Keep three sections:** **Head**, **Face** and **Neck**.
  - Each section has its own **None** button, which clears only that slot.
  - Choosing an item replaces what's in that slot, and the other slots stay as they are. Example: beanie + sunglasses, then removing just the sunglasses.
  - Keep one global **Remove all**.
- No item is ever greyed out.
- **Migration:** a saved outfit becomes a layer in its slot, and an Auto or pumpkin selection becomes nothing. Keep the favourites and presets that still make sense.
- Update `src/settings/wardrobe.ts`, `src/mochi/wardrobe.ts`, `appearance.ts` and the tests.

## 6. Looks that grow: Night → Grove, Ember → Blaze, Frost → Tide
Today there are six separate looks (`src/mochi/skins.ts`: night, ember, frost, grove, blaze, tide). In the code, "Amber" is `ember`.

**New model: three looks that grow** as Groovy lives on the PC:

| Look | Grows into | How it grows (stage 0 → 5) |
| --- | --- | --- |
| **Night** | **Grove** | nothing on his head → a tiny sprout → two small leaves → a sapling with a few leaves → leafy branches → a small **bonsai tree** crowning his head. Colours shift from Night's toward Grove's as he grows. |
| **Ember** | **Blaze** | nothing → a spark → a small flame on top → a bigger flame → flames starting to wrap around him → a full **fire aura** surrounding the whole character. Colours shift toward Blaze. |
| **Frost** | **Tide** | empty → a few droplets inside → water filling about 25% → 50% → 75% → **completely filled** with gently moving water. Colours shift toward Tide. |

- **Growth clock:**
  - It's based on time Groovy has actually run on the PC (active, not asleep), saved in settings.
  - Suggested stage thresholds: 1 day, 3 days, 1 week, 2 weeks and 1 month of use. Keep them in one constant so they're easy to tune.
  - Growth never goes backwards.
  - The stage is **shared** across the three looks: switching from Night to Frost keeps the same maturity.
- **Wardrobe UI:**
  - three look cards (Night, Ember, Frost), each showing its current stage;
  - a small progress line, e.g. "Stage 2 of 5 · next in about 3 days";
  - "Show stage" lets the user pick any **stage already reached**, for example to keep him small.
- **Drawing:**
  - Extend the existing nature touches (`nature: "leaf" | "flame" | "water"` in `skins.ts`, `drawSprout` etc.) into stage-aware drawings.
  - Keep it in code (canvas), not images.
  - Keep the animations cheap: fewer particles and slower updates when the Lightweight profile is on or the island is collapsed.
- **Migration:** users who had Grove, Blaze or Tide selected get the matching base look at full stage (5) unlocked.
- Remove `grove`, `blaze` and `tide` as separate selectable looks. Their colour styles stay as the stage-5 targets.
- Tests: the stage from runtime, stages that never go backwards, migration, and drawing each stage without errors.

## 7. New intro animation: "Seed" (replaces Coucou's drop-and-wave)
- Today's launch greeting (`src/mochi/greeting.ts`, a port of Coucou's `GreetingCanvasView.swift`) drops Groovy into the island, bounces, slides aside and waves. It also plays Coucou's `greeting` sound. Replace both.
- Mockup: `docs/design/island-round3/IntroSeed.dc.html`. Its CSS keyframes give the order and rough timing; a 6 s loop there is about 3 s in the app.
- **First launch after install (about 3 s):**
  1. A small **glowing seed** in the current look's colour (Night green, Ember orange, Frost blue) falls from the top edge into the island's spot. It bounces once and squashes slightly.
  2. It **cracks open** into two halves that fall away and fade, with a soft flash of light.
  3. **Groovy pops out** at **stage 0** with a small overshoot (scale about 1.15, then 1) and blinks.
  4. His stage touch appears: for stage 0, a single tiny sprout, spark or droplet.
  5. The **glass island grows around him** from a small point to full size (scale with a slight overshoot).
  6. Text fades in: "Hi, I'm Groovy." / "I'll grow the longer we work together."
  7. It settles into the normal island (F layout).
- **Every later launch (about 1 s):** the seed lands, a quick crack, and Groovy pops out **at his current stage**, showing his current growth.
  - If he reached a new stage since last time, add one beat: the new leaf, flame or water level grows in, with the line "I grew while you were away!".
  - Otherwise show "Up next" (e.g. "2 new emails").
- **Rules:**
  - Click to skip.
  - Never shown more than once per launch.
  - Respect Windows "Show animations" / reduced motion: just fade the island in.
  - Draw it on the existing canvas engine (`BotEngine`), and **don't** keep it in memory once it's finished.
  - Keep the frame rate cheap on the Lightweight profile.
- **Sound:** a new short sound of Groovy's own (a soft "pop" plus a rising chime, under 1 s), with volume and mute following the sound setting.
  - Don't use Coucou's `greeting.wav`.
  - **Also check every file in `windows/sounds/`:** `LICENSE-ASSETS.md` reserves Coucou's sounds, so replace any that came from Coucou with original or properly licensed ones before any public release, and list where each comes from in `windows/sounds/SOURCES.md`.
- Tests: the stage shown matches the saved stage, the first-launch vs returning choice, skipping, and reduced motion.

## Done means
- One island, glass by default, with a Solid option and a measured cost.
- The F layout normally, which turns into E while music plays and back about 5 s after it stops. The Spotify tile's ⏮ ▶ ⏭ work.
- Dictate, Call and Record work from the island, with the badge and live strip.
- Any integration can be added. Spotify, Cal.com and Notion were tested.
- The wardrobe has independent Head, Face and Neck slots.
- The three looks grow over 5 stages.
- The Seed intro replaces the old greeting, with no Coucou sounds left.
- STATUS.md is updated with what was tested on Windows and how.
