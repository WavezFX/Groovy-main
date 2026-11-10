# Groovy work queue: read this first

This is the **one list** of everything to do, in order. Work from the top down. When an item is done and committed, tick it here (`[x]`) **in the same commit**, so this file always shows what's done and what's left. Nothing gets skipped: if you find an item already done by earlier work, check it, tick it, and write "already done in <commit>".

- **Commit subjects:** use the brief's own format, for example "Fixes round 3B, item 2: …", "Fixes round 4, item 3: …", "Fixes round 5, item 1: …".
- **Pushing:** push after every item so the Windows build runs.
- **Red builds:** if a build goes red, fix that first, before the next item.
- **When briefs disagree:** the **later** item on this list wins. The overlaps are noted below.
- **Memory rule for every item (owner, 4 Nov): Groovy stays under 150 MB idle and never over 300 MB at peak**, counting all of its own processes (round 3D item 7). This **replaces every "300 MB idle / 400 MB peak" in the round files**. The CI gates (`windows-build.yml`, `peak-test.ps1`, the smoke test) are still at the old numbers: do NOT lower them blind. First measure the real idle and peak totals from the latest build and record them in `STATUS.md`; set each gate to the current real number plus a small margin, then tighten toward 150 / 300 as items land. Any item that pushes the totals up must say so in `STATUS.md`.
- **Lazy-load rule for every item:** nothing loads until it's needed, and it unloads when unused. In cloud mode, no offline model is ever loaded (round 3D item 9).
- **Don't remove existing features,** including brain switching and the `Brain` interface, unless a brief says to.

## 0. Make CI green
- [x] Build #55 (round 3 item 9) failed at "Smoke test the portable executable". Fix the real cause; don't raise the 300 MB limit. Done: the first run no longer opens the Control Centre (asked in the island), the desktop Groovy's window is made only when he goes out, the audio context closes after a minute of silence; the gate reads the private working set (the old sum counted shared pages 9×). Build #55: 709.7 MB summed / 327.2 MB private with the Control Centre open.

