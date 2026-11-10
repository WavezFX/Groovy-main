# Groovy spec (owner's brief, 10 Nov 2026)

The one statement of what Groovy is. **Where a round file, `HANDOVER.md` or `STATUS.md` disagrees with this file, this file wins.** Read it first in every session, then `graphify-out/GRAPH_REPORT.md`, then only the files the item needs.

## 1. Product
- A Windows desktop companion by SSD. Tauri 2: Rust in `windows/src-tauri/src`, TypeScript in `windows/src`.
- Free edition first. **Studio Edition is out of scope.**
- Every asset is original: character, looks, wardrobe, sounds, icons. Other projects were inspirations only.
- Platforms: Windows now. Android phone link now. iOS, Mac and Linux later: **keep the Linux code compiling and untouched.**

## 2. Character
- Hatches bare from a seed; the user picks a look and names him.
- Body: the "pillow" outline (sides bow in slightly, corners stay full) with small element wisps (leaf, flame, droplet). No oval hands.
- Three looks:
  - **Night** (default): body with a slight green glow, sharp glowing green eyes, a green halo.
  - **Ember** and **Frost**.
- Growth: each look grows on its own, only by time spent wearing it.
  - Night → Grove (bonsai, sakura). Ember → Blaze (fire). Frost → Tide (water, marine life, submarine).
  - **The existing growth art stays exactly as it is.**
- Animation: the engine's motion must be original (timing, easing, blink, breathing, emotes, particles). Anything ported from another project is rewritten.
- Wardrobe: Groovy's own items only, worn on the head model, with stage unlocks and a seasonal set.

## 3. UI
- One island at the top of the screen holds everything. No separate bottom pill.
- Groovy's own design: frosted glass, everything animated out of and back into the island.
- Soft settings in the island; hard settings in the Settings window.

## 4. Voice
- Wake words: **only "Groovy", "Hey Groovy", "Yo Groovy"**. A bare "Groovy" counts only as the first word said.
- **One listener**, in Rust. The "OK Groovy" / "Okay Groovy" set is removed, and so is the second listener in the page.
  - Waking to his given name is not in the list, so it is removed too. *Owner to confirm.*
- Speech-to-text runs on this PC by default. Cloud speech services are opt-in.
- His voice runs on this PC by default: the **light offline voice**. Kokoro is an optional download. Cloud voices (Fish Audio, ElevenLabs) are opt-in.
- LiveKit is an option for live conversation.
- Never silent: if something is missing, he says or shows what.

## 5. Screen buddy
- A live, talk-back conversation about what is on screen.
- He looks at the screen, or takes a screenshot, **only when asked**. Captures stay in memory, go only to the brain answering, and are dropped after the reply.
- He explains what he sees and draws on the screen (arrow, circle, box, underline, short label).
- Two MIT projects are read for behaviour only; no asset or code is copied. They are named only in `FIXES_ROUND4C.md`, `FIXES_ROUND4E.md` and the reference-measure workflow, and those lines are deleted when round 4E closes.

## 6. Brains
- **One `Brain` interface in Rust** that every provider implements: Claude, Gemini, Codex, OpenAI-compatible services, local servers.
- Sign-in with the user's own account and plan comes first, with no API key needed. API keys are optional.
  - Claude and Codex sign-in exist. **Gemini sign-in is added.**
  - **Subscription brains can use tools** (voice commands, screen buddy).
- Switching between brains stays: order, switch when a limit is hit, model per job.
- A slot for a later **router plugin** (small models for light jobs, big ones for heavy jobs). Groovy's own switching stays as the backup.
- Order stays cloud and subscription first, local last. Don't redesign features around local Ollama.

## 7. Other features (keep current behaviour; plan only)
- **Skills manager** on Vercel's `skills` CLI: analyses a project, recommends, installs, removes and edits agent skills, always with the user's approval.
- **Android phone link:** must feel smooth (pairing, session list, approvals, chat). Blocked on the Android app's source.
- **Later:** Discord (mute, deafen, messages) and Instagram messages by voice, through the installed apps.
- Mail, Spotify and music, reminders, This PC, clipboard, memory notes, agent hooks, PC control, Game Mode: unchanged.

