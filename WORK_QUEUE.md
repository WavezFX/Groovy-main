# Groovy work queue: read this first

This is the **one list** of everything to do, in order. Work from the top down. When an item is done and committed, tick it here (`[x]`) **in the same commit**, so this file always shows what's done and what's left. Nothing gets skipped: if you find an item already done by earlier work, check it, tick it, and write "already done in <commit>".

- **Commit subjects:** use the brief's own format, for example "Fixes round 3B, item 2: …", "Fixes round 4, item 3: …", "Fixes round 5, item 1: …".
- **Pushing:** push after every item so the Windows build runs.
- **Red builds:** if a build goes red, fix that first, before the next item.
- **When briefs disagree:** the **later** item on this list wins. The overlaps are noted below.
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
- [ ] 1. Real frosted glass: translucent and blurred, clipped to the island's shape
- [ ] 2. Hover opens the island; leaving closes it
- [ ] 3. Bigger icons and controls (Spotify buttons)
- [ ] 4. Tiles open in the island (soft settings): Reminders, This PC, Mail, Spotify and the others
- [ ] 5. Connect real mail: Gmail, Outlook, iCloud (iPhone) and IMAP
- [ ] 6. Settings animates open and closed (no pop-in, no white flash)
- [ ] 7. Checks

## 2. Fixes round 4: `FIXES_ROUND4.md`
- [x] 1. "OK Groovy" is never silent; basic commands work without AI
- [x] 2. The gear opens quick settings. **It must unfold out of the island using round 3B item 8's motion module.**
- [x] 3. Live voice chat, like ChatGPT Voice
- [x] 4. DuckDuckGo browser
- [x] 5. More PC control, behind permissions
- [x] 6. Replace every Coucou sound. **Round 3 item 7 already generates sounds in code:** check that no Coucou sound file or sound copied from Coucou is left, and finish whatever is missing.

## 3. Fixes round 5: `FIXES_ROUND5.md` (Groovy's own wardrobe)
- [ ] 1. Growth art from the canvas. **There's no stage picker:** round 3B item 5 removed it.
- [ ] 2. Stage 6 at six months
- [ ] 3. Wardrobe v2. **Keep round 3B item 4's speed fixes** (cached previews, one live engine).
- [ ] 4. Seasonal collection
- [ ] 6. Choose your Groovy (Night, Ember or Frost) and name him at hatching
- [ ] 5. Tests and checks

## Later (don't start yet)
- `editions/studio/STUDIO_EDITION_BETA.md` goes on its own branch, `studio-edition-beta`, after everything above is done and green.
- The phone edition (Android smoothness, iOS and Mac): waiting on the owner for the Android app's source.