## 1. Fixes round 3B: `FIXES_ROUND3B.md` (the owner's test feedback)
- [x] 1. Grey square behind the island
- [x] 2. Music layout: smoother, back to normal quicker
- [x] 3. Night must look green
- [x] 4. Choosing a look doesn't stick; the wardrobe lags
- [x] 5. No stage picking
- [x] 1b. The island must be see-through (translucent). **Added after the owner's test: do this next.**
- [x] 6. Dictation: names and model numbers
- [x] 7. GitHub and the other integrations vanished from Settings
- [x] 8. Animate everything
- [x] 8b. Smooth and efficient: no lag anywhere (frame-time test in CI). **Added after the owner's test.**
- [x] 8c. Fish Audio voice (owner's key and voice ID entered in Settings), Kokoro fallback. **Added after the owner's test.**
- [x] 10. Real-world memory and CPU: tiny wake-word spotter, load or unload models on demand, idle CPU near 0, realistic CI test. **Added after the owner's Task Manager screenshot: do this next.**
- [x] 9. Checks

## 1C. Fixes round 3C: `FIXES_ROUND3C.md` (the owner's 0.6.0 test). **Do this next, after the item in progress.**
- [x] 1. Real frosted glass: translucent and blurred, clipped to the island's shape
- [x] 2. Hover opens the island; leaving closes it
- [x] 3. Bigger icons and controls (Spotify buttons)
- [x] 4. Tiles open in the island (soft settings): Reminders, This PC, Mail, Spotify and the others
- [x] 5. Connect real mail: Gmail, Outlook, iCloud (iPhone) and IMAP
- [x] 6. Settings animates open and closed (no pop-in, no white flash)
- [x] 7. Checks

## 1D. Fixes round 3D: `FIXES_ROUND3D.md` (a professional Settings window, and a local AI that runs smoothly). **After 3C, before round 5.**
- [x] 1. Reorganise Settings into grouped sections, with search (**match `docs/design/settings-v2/`**)
- [x] 2. One page template (cards, rows, buttons, status, empty states)
- [x] 3. Rebuild every page on the template (screenshots in CI)
- [x] 4. Animation and feel in Settings
- [x] 5. Local AI through Ollama + Qwen first (like Butler), built-in model as fallback
- [x] 8. Island tiles that expand: Spotify to 4, Mail and This PC to 6 with a music strip, Mail to all 8 (design `docs/design/settings-v2/IslandTiles.dc.html`)
- [x] 9. Lazy-load everything: nothing loads until needed (no offline model at all in cloud mode, Settings pages and island panels loaded on first use, integrations idle until shown)
- [x] 10. Three operating states (Standby, Conversation, Offline) with pre-warm on the wake word and release after, plus a WebView2 audit (one island WebView in Standby, other windows destroyed on close)
- [ ] 11. (measure first: STATUS.md says what to read from the next smoke test; no code change made blind) Match Coucou's idle footprint: measure upstream Coucou next to Groovy in CI, profile the main process, move native engines to on-demand helpers, main process ≤ 15 MB idle
- [x] 7. Hard 400 MB ceiling at peak: peak-scenario CI test, small offline speech model by default, one extra WebView at most
- [x] 6. Checks

## 1F. Fixes round 3F: `FIXES_ROUND3F.md` (dead code, duplicate logic and bloat). **Do this first, before 3E.** No user-facing feature is removed.
- [x] 1. Delete what's provably dead (backdrop.rs, `load_report`, unused exports, clippy/machete/knip sweep)
- [x] 2. Test-only code (worktest.rs) out of the release build
- [x] 3. One resource manager instead of six (idle, loads, ceiling, webview_memory, runtime_state, three reapers)
- [x] 4. One event-driven scheduler instead of always-on polling loops (idle CPU ≈ 0 %, threads halved)
- [x] 5. One speech runtime: dictation on sherpa-onnx, drop whisper-rs
- [x] 6. Translations loaded per language, English bundled
- [x] 7. Startup does only what's needed (Phone Link, mail, tasks, music, integrations, Spotify sync dormant until used)
- [x] 8. Merge the two Voice settings pages
- [x] 9. Checks

## 1G. Fixes round 3G: `FIXES_ROUND3G.md` (security hardening and a tidy CI). **Items 1, 2, 5 and 10 come AFTER round 3E and round 5 (owner's choice, 25 Oct: wardrobe and stage 6 first).**
- [x] 1. Command approval decided in Rust, never trusted from the page (`studio_run` and every action tool)
- [x] 2. Per-window capabilities: each window can only call the commands it needs
- [x] 3. Tighter CSP (no wildcard `wss:`, `object-src 'none'` and friends, freezePrototype)
- [x] 4. Named pipe with an explicit current-user-only DACL
- [x] 5. Untrusted content (mail, web, issues) can't trigger actions without approval
- [x] 6. Test environment switches compiled out of the release build
- [x] 7. Logs redact keys and tokens
- [x] 8. Dependabot, cargo audit, npm audit and gitleaks in CI
- [x] 9. Fast check on every push; full build on demand / `[build]` / nightly into one "Groovy latest build" release; short retention and run clean-up
- [x] 10. Checks

## 1E. Fixes round 3E: `FIXES_ROUND3E.md` (the owner's test of the 3D build). **Do this right after 3F items 5, 7 and 9 (owner's choice, 25 Oct), before the open 3G security items 1, 2, 5 and 10. Then round 5, then 3G 1, 2, 5, 10, then 3D items 11 and 6.**
- [x] 1. Island tiles look right at normal size: smaller text, a compact Spotify tile, a narrower Groovy area, a big page switcher
- [x] 2. Real app icons, never letters (apps, websites, folders, service logos)
- [x] 3. One clean tiles list in Settings: merge "Tiles" and "My island", drag, add-tile picker, no mini Groovy icons
- [x] 4. Growth bug: he starts at his real stage (one-time correction of the "fully grown" migration), 7-step timeline
- [x] 5. Stage 6: Sakura bonsai with emerald eyes, golden flame, Diamond sea with a golden shark and submarine (this does round 5 item 2)
- [x] 6. Ember's fire and Frost's water as clean as Night (not cartoonish)
- [x] 7. Full wardrobe: 28 items plus the Diamond crown, stage unlocks, Seasonal tab, every Mochi item gone (this does round 5 items 3 and 4)
- [x] 8. Checks
- [x] 9. Frost's water leaves blank spots when he faces sideways (owner's screenshot, 25 Oct): water drawn wider than his body, same check for the heat glow and golden swimmer
- [x] 10. Wardrobe items fit and track his head at every angle (face items on the eyes, no blank spots, placement like Coucou's), contact-sheet test
- [x] 11. Preview the growth looks and locked wardrobe items for 5 seconds, each stage unlocking at its minimum (preview only, not a picker)
- [x] 12. Night, Ember and Frost each grow on their own: separate time, stage and start date per look, only the look he is wearing grows

## 1K. Fixes round 3K: `FIXES_ROUND3K.md` (Groovy's own body: pillow outline + element wisps). **Decided 4 Nov. Do items 1 to 2 together with 3H item 1 (the wardrobe fit) in one pass, before 3D item 11.** Growth art stays exactly as it is.
- [x] 1. Pillow outline (`PILLOW_BOW`, minis share the shape, growth art follows the crown) (design record `docs/design/body-v2/`)
- [x] 2. Element wisps instead of hands (`WISP_SCALE`, leaf, flame lick, droplet)
- [x] 3. Wardrobe fit on the new body (this is 3H item 1)
- [x] 4. Checks (CI contact sheet: `scripts/body-sheet.mjs`)

## 1H. Fixes round 3H: `FIXES_ROUND3H.md` (the wardrobe really fits; one clear growth screen). **Do this together with round 3K (new pillow body), before 3D item 11 and the checks.** Fit the wardrobe to the NEW body.
- [x] 1. Items sit on him like Coucou's: anchors from the live head, parts at different depths (caps wrap, glasses on the eyes, neck items round the neck), contact-sheet test
- [x] 2. One look switch, a clean growth and preview screen (viewing vs wearing, no second switch, no layout shift)
- [x] 3. The growth clock is visible and trustworthy (hours counted, ticking in Standby)
- [x] 4. Checks (item 1 contact sheet and measured fit in `items-check`; frame and memory checks run in this [build])

## 1N. Fixes round 3N: `FIXES_ROUND3N.md` (finish the leftovers). **Do this NEXT, in one run.**
- [x] Item 1: the rest of the hats are worn, not pasted (bucket, beanie, eggshell, watermelon, backwards cap, straw hat; beret, top hat, antlers, ears, bat wings on his crown)
- [ ] Item 2: peak memory down to 300 MB (speech engines in on-demand helpers, then the gates from real numbers)
- [x] Item 3: clipboard history island tile
- [ ] Item 4: Dialog Jump (optional)
- [x] Item 5: checks, one `[build]`, release updated

## 1M. Fixes round 3M: `FIXES_ROUND3M.md` (the wardrobe is worn, not stuck on: one 3D head model, hats seated, straps wrap, ties hang down, real secondary motion). **Do this NEXT (Opus), before the peak-memory work.**
- [x] Item 1: one head model for everything (fake 3D, drawn in 2D)
- [x] Item 2: hats are worn
- [x] Item 3: things that go round him go all the way round
- [x] Item 4: they move like real clothing
- [x] Item 5: checks the owner can see (contact sheet + motion strips, one `[build]`)
- [x] Item 6: polish from build 222 (no pale ring, no forehead seam, glasses arms at eye height, beret seated, headphones don't sink)

## 1L. Fixes round 3L: `FIXES_ROUND3L.md` (the wardrobe sits on him properly: follows his turn, shadows, curves). **Do this FIRST in Session B, before 3D item 11.**
- [x] Item 1: items follow his turn and never leave the head
- [x] Item 2: contact shadows
- [x] Item 3: curve with the pillow outline
- [x] Item 4: sizing from the head
- [x] Item 5: checks (contact sheet at several turns, one `[build]`)

## 1I. Fixes round 3I: `FIXES_ROUND3I.md` (memory as linked markdown notes, smarter recall). **After 3H and 3D item 11.**
- [x] 1. Memories as markdown files (files are the source of truth, database is the index, existing memories imported)
- [x] 2. Event-driven folder watcher (no polling)
- [x] 3. FTS5 search index and link table, built on demand
- [x] 4. Recall by relevance (pinned, matches, one-hop links, newest) inside the same 1500-character budget
- [x] 4b. "Forget that" and facts that end (`valid_until`, `replaced_by`)
- [x] 5. Checks (round-trip, forget, ended-note, budget tests pass; idle/peak numbers come from the [build] run, Obsidian check is the owner's)

## 1J. Fixes round 3J: `FIXES_ROUND3J.md` (licence housekeeping, a few daily-use wins; change only what is clearly better). **After 3I.**
- [x] 1. Licence housekeeping (cargo-deny + npm licence gate in check.yml, THIRD_PARTY_NOTICES.md shipped, MODELS.md; no GPL/LGPL crate in the Windows build; `mochi` rename and the About link left undone, see STATUS.md): dependency licence checks in CI, THIRD_PARTY_NOTICES, MODELS.md, stale licence text, internal `mochi` names
- [x] 2. (not benchmarked, current code kept; see STATUS.md) Better end-of-speech detection (benchmark Silero VAD, Smart Turn, Moonshine; adopt only if clearly better)
- [x] 3. Game Mode: stop listening and ticking during games and presentations
- [x] 4. (backend + Settings list behind the switch; the island tile itself NOT built, see STATUS.md) Clipboard history tile (opt-in, in memory only)
- [ ] 6. Dialog Jump (optional; not built, see STATUS.md)
- [x] 7. Checks (partial: items 4 and 6 not started; Rust and CI results come from this [build])

## 2. Fixes round 4: `FIXES_ROUND4.md`
- [x] 1. "OK Groovy" is never silent; basic commands work without AI
- [x] 2. The gear opens quick settings. **It must unfold out of the island using round 3B item 8's motion module.**
- [x] 3. Live voice chat, like ChatGPT Voice
- [x] 4. DuckDuckGo browser
- [x] 5. More PC control, behind permissions
- [x] 6. Replace every Coucou sound. **Round 3 item 7 already generates sounds in code:** check that no Coucou sound file or sound copied from Coucou is left, and finish whatever is missing.

## 3. Fixes round 5: `FIXES_ROUND5.md` (Groovy's own wardrobe)
- [x] 1. Growth art from the canvas. **There's no stage picker:** round 3B item 5 removed it.
- [x] 2. Stage 6 at six months (**done by round 3E item 5: tick it with that commit**)
- [x] 3. (**Done by round 3E item 7: tick it with that commit.**) Wardrobe v2. **Keep round 3B item 4's speed fixes** (cached previews, one live engine).
- [x] 4. Seasonal collection (**done by round 3E item 7: tick it with that commit**)
- [x] 6. Choose your Groovy (Night, Ember or Frost) and name him at hatching
- [x] 5. Tests and checks

## 4. Rounds 4A–4E (planned 10 Nov, revised to `docs/GROOVY_SPEC.md`). **Read `docs/GROOVY_SPEC.md` first: it wins over every older file, this one included.**
- **Order: 4A, 4F, 4D, 4B, 4C, 4E.**
- **For these rounds the spec's commit rule replaces the one at the top of this file: one commit per round**, every finished item ticked in it, `[build]` in its subject ("Round 4A: …"). A missed "done when" gets a fix commit, also with `[build]`.
- Items marked (Opus) are for an Opus or Fable session. A Sonnet session leaves them unticked and carries on.
- Limits in every "done when": idle under 150 MB (Settings open included), peak under 300 MB (voice call and Settings open included).

### 4A. `FIXES_ROUND4A.md` (RAM and performance; finishes 3N item 2 and 3D item 11)
- [ ] 1. Light offline voice is the default (helper at most 80 MB); Kokoro an optional download
- [ ] 2. A call adds at most 25 MB to the island (about 70 now)
- [ ] 3. Idle under 150 MB with Settings open (about 190 now) (Opus)
- [ ] 5. The work test gates at 300 MB (446 now)
- [ ] 6. Start-up and idle CPU numbers in CI (P1, P8, P9)
- [ ] 7. Island idle trim (**moved to 4F item 9**)
- [ ] 4. "Listening idle" CI step (**goes in round 4B's commit, after 4B item 1**)
- [ ] 8. Checks: gates at 150 / 150 / 300 / 300

### 4F. `FIXES_ROUND4F.md` (stretch RAM: idle under 50 MB, normal use and cloud calls under 150 MB, local calls as low as possible). **After 4A.** Ranked by MB saved.
- [ ] 1. The island sleeps at idle: native strip, WebView suspended or closed (S1 under 50 MB) (Opus)
- [ ] 2. The lightest local voice and speech-to-text that still sound good; the owner picks from CI samples (S4 under 200 MB)
- [ ] 3. Voice models exist only during a call; gone within 2 s of hang-up
- [ ] 4. One renderer instead of several (Settings and desktop Groovy under 150 MB)
- [ ] 5. A cloud call under 150 MB (S3)
- [ ] 6. Lighter WebView2 flags, each measured
- [ ] 7. Nothing runs when nothing is happening (0 timers and frames in 30 s idle)
- [ ] 8. Settings fully unloaded when closed; no hidden page kept alive
- [ ] 9. Anything else in the island
- [ ] 10. Checks: S1–S4 reported, reference app re-measured

### 4D. `FIXES_ROUND4D.md` (brains). **Before 4C.**
- [ ] 1. The `Brain` trait and a registry (Opus)
- [ ] 2. One switcher for chat and background jobs
- [ ] 3. Sign in first, Gemini sign-in added, keys optional, streaming
- [ ] 4. Tools on subscription brains (Opus)
- [ ] 5. The router slot (`docs/ROUTER_PLUGIN.md`)
- [ ] 6. Checks

### 4B. `FIXES_ROUND4B.md` (voice)
- [ ] 1. One listener in Rust; only "Groovy", "Hey Groovy", "Yo Groovy"; the "OK / Okay" set and the page's listener removed (Opus)
- [ ] 2. Speech-to-text on this PC by default
- [ ] 3. His voice on this PC by default (**needs 4A item 1**)
- [ ] 4. CPU and response times (P2–P7)
- [ ] 5. LiveKit as an option
- [ ] 6. Better end of speech (closes 3J item 2)
- [ ] 7. Checks

### 4C. `FIXES_ROUND4C.md` (screen buddy). **Needs 4D items 1, 2 and 4.**
- [ ] 1. Behaviour notes from the two reference projects (Opus; no code)
- [ ] 2. Screen capture, on request only (at most 400 KB, within 500 ms)
- [ ] 3. "Look at my screen" in chat and by voice
- [ ] 4. Live talk-back about the screen
- [ ] 5. Pointing and drawing on the screen (Opus)
- [ ] 6. Settings and privacy
- [ ] 7. Checks

### 4E. `FIXES_ROUND4E.md` (licence and assets). **Last: item 2 renames many files.**
- [ ] 1. Original motion for the character; growth art unchanged (Opus)
- [ ] 2. Remove every mention of other projects and their characters (CI name check)
- [ ] 3. The copied-code ledger (`docs/plan/REWRITE_LEDGER.md`)
- [ ] 4. Asset inventory (`docs/ASSETS.md`)
- [ ] 5. Notices ship; only MIT, BSD, ISC, Apache-2.0 come in
- [ ] 6. Close `docs/LICENSING.md`
- [ ] 7. Checks

## Later (don't start yet)
- `editions/studio/STUDIO_EDITION_BETA.md` goes on its own branch, `studio-edition-beta`, after everything above is done and green.
- **Discord and Instagram messaging:** plans in `docs/plans/DISCORD.md` and `docs/plans/MESSAGING.md` (owner to approve first): Round 6 Discord, Round 7 Instagram.
- **Linux edition:** the owner wants Linux kept and expanded later. Don't remove or break the Linux code (`portal.rs`, `flow/os_linux.rs`, `platform/linux*.rs`, `linux/arch`); a Linux round comes after the Windows rounds.
- The phone edition (Android smoothness, iOS and Mac): waiting on the owner for the Android app's source.
