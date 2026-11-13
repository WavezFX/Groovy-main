# Fixes round 3J: licence housekeeping and a few daily-use wins (4 Nov)

**Do this after 3I.** One item per commit: "Fixes round 3J, item N: ...", tick it in `WORK_QUEUE.md` in the same commit, push, keep the build green. No user-facing feature is removed. Every item must keep Groovy under 300 MB idle and 400 MB at peak, add no work at idle, and load only when used.

## The rule for this round: change something only if it is clearly better
From the repo scouting (`docs/research/REPO_SCOUTING.md`). For each idea the item says what Groovy has today and what must be proven before anything is swapped. If the test does not show a clear win, **keep the current code and write the result in `STATUS.md`**. Never change a source or library just to change it.

| Idea | Groovy today | Verdict |
|---|---|---|
| Smart Turn + a neural VAD | `okgroovy/wake.rs` `Vad`: loudness threshold plus a fixed number of silent frames | **Test (item 2).** Likely a real gain in noisy rooms. |
| Moonshine speech-to-text | sherpa-onnx with the current default model | **Benchmark only (item 2).** Switch only if it uses less RAM or is clearly more accurate. |
| Handy's ideas (hotkey modes, CLI flags, model folder) | Dictation, shortcuts and a model folder already exist | **Do nothing.** No clear gain. |
| Flow Launcher Game Mode | `fullscreen_busy()` already hides the island | **Extend (item 3):** also stop listening and ticking while a game runs. |
| Clipboard history | none (clipboard is only used to paste dictation) | **Add (item 4)**, opt-in. |
| Dialog Jump | none | **Add if time (item 6)**, hotkey-only. |
| "Forget that", fact end-dates | none | **Add (3I item 4b).** |
| Windows UI Automation for messaging | already used to read text (`flow/os_windows.rs`, `browser.rs`) | **Keep for Rounds 6 and 7**; no new library. |
| Cua Driver, OpenAdapt, UI-TARS, OmniParser, Screenpipe, memory frameworks (Letta, Mem0, Graphiti, Cognee) | n/a | **Not now** (heavy, unclear licences, or Python). |

## Item 1: Licence housekeeping (the repo is now Apache-2.0; see `docs/LICENSING.md`)
- CI (fast check): add `cargo deny check licenses bans` with an allow-list (MIT, Apache-2.0, BSD-2/3, ISC, Unicode, Zlib, MPL-2.0 for unmodified crates, CC0, Unlicense, OpenSSL/ring terms). **Fail on GPL, AGPL and LGPL** crates in the release build (list any that appear and tell the owner; do not silently exempt).
- npm: run a licence checker on production dependencies; fail on GPL/AGPL.
- Generate `THIRD_PARTY_NOTICES` (crates and npm) at build time, ship it in the installer, and link it from Settings -> About.
- `windows/docs/licenses/MODELS.md`: for every model Groovy downloads or bundles (speech, wake word, Kokoro, any VAD), record name, source, licence, and whether credit is required. Show required credits in About.
- Sweep for stale licence text (README, About, `package.json`, docs) and for anything still saying GPL or MIT-for-everything.
- Rename the internal `mochi` identifiers (folder `src/mochi/`, `mochi.html`, labels) to `groovy` in one mechanical commit with tests green. Nothing user-facing changes. Skip this sub-step if it risks the build; say so in `STATUS.md`.

## Item 2: Better "has he finished speaking?" (benchmark first)
- Build a small test set (clean, noisy room, music, fan noise, long pauses mid-sentence, short answers like "yes") and measure the current RMS `Vad` for: cut-offs, wait time after the last word, false starts.
- Try (a) Silero VAD (check if sherpa-onnx already ships it) and (b) Smart Turn v3 (BSD-2, ~8 MB int8; first confirm an ONNX export exists, else skip it). Also benchmark Moonshine against the current dictation default (RAM, speed, word error on the same clips).
- **Adopt only if** it clearly lowers cut-offs and false triggers, adds no more than ~10 MB while a conversation is open, and nothing at idle (load on conversation start, unload after). Otherwise keep the current code. Record all numbers in `STATUS.md`.
- Never ship a model without its licence in `MODELS.md` (item 1).

## Item 3: Game Mode, fully
- Check what `fullscreen_busy()` stops today (it hides the island). When a full-screen game or presentation is running also: pause the wake-word microphone loop, suspend non-essential scheduler jobs, and ignore global shortcuts except Mute and "open island". Resume when it ends. Reuse the existing Windows notification-state check (no new polling: use the scheduler, 3F item 4).
- Add a Settings -> General switch ("Quiet during games and presentations"), on by default.
- Test: with the busy state forced, wake word and timers are idle (CPU near 0); turning it off restores them.

## Item 4: Clipboard history tile (opt-in)
- Off by default. When on: the last 20 text items, kept **in memory only** (cleared on exit; a switch to keep them on disk is off by default). Use the Windows clipboard-change event, not polling.
- Respect the privacy flags: ignore items that password managers mark as excluded from history/monitoring; skip anything larger than 10 KB; never send history to an AI unless the user picks one item and asks.
- A tile on the island: list, click to paste, "Clear". Nothing runs when the tile is off.

## Item 5: (moved) "Forget that" and fact end-dates are in `FIXES_ROUND3I.md` item 4b.

## Item 6: Dialog Jump (optional, only if items 1-4 are done)
- Hotkey-only, no background hook. In an Open/Save dialog, the shortcut lists the folders of open Explorer windows and the user's pinned folders; choosing one sets the dialog's path (Windows UI Automation; fall back to doing nothing).
- Test with the standard Windows common dialog. If it is flaky across apps, drop it and say so in `STATUS.md`.

## Item 7: Checks
- All new tests pass; idle private working set and CPU unchanged within noise; peak under 400 MB; every benchmark number and every "kept as is" decision recorded in `STATUS.md`.