## 8. Hard limits
Counted as Task Manager's private working set, summed over every process Groovy owns.
| Limit | Value | Known today |
|---|---|---|
| Idle | **under 150 MB**, including with Settings open | 105–115 MB island only; about 190 MB with Settings open |
| Peak | **never above 300 MB**, including during a voice call and with Settings open | 331 MB in a call (offline voice helper 171 MB; the call adds about 70 MB to the island) |
All three gaps must be fixed: the Settings-open idle, the voice helper, and what a call adds to the island.

### Targets: APPROVED by the owner, 10 Nov 2026
Measured on the Windows CI runner (4 cores) unless noted.
| # | What | Target | Today |
|---|---|---|---|
| P1 | CPU at idle (island only, 60 s) | ≤ 0.3 % of the machine | 0.04–0.18 % |
| P2 | CPU while listening, room quiet (60 s) | ≤ 1.0 % | not measured |
| P3 | CPU while listening, someone talking, no wake word | ≤ 3.0 % | not measured |
| P4 | Wake-word response: end of the phrase to the listening cue | ≤ 500 ms (p95) | not measured |
| P5 | Missed and false wakes on the test set | ≥ 9 of 10 wake phrases heard; 0 of 20 decoys | 14 of 16; 0 of 16 (old phrases) |
| P6 | Voice reply, local parts: end of speech to transcript, plus first clause of voice | ≤ 1.0 s added to the brain's own time | not measured |
| P7 | Voice reply, end to end with a cloud brain: end of speech to first sound | ≤ 2.5 s (p50), ≤ 4 s (p95) | about 9.5 s for a whole reply |
| P8 | Start-up: launch to the island drawn | ≤ 2.0 s cold, ≤ 1.0 s warm | not measured |
| P9 | Start-up: launch to listening for the wake word | ≤ 4.0 s | not measured |
| P10 | Frame time in island animations | p95 ≤ 20 ms, none over 50 ms | p95 15.7 ms |
| P11 | Screen look: "look at my screen" to the capture sent | ≤ 500 ms; capture ≤ 400 KB | not built |

### Stretch targets (owner, 10 Nov 2026)
The hard limits above gate the build. These are the goals the work aims at after them; a CI step reports each, and it becomes a gate once it is met.
| # | What | Stretch target | Today |
|---|---|---|---|
| S1 | Idle, total | **under 50 MB** | 105–115 MB (WebView2 is about 100 of it) |
| S2 | Peak in normal use (island open, panels, chat, Settings) | **under 150 MB** | about 190 MB with Settings open |
| S3 | Peak in a call with a cloud brain and cloud or Windows voice (no local model loaded) | **under 150 MB** | about 160 MB (331 less the 171 MB voice helper) |
| S4 | Peak in a call with the local voice and local speech-to-text | **as low as possible**; first goal under 200 MB | 331 MB |
- The reference app is measured on the same runner with the same counters (`.github/workflows/reference-measure.yml`); its numbers sit beside Groovy's in `STATUS.md`.
- Round `FIXES_ROUND4F.md` holds the work.

## 9. Licence
- **Apache-2.0** for Groovy's code. Name, character and artwork stay SSD's (`LICENSE-ASSETS.md`).
- Only MIT, BSD, ISC or Apache-2.0 code and models may be added (CC-BY models with visible credit). **No GPL, AGPL or LGPL.**
- Every mention of other companion projects or their characters is removed from code, comments, file names, settings keys, docs and the app.
- Code that was copied from an MIT project is rewritten. Until it is, it keeps a credit in `THIRD_PARTY_NOTICES.md` (copyright line and the MIT text, which the MIT licence requires), and nowhere else.
- Phone link code keeps its author's MIT credit in `THIRD_PARTY_NOTICES.md`.

## 10. Rules for every session
- Read this file and the graph first. Open only the files the item needs; read big files by line range.
- Opus or Fable for the items marked **(Opus)**; Sonnet for the rest.
- **One commit per round**, every finished item ticked in `WORK_QUEUE.md` in that commit, `[build]` in its subject. If the build's numbers miss a "done when" line, a fix commit follows, also with `[build]`.
- Memory, CPU and timing are measured on Windows CI only. Write before and after in `STATUS.md`. Never lower a gate blind; never raise one.
- Don't remove a feature unless this file says so. Never commit keys.
- New Tauri command: `node scripts/command-access.mjs`. New visible text: all ten languages. New setting: `tests/settings-keys.test.mjs`.
